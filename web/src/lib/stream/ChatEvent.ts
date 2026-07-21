// ChatEvent — unified event shape consumed by StreamProcessor and chat components.
//
// Oxios WS chunks (StreamChunk) are converted to ChatEvents by adapter.ts.
// Only "content streaming" chunks become ChatEvents; Oxios-semantic chunks
// (model, memory, interview_question, tool_approval, mount_detected) are
// passed through as raw StreamChunk for the store's existing reducer arms.
//
// Phase 1 of LobeHub port (2026-07-21). See
// docs/designs/2026-07-21-lobehub-chat-port-design.md §5.1, §6.2.

import type {
  ChatError,
  ChatFileChunk,
  GroundingSearch,
} from '@/types/chat'

/** Token usage for a single assistant message. */
export interface TokenUsage {
  inputTokens: number
  outputTokens: number
  costUsd?: number
}

/**
 * Discriminated union of all content-streaming events.
 * One WS chunk may produce 0..N events (e.g. `done` → stream.stop + reasoning.end).
 */
export type ChatEvent =
  | { kind: 'text.delta'; messageId: string; text: string }
  | { kind: 'reasoning.start'; messageId: string }
  | { kind: 'reasoning.delta'; messageId: string; text: string }
  | { kind: 'reasoning.end'; messageId: string; durationMs?: number }
  | {
      kind: 'tool.start'
      messageId: string
      toolCallId: string
      toolName: string
      args?: unknown
      tabId?: string
    }
  | {
      kind: 'tool.progress'
      messageId: string
      toolCallId: string
      progress?: string
      tabId?: string
    }
  | {
      kind: 'tool.end'
      messageId: string
      toolCallId: string
      result?: unknown
      durationMs?: number
      error?: ChatError
    }
  | { kind: 'grounding'; messageId: string; search: GroundingSearch }
  | { kind: 'file_chunks'; messageId: string; chunks: ChatFileChunk[] }
  | { kind: 'usage'; messageId: string; usage: TokenUsage }
  | { kind: 'phase'; messageId?: string; phase: string; evaluationPassed?: boolean }
  | {
      kind: 'stream.stop'
      messageId?: string
      reason: 'done' | 'aborted' | 'error'
      error?: ChatError
      phase?: string
      evaluationPassed?: boolean
      durationMs?: number
    }
