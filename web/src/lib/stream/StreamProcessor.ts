// StreamProcessor — client state machine for one assistant message stream.
//
// LobeHub analogue: src/store/chat/agents/StreamingHandler.ts
//
// Responsibilities:
//   • Accumulate content streaming state for one message (text, reasoning,
//     tool calls, search, usage).
//   • Emit partial ChatMessage patches per ChatEvent so the store can update
//     React state incrementally without rebuilding the whole message array.
//   • Track lifecycle (generating, first-reasoning-seen, etc.).
//
// What it does NOT do:
//   • Talk to React / zustand directly. Caller applies patches.
//   • Handle Oxios-semantic chunks (model, memory, interview_question,
//     tool_approval, mount_detected) — those stay on the store's legacy arms.
//   • RAF batching. Caller batches text.delta events before calling.
//
// One StreamProcessor per assistant message. Store keeps Map<msgId, StreamProcessor>.
// See docs/designs/2026-07-21-lobehub-chat-port-design.md §6.2.

import type { ChatMessage } from '@/types'
import type { ChatError, ChatFileChunk, ChatToolPayload, ChatToolStatus } from '@/types/chat'
import type { ChatEvent, TokenUsage } from './ChatEvent'

export interface ProcessorResult {
  /** Partial ChatMessage patch to merge into the stored message. */
  patch: Partial<ChatMessage>
  /** Set when the stream has terminated (stop event). Cleanup hint for store. */
  finished?: boolean
  /** Side-effect activity emission — caller may push as ChatActivity for
   *  backward-compat with existing timeline rendering (Phase 1 keeps activities
   *  working alongside new toolCalls field). */
  activity?: ChatActivityEmission
}

/** Activity side-channel emission (kept for Phase 1 backward compat). */
export interface ChatActivityEmission {
  type: 'tool_call' | 'usage'
  toolCallId?: string
  /** 'merge' = update existing activity by id; 'append' = push new activity. */
  mode: 'merge' | 'append'
  patch: Record<string, unknown>
}

/**
 * State machine for one streaming assistant message.
 *
 * Construct with the message id; feed ChatEvents via handleEvent; apply
 * returned patch to the store; on `finished: true`, call materialize() for
 * a clean final state and discard the processor.
 */
export class StreamProcessor {
  readonly messageId: string

  private text = ''
  private reasoningText = ''
  private reasoningStartTs: number | null = null
  private reasoningEverSeen = false
  private tools = new Map<string, ChatToolPayload>()
  private search: ChatMessage['search'] = null
  private chunks: ChatFileChunk[] = []
  private lastUsage: TokenUsage | null = null
  private error: ChatError | null = null
  private stopped = false

  constructor(messageId: string) {
    this.messageId = messageId
  }

  /** Feed one ChatEvent. Returns incremental patch + lifecycle signals. */
  handleEvent(ev: ChatEvent): ProcessorResult {
    if (this.stopped && ev.kind !== 'stream.stop') {
      return { patch: {} }
    }

    switch (ev.kind) {
      case 'text.delta':
        this.text += ev.text
        return { patch: { content: this.text, generating: true } }

      case 'reasoning.start':
        this.beginReasoning()
        return { patch: { isReasoning: true, generating: true } }

      case 'reasoning.delta': {
        if (!this.reasoningEverSeen) this.beginReasoning()
        this.reasoningText += ev.text
        return {
          patch: {
            isReasoning: true,
            generating: true,
            reasoning: {
              content: this.reasoningText,
              duration: this.reasoningDuration(),
              thinking: true,
            },
          },
        }
      }

      case 'reasoning.end': {
        const duration = ev.durationMs ?? this.reasoningDuration()
        return {
          patch: {
            isReasoning: false,
            reasoning: {
              content: this.reasoningText,
              duration,
              thinking: false,
            },
          },
        }
      }

      case 'tool.args_delta': {
        // oxi 0.58+: partial tool-call args streamed by the LLM before
        // ToolExecutionStart. Create a placeholder if this tool_call_id is
        // unseen; otherwise accumulate the raw JSON fragment. When tool.start
        // arrives it replaces the placeholder with the parsed args + real name.
        const cur = this.tools.get(ev.toolCallId)
        if (!cur) {
          this.tools.set(ev.toolCallId, {
            id: ev.toolCallId,
            identifier: 'kernel',
            apiName: '(constructing…)',
            arguments: ev.argsDelta,
            status: 'loading' satisfies ChatToolStatus,
            startedAt: Date.now(),
          })
        } else {
          this.tools.set(ev.toolCallId, {
            ...cur,
            arguments:
              (typeof cur.arguments === 'string' ? cur.arguments : '') +
              ev.argsDelta,
          })
        }
        return { patch: { toolCalls: this.toolsList() } }
      }

      case 'tool.start': {
        const tool: ChatToolPayload = {
          id: ev.toolCallId,
          identifier: 'kernel',
          apiName: ev.toolName,
          arguments: ev.args,
          status: 'loading' satisfies ChatToolStatus,
          startedAt: Date.now(),
          ...(ev.tabId !== undefined ? { tabId: ev.tabId } : {}),
        }
        this.tools.set(ev.toolCallId, tool)
        return {
          patch: {
            toolCalls: this.toolsList(),
            isToolCallGenerating: true,
            generating: true,
          },
          activity: {
            type: 'tool_call',
            toolCallId: ev.toolCallId,
            mode: 'append',
            patch: {
              toolName: ev.toolName,
              toolCallId: ev.toolCallId,
              toolArgs: ev.args,
              isRunning: true,
              ...(ev.tabId !== undefined ? { tabId: ev.tabId } : {}),
            },
          },
        }
      }

      case 'tool.progress': {
        const cur = this.tools.get(ev.toolCallId)
        if (!cur) return { patch: {} }
        const next: ChatToolPayload = {
          ...cur,
          progress: ev.progress,
          ...(ev.tabId !== undefined ? { tabId: ev.tabId } : {}),
        }
        this.tools.set(ev.toolCallId, next)
        return {
          patch: { toolCalls: this.toolsList() },
          activity: {
            type: 'tool_call',
            toolCallId: ev.toolCallId,
            mode: 'merge',
            patch: {
              progress: ev.progress,
              ...(ev.tabId !== undefined ? { tabId: ev.tabId } : {}),
            },
          },
        }
      }

      case 'tool.end': {
        const cur = this.tools.get(ev.toolCallId)
        if (!cur) return { patch: {} }
        const status: ChatToolStatus = ev.error ? 'error' : 'success'
        const endedAt = Date.now()
        const durationMs = ev.durationMs ?? (cur.startedAt ? endedAt - cur.startedAt : undefined)
        const next: ChatToolPayload = {
          ...cur,
          result: ev.result,
          error: ev.error ?? null,
          status,
          endedAt,
          durationMs,
        }
        this.tools.set(ev.toolCallId, next)
        const allSettled = [...this.tools.values()].every(
          t => t.status === 'success' || t.status === 'error' || t.status === 'aborted',
        )
        return {
          patch: {
            toolCalls: this.toolsList(),
            isToolCallGenerating: !allSettled,
          },
          activity: {
            type: 'tool_call',
            toolCallId: ev.toolCallId,
            mode: 'merge',
            patch: {
              isRunning: false,
              isError: !!ev.error,
              outputSummary: summariseResult(ev.result),
              durationMs,
            },
          },
        }
      }

      case 'grounding':
        this.search = ev.search
        return { patch: { search: ev.search } }

      case 'file_chunks':
        this.chunks = ev.chunks
        return { patch: { chunksList: ev.chunks } }

      case 'usage':
        this.lastUsage = ev.usage
        return {
          patch: {
            totalInputTokens: ev.usage.inputTokens,
            totalOutputTokens: ev.usage.outputTokens,
          },
          activity: {
            type: 'usage',
            mode: 'append',
            patch: {
              inputTokens: ev.usage.inputTokens,
              outputTokens: ev.usage.outputTokens,
            },
          },
        }

      case 'phase':
        return { patch: {} }

      case 'stream.stop':
        this.stopped = true
        this.error = ev.error ?? null
        return {
          patch: {
            generating: false,
            isReasoning: false,
            isToolCallGenerating: false,
            error: ev.error ?? undefined,
          },
          finished: true,
        }

      default: {
        const _exhaustive: never = ev
        void _exhaustive
        return { patch: {} }
      }
    }
  }

  /** Produce final ChatMessage (snapshot of accumulated state). */
  materialize(base: ChatMessage): ChatMessage {
    return {
      ...base,
      id: this.messageId,
      content: this.text || base.content,
      reasoning:
        this.reasoningText || base.reasoning
          ? {
              content: this.reasoningText || base.reasoning?.content || '',
              duration: this.reasoningDuration() ?? base.reasoning?.duration,
              thinking: false,
            }
          : null,
      toolCalls: this.tools.size ? this.toolsList() : base.toolCalls,
      search: this.search ?? base.search,
      chunksList: this.chunks.length ? this.chunks : base.chunksList,
      totalInputTokens: this.lastUsage?.inputTokens ?? base.totalInputTokens,
      totalOutputTokens: this.lastUsage?.outputTokens ?? base.totalOutputTokens,
      error: this.error ?? base.error ?? null,
      generating: false,
      isReasoning: false,
      isToolCallGenerating: false,
    }
  }

  // ── Internals ──

  private beginReasoning() {
    if (!this.reasoningEverSeen) {
      this.reasoningEverSeen = true
      this.reasoningStartTs = Date.now()
    }
  }

  private reasoningDuration(): number | undefined {
    return this.reasoningStartTs ? Date.now() - this.reasoningStartTs : undefined
  }

  private toolsList(): ChatToolPayload[] {
    return [...this.tools.values()]
  }
}

/** Compress a tool result into a short human-readable summary for activity cards. */
function summariseResult(result: unknown): string | undefined {
  if (result == null) return undefined
  if (typeof result === 'string') {
    return result.length > 120 ? result.slice(0, 117) + '...' : result
  }
  try {
    const json = JSON.stringify(result)
    if (!json) return undefined
    return json.length > 120 ? json.slice(0, 117) + '...' : json
  } catch {
    return undefined
  }
}
