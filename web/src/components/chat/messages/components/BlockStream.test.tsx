import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ChatBlock } from '@/types'
import { BlockStream } from './BlockStream'

describe('BlockStream', () => {
  it('renders tool before text (execution order, not inverted)', () => {
    const blocks: ChatBlock[] = [
      {
        type: 'tool',
        id: 't1',
        identifier: 'kernel',
        apiName: 'grep',
        arguments: {},
        status: 'success',
      },
      { type: 'text', id: 'x1', text: 'Final answer' },
    ]
    const { container } = render(<BlockStream blocks={blocks} messageId="m1" />)
    const text = container.textContent ?? ''
    expect(text).toContain('grep')
    expect(text).toContain('Final answer')
    // The tool must appear BEFORE the answer — the core fix over the old
    // "tool list below the answer" categorized layout.
    expect(text.indexOf('grep')).toBeLessThan(text.indexOf('Final answer'))
  })

  it('renders a lone text block (simple Q&A turn)', () => {
    const blocks: ChatBlock[] = [{ type: 'text', id: 'x1', text: 'Just an answer' }]
    const { container } = render(<BlockStream blocks={blocks} messageId="m1" />)
    expect(container.textContent).toContain('Just an answer')
  })
})
