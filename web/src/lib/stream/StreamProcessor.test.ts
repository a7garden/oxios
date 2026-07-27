import { describe, expect, it } from 'vitest'
import type { ReasoningBlock } from '@/types'
import type { ChatMessage } from '@/types'
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
    p.handleEvent({ kind: 'tool.start', messageId: 'm1', toolCallId: 't1', toolName: 'grep', args: { q: 'foo' } })
    p.handleEvent({ kind: 'tool.end', messageId: 'm1', toolCallId: 't1', result: 'match', durationMs: 10 })
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
    p.handleEvent({ kind: 'tool.start', messageId: 'm1', toolCallId: 't1', toolName: 'bash', args: {} })
    p.handleEvent({ kind: 'tool.progress', messageId: 'm1', toolCallId: 't1', progress: 'step 1' })
    p.handleEvent({ kind: 'tool.end', messageId: 'm1', toolCallId: 't1', result: 'ok', durationMs: 5 })

    const tools = blocksOf(p).filter((b) => b.type === 'tool')
    expect(tools).toHaveLength(1)
    expect(tools[0]).toMatchObject({ progress: 'step 1', status: 'success', durationMs: 5 })
  })

  it('appends to the trailing text block, but opens a new one after a tool', () => {
    const p = new StreamProcessor('m1')
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: 'Hello' })
    p.handleEvent({ kind: 'text.delta', messageId: 'm1', text: ' world' })
    p.handleEvent({ kind: 'tool.start', messageId: 'm1', toolCallId: 't1', toolName: 'ls', args: {} })
    p.handleEvent({ kind: 'tool.end', messageId: 'm1', toolCallId: 't1', result: 'x', durationMs: 1 })
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
    p.handleEvent({ kind: 'tool.start', messageId: 'm1', toolCallId: 't1', toolName: 'ls', args: {} })

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
})
