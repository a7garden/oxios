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
import type { ChatBlock, ChatError, ChatFileChunk, ChatToolPayload, ChatToolStatus } from '@/types/chat'
import type { ChatEvent, TokenUsage } from './ChatEvent'

/** Total reasoning text budget per turn — bounds the persisted trace. On
 *  overflow, further reasoning deltas are dropped (a marker is left on the
 *  last reasoning block by the caller if desired). */
const REASONING_BUDGET_BYTES = 16 * 1024

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
  // ── Block-stream transparency (single source of truth for the timeline) ──
  private blocks: ChatBlock[] = []
  private reasoningSeq = 0
  private textSeq = 0
  private reasoningBytes = 0

  constructor(messageId: string) {
    this.messageId = messageId
  }

  /** Feed one ChatEvent. Returns incremental patch + lifecycle signals.
   *  Always attaches the block-stream timeline (`blocks`) to the patch —
   *  the single source of truth rendered by BlockStream. */
  handleEvent(ev: ChatEvent): ProcessorResult {
    const result = this.handleEventInner(ev)
    return { ...result, patch: { ...result.patch, blocks: [...this.blocks] } }
  }

  private handleEventInner(ev: ChatEvent): ProcessorResult {
    if (this.stopped && ev.kind !== 'stream.stop') {
      return { patch: {} }
    }

    switch (ev.kind) {
      case 'text.delta':
        this.text += ev.text
        this.appendText(ev.text)
        return { patch: { content: this.text, generating: true } }

      case 'reasoning.start':
        this.openReasoning()
        return { patch: { isReasoning: true, generating: true } }

      case 'reasoning.delta': {
        if (!this.reasoningEverSeen) this.beginReasoning()
        this.reasoningText += ev.text
        this.appendReasoning(ev.text)
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
        this.closeReasoningBlock()
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
            arguments: (typeof cur.arguments === 'string' ? cur.arguments : '') + ev.argsDelta,
          })
        }
        this.closeReasoningBlock()
        this.upsertToolBlock(this.tools.get(ev.toolCallId)!)
        return { patch: { toolCalls: this.toolsList(), ...this.closeReasoningIfOpen() } }
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
        this.closeReasoningBlock()
        this.upsertToolBlock(tool)
        return {
          patch: {
            toolCalls: this.toolsList(),
            isToolCallGenerating: true,
            generating: true,
            ...this.closeReasoningIfOpen(),
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
        this.upsertToolBlock(next)
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
        this.upsertToolBlock(next)
        const allSettled = [...this.tools.values()].every(
          (t) => t.status === 'success' || t.status === 'error' || t.status === 'aborted',
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
        this.closeAllBlocks()
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
      blocks: [...this.blocks],
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
  /**
   * Close an open reasoning span when the stream transitions to tool
   * execution. Reasoning_content providers (GLM/DeepSeek/Qwen) that never
   * emit ThinkingEnd go straight from reasoning to a tool call with no Text,
   * so the gateway's first-Text `reasoning.end` heuristic never fires — the
   * Thinking spinner would otherwise run through the entire tool run.
   * Returns the patch fragment to merge, or `{}` if no reasoning was open.
   * Idempotent: reasoningText/reasoningStartTs are kept so a reasoning span
   * that resumes after the tool continues accumulating.
   */
  private closeReasoningIfOpen(): Partial<ChatMessage> {
    if (!this.reasoningEverSeen) return {}
    return {
      isReasoning: false,
      reasoning: {
        content: this.reasoningText,
        duration: this.reasoningDuration(),
        thinking: false,
      },
    }
  }

  private reasoningDuration(): number | undefined {
    return this.reasoningStartTs ? Date.now() - this.reasoningStartTs : undefined
  }

  private toolsList(): ChatToolPayload[] {
    return [...this.tools.values()]
  }
  // ── Block-stream helpers (2026-07-27) ───────────────────────────────
  // Mutate `this.blocks` immutably (replace objects, never in-place) so the
  // shallow clone `[...this.blocks]` returned in each patch yields new block
  // refs and React detects per-block changes. Block ids are counter-assigned
  // at open (stable across re-renders / rAF batches); tool blocks reuse
  // tool_call_id.

  private openReasoning(): void {
    this.reasoningSeq++
    this.blocks.push({
      type: 'reasoning',
      id: `r-${this.messageId}-${this.reasoningSeq}`,
      text: '',
      status: 'streaming',
      startedAt: Date.now(),
    })
  }

  private appendReasoning(text: string): void {
    if (this.reasoningBytes >= REASONING_BUDGET_BYTES) return
    const slice = text.slice(0, REASONING_BUDGET_BYTES - this.reasoningBytes)
    if (!slice) return
    this.reasoningBytes += slice.length
    const i = this.blocks.length - 1
    const last = this.blocks[i]
    if (last && last.type === 'reasoning' && last.status === 'streaming') {
      this.blocks[i] = { ...last, text: last.text + slice }
    } else {
      this.openReasoning()
      const j = this.blocks.length - 1
      const cur = this.blocks[j]
      if (cur && cur.type === 'reasoning') this.blocks[j] = { ...cur, text: slice }
    }
  }

  /** Close the trailing reasoning block if it is still streaming. */
  private closeReasoningBlock(): void {
    const i = this.blocks.length - 1
    const last = this.blocks[i]
    if (last && last.type === 'reasoning' && last.status === 'streaming') {
      this.blocks[i] = { ...last, status: 'done', durationMs: Date.now() - last.startedAt }
    }
  }

  /** Append text to the trailing text block, opening a new one (and closing
   *  any open reasoning span) when the previous block isn't text. */
  private appendText(text: string): void {
    this.closeReasoningBlock()
    const i = this.blocks.length - 1
    const last = this.blocks[i]
    if (last && last.type === 'text') {
      this.blocks[i] = { ...last, text: last.text + text, streaming: true }
    } else {
      this.textSeq++
      this.blocks.push({
        type: 'text',
        id: `t-${this.messageId}-${this.textSeq}`,
        text,
        streaming: true,
      })
    }
  }

  /** Insert or replace the tool block for `payload.id`, preserving stream position. */
  private upsertToolBlock(payload: ChatToolPayload): void {
    const i = this.blocks.findIndex((b) => b.type === 'tool' && b.id === payload.id)
    const block: ChatBlock = { type: 'tool', ...payload }
    if (i >= 0) this.blocks[i] = block
    else this.blocks.push(block)
  }

  /** Mark every open block as done (called on stream.stop). */
  private closeAllBlocks(): void {
    for (let i = 0; i < this.blocks.length; i++) {
      const b = this.blocks[i]!
      if (b.type === 'text') {
        this.blocks[i] = { ...b, streaming: false }
      } else if (b.type === 'reasoning' && b.status === 'streaming') {
        this.blocks[i] = { ...b, status: 'done', durationMs: Date.now() - b.startedAt }
      }
    }
  }
}

/** Compress a tool result into a short human-readable summary for activity cards. */
function summariseResult(result: unknown): string | undefined {
  if (result == null) return undefined
  if (typeof result === 'string') {
    return result.length > 120 ? `${result.slice(0, 117)}...` : result
  }
  try {
    const json = JSON.stringify(result)
    if (!json) return undefined
    return json.length > 120 ? `${json.slice(0, 117)}...` : json
  } catch {
    return undefined
  }
}
