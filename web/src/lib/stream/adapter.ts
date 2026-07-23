// adapter — Oxios WS chunk (StreamChunk) → ChatEvent[] (+ optional passthrough).
//
// Strategy (advisory 2026-07-21): split chunks by concern.
//   • Content streaming (token/reasoning/tool_*/usage/phase/done/error) → ChatEvent[]
//     consumed by StreamProcessor.
//   • Oxios-semantic (model/memory/interview_question/tool_approval/mount_detected)
//     → passthrough as raw StreamChunk; the store's existing reducer arms handle
//     these because each drives a feature that has no ChatEvent equivalent yet
//     (patchAssistantModel, activities, _interviewQuestions, GatedTool flow,
//     mount suggestion UI).
//   • `done` and `error` are DUAL-PATH: emit ChatEvents (reasoning.end +
//     stream.stop) AND passthrough raw chunk. The store's `case 'done'` arm is
//     ~150 lines that handle session_id persistence, mount detection, queue
//     drain, etc.; the `case 'error'` arm creates a dedicated error message
//     with kind-specific copy. Both must run.
//
// See docs/designs/2026-07-21-lobehub-chat-port-design.md §5.1.

import type { StreamChunk } from '@/types'
import type { ChatError } from '@/types/chat'
import type { ChatEvent, TokenUsage } from './ChatEvent'

export interface AdaptedChunk {
  events: ChatEvent[]
  /** Raw chunk forwarded to the store's legacy reducer arms. */
  passthrough?: StreamChunk
}

/**
 * Convert one Oxios WS chunk into 0..N ChatEvents plus an optional passthrough.
 *
 * @param raw        The WS chunk as parsed by `parseChunk`.
 * @param ctx.msgId  Assistant message id this stream belongs to. Adapter does
 *                   not infer it; caller knows which message is "current".
 */
export function adaptChunk(raw: StreamChunk, ctx: { msgId: string }): AdaptedChunk {
  const mid = ctx.msgId
  switch (raw.type) {
    case 'token':
      return raw.content
        ? { events: [{ kind: 'text.delta', messageId: mid, text: raw.content }] }
        : { events: [] }

    case 'reasoning': {
      // Phase B: check for lifecycle subtype markers emitted by the backend.
      if (raw.subtype === 'start') {
        return { events: [{ kind: 'reasoning.start', messageId: mid }] }
      }
      if (raw.subtype === 'end') {
        return { events: [{ kind: 'reasoning.end', messageId: mid }] }
      }
      // Regular reasoning delta (accumulated text).
      const text = raw.content ?? ''
      return text
        ? { events: [{ kind: 'reasoning.delta', messageId: mid, text }] }
        : { events: [] }
    }
    case 'grounding':
      return raw.citations && raw.citations.length > 0
        ? {
            events: [
              {
                kind: 'grounding',
                messageId: mid,
                search: { citations: raw.citations },
              },
            ],
          }
        : { events: [] }

    case 'tool_call_delta':
      return {
        events: [
          {
            kind: 'tool.args_delta',
            messageId: mid,
            toolCallId: raw.tool_call_id ?? '',
            argsDelta: raw.args_delta ?? '',
          },
        ],
      }

    case 'tool_start':
      return {
        events: [
          {
            kind: 'tool.start',
            messageId: mid,
            toolCallId: raw.tool_call_id ?? raw.tool_name ?? cryptoFallbackId(),
            toolName: raw.tool_name ?? 'unknown',
            args: raw.tool_args,
            tabId: raw.tab_id,
          },
        ],
      }

    case 'tool_progress':
      return {
        events: [
          {
            kind: 'tool.progress',
            messageId: mid,
            toolCallId: raw.tool_call_id ?? raw.tool_name ?? '',
            progress: raw.progress,
            tabId: raw.tab_id,
          },
        ],
      }

    case 'tool_end': {
      const err: ChatError | undefined = raw.is_error
        ? {
            type: 'tool_error',
            message:
              typeof raw.tool_result === 'string' ? raw.tool_result : undefined,
            severity: 'error',
          }
        : undefined
      // Backend may send either `tool_result` (structured) or `output_summary`
      // (pre-formatted string). Pass either through; StreamProcessor's
      // summariseResult will format appropriately.
      const result = raw.tool_result ?? raw.output_summary
      return {
        events: [
          {
            kind: 'tool.end',
            messageId: mid,
            toolCallId: raw.tool_call_id ?? raw.tool_name ?? '',
            result,
            durationMs: raw.duration_ms,
            error: err,
          },
        ],
      }
    }

    case 'usage':
      return {
        events: [
          {
            kind: 'usage',
            messageId: mid,
            usage: {
              inputTokens: raw.input_tokens ?? 0,
              outputTokens: raw.output_tokens ?? 0,
            } satisfies TokenUsage,
          },
        ],
      }

    case 'done':
      return {
        events: [
          { kind: 'reasoning.end', messageId: mid },
          {
            kind: 'stream.stop',
            messageId: mid,
            reason: 'done',
            phase: raw.phase,
            evaluationPassed:
              typeof raw.evaluation_passed === 'boolean'
                ? raw.evaluation_passed
                : undefined,
            durationMs: raw.duration_ms,
          },
        ],
        // CRITICAL: passthrough raw chunk — store's `case 'done'` arm
        // performs session_id/project_id persistence, mount_tag/mount_ids
        // detection, completion metadata, queue drain, and tool-only
        // placeholder creation. Without passthrough these die.
        passthrough: raw,
      }

    case 'error':
      return {
        events: [
          { kind: 'reasoning.end', messageId: mid },
          {
            kind: 'stream.stop',
            messageId: mid,
            reason: 'error',
            error: {
              type: 'stream_error',
              message: raw.error,
              severity: 'error',
              retryable: true,
            },
          },
        ],
        // Passthrough — store's `case 'error'` arm creates a dedicated error
        // assistant message with kind-specific copy + suggestion text.
        passthrough: raw,
      }

    case 'phase':
      return {
        events: [
          {
            kind: 'phase',
            messageId: mid,
            phase: raw.phase ?? '',
            evaluationPassed:
              typeof raw.evaluation_passed === 'boolean'
                ? raw.evaluation_passed
                : undefined,
          },
        ],
        passthrough: raw,
      }

    case 'model':
    case 'memory':
    case 'interview':
    case 'tool_approval':
      return { events: [], passthrough: raw }

    default:
      return { events: [] }
  }
}

/** Per-call fallback id when backend omits tool_call_id. Three call sites in
 *  this module need lockstep behavior, so the wrapper is justified. */
function cryptoFallbackId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID()
  }
  return `tc_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
}

export type { ChatEvent, TokenUsage } from './ChatEvent'
