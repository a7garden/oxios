import { describe, expect, it } from 'vitest'
import type { ChatMessage, ReasoningBlock } from '@/types'
import { StreamProcessor } from './StreamProcessor'

const base = { id: 'm1', role: 'assistant', content: '' } as ChatMessage

function blocksOf(p: StreamProcessor) {
  return p.materialize(base).blocks!
}

describe('StreamProcessor — block-stream timeline', () => {
  it('interleaves reasoning, tool, and text in arrival order', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'need to grep' })
    p.handleEvent({
      kind: 'tool.start',
      messageId: 'm1',
      toolCallId: 't1',
      toolName: 'grep',
      args: { q: 'foo' },
    })
    p.handleEvent({
      kind: 'tool.end',
      messageId: 'm1',
      toolCallId: 't1',
      result: 'match',
      durationMs: 10,
    })
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'now read' })
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: 'Answer' })

    const blocks = blocksOf(p)
    expect(blocks.map((b) => b.type)).toEqual(['reasoning', 'tool', 'reasoning', 'text'])
    expect(blocks[0]).toMatchObject({ type: 'reasoning', text: 'need to grep', status: 'done' })
    expect(blocks[1]).toMatchObject({ type: 'tool', apiName: 'grep', status: 'success' })
    expect(blocks[2]).toMatchObject({ type: 'reasoning', text: 'now read', status: 'done' })
    expect(blocks[3]).toMatchObject({ type: 'text', text: 'Answer' })
  })

  it('merges tool progress/end into the same tool block by id', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({
      kind: 'tool.start',
      messageId: 'm1',
      toolCallId: 't1',
      toolName: 'bash',
      args: {},
    })
    p.handleEvent({ kind: 'tool.progress', messageId: 'm1', toolCallId: 't1', progress: 'step 1' })
    p.handleEvent({
      kind: 'tool.end',
      messageId: 'm1',
      toolCallId: 't1',
      result: 'ok',
      durationMs: 5,
    })

    const tools = blocksOf(p).filter((b) => b.type === 'tool')
    expect(tools).toHaveLength(1)
    expect(tools[0]).toMatchObject({ progress: 'step 1', status: 'success', durationMs: 5 })
  })

  it('appends to the trailing text block, but opens a new one after a tool', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: 'Hello' })
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: ' world' })
    p.handleEvent({
      kind: 'tool.start',
      messageId: 'm1',
      toolCallId: 't1',
      toolName: 'ls',
      args: {},
    })
    p.handleEvent({
      kind: 'tool.end',
      messageId: 'm1',
      toolCallId: 't1',
      result: 'x',
      durationMs: 1,
    })
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: 'After' })

    const texts = blocksOf(p).filter((b) => b.type === 'text')
    expect(texts).toHaveLength(2)
    expect(texts[0]).toMatchObject({ text: 'Hello world' })
    expect(texts[1]).toMatchObject({ text: 'After' })
  })

  it('closes an open reasoning span when a tool starts', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'thinking…' })
    p.handleEvent({
      kind: 'tool.start',
      messageId: 'm1',
      toolCallId: 't1',
      toolName: 'ls',
      args: {},
    })

    expect(blocksOf(p)[0]).toMatchObject({ type: 'reasoning', status: 'done' })
  })

  it('caps total reasoning text at the budget', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'x'.repeat(20_000) })

    const reasoning = blocksOf(p).filter((b): b is ReasoningBlock => b.type === 'reasoning')
    expect(reasoning[0]!.text.length).toBeLessThanOrEqual(16 * 1024)
  })

  it('marks every block done on stream.stop', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'hmm' })
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: 'A' })
    p.handleEvent({ kind: 'stream.stop', messageId: 'm1', reason: 'done' })

    const blocks = blocksOf(p)
    const openReasoning = blocks.filter((b) => b.type === 'reasoning' && b.status === 'streaming')
    const openText = blocks.filter((b) => b.type === 'text' && b.streaming)
    expect(openReasoning).toHaveLength(0)
    expect(openText).toHaveLength(0)
  })

  it('opens a reasoning block with source on the first delta and preserves it on append', () => {
    // A no-subtype reasoning delta carrying `source: 'compaction'` opens the
    // reasoning block with that label, and subsequent deltas (even without a
    // matching source on the event) keep the block's source stable.
    const p = new StreamProcessor('m1')
    p.handleEvent({
      kind: 'reasoning.delta',
      messageId: 'm1',
      text: 'compaction complete',
      source: 'compaction',
    })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: ' (kept)' })
    p.handleEvent({
      kind: 'reasoning.delta',
      messageId: 'm1',
      text: ' more',
      source: 'thinking',
    })

    const reasoning = blocksOf(p).filter((b): b is ReasoningBlock => b.type === 'reasoning')
    expect(reasoning).toHaveLength(1)
    expect(reasoning[0]).toMatchObject({
      source: 'compaction',
      text: 'compaction complete (kept) more',
    })
  })

  it('applies source when the first reasoning delta opens its own block', () => {
    // appendReasoning must accept `source` and stamp it on the freshly opened
    // block when no prior reasoning span is open — this is the path that the
    // adapter takes for a no-subtype compaction chunk.
    const p = new StreamProcessor('m1')
    p.handleEvent({
      kind: 'reasoning.delta',
      messageId: 'm1',
      text: 'first',
      source: 'thinking',
    })

    const reasoning = blocksOf(p).filter((b): b is ReasoningBlock => b.type === 'reasoning')
    expect(reasoning[0]).toMatchObject({ source: 'thinking', text: 'first' })
  })

  it('leaves source absent when neither the first delta nor its successors carry one', () => {
    // No-subtype reasoning deltas without `source` should still flow through;
    // the block must simply omit the optional field.
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'plain' })

    const reasoning = blocksOf(p).filter((b): b is ReasoningBlock => b.type === 'reasoning')
    expect(reasoning[0]).toMatchObject({ text: 'plain' })
    expect((reasoning[0] as ReasoningBlock).source).toBeUndefined()
  })
  // ── Reasoning coalescing (turn-boundary invariant) ──
  // A single uninterrupted thinking run must stay ONE block even when the
  // runtime emits multiple start/end marker pairs (or a delta after an end).
  // Only a tool/text block may split it. Mirrors the backend's per-position
  // segment coalescing (agent_runtime.rs).
  it('merges adjacent reasoning spans with no tool between into one block', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'Thinking ' })
    p.handleEvent({ kind: 'reasoning.end', messageId: 'm1' })
    // Second span, NO tool between — before the fix this spawned a sibling
    // "Thought" card. It must reopen and append to the same block.
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'more' })
    p.handleEvent({ kind: 'reasoning.end', messageId: 'm1' })

    const reasoning = blocksOf(p).filter((b) => b.type === 'reasoning')
    expect(reasoning).toHaveLength(1)
    expect(reasoning[0]).toMatchObject({ type: 'reasoning', text: 'Thinking more', status: 'done' })
  })

  it('reopens a closed reasoning span when a delta arrives with no fresh start', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'part1 ' })
    p.handleEvent({ kind: 'reasoning.end', messageId: 'm1' })
    // Delta after end, no new start — must reopen, not spawn a sibling.
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'part2' })

    const reasoning = blocksOf(p).filter((b) => b.type === 'reasoning')
    expect(reasoning).toHaveLength(1)
    expect(reasoning[0]).toMatchObject({ text: 'part1 part2' })
  })

  it('does not merge reasoning across a tool (flow order preserved)', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'before' })
    p.handleEvent({ kind: 'reasoning.end', messageId: 'm1' })
    p.handleEvent({
      kind: 'tool.start',
      messageId: 'm1',
      toolCallId: 't1',
      toolName: 'ls',
      args: {},
    })
    p.handleEvent({
      kind: 'tool.end',
      messageId: 'm1',
      toolCallId: 't1',
      result: 'ok',
      durationMs: 1,
    })
    p.handleEvent({ kind: 'reasoning.start', messageId: 'm1' })
    p.handleEvent({ kind: 'reasoning.delta', messageId: 'm1', text: 'after' })
    p.handleEvent({ kind: 'reasoning.end', messageId: 'm1' })

    const blocks = blocksOf(p)
    expect(blocks.map((b) => b.type)).toEqual(['reasoning', 'tool', 'reasoning'])
    expect(blocks[0]).toMatchObject({ text: 'before' })
    expect(blocks[2]).toMatchObject({ text: 'after' })
  })
})
