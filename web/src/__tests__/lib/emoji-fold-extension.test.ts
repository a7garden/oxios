import { markdown } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { buildEmojiDecorations } from '@/lib/emoji-fold-extension'

/** Map line number → EmojiWidget presence for the decorations of `doc`. */
function widgets(doc: string): Map<number, string> {
  const state = EditorState.create({ doc, extensions: [markdown()] })
  ensureSyntaxTree(state, state.doc.length)
  const set = buildEmojiDecorations(state)
  const byLine = new Map<number, string>()
  const cursor = set.iter()
  while (cursor.value) {
    const name = cursor.value.spec?.widget?.constructor?.name
    if (typeof name === 'string') byLine.set(state.doc.lineAt(cursor.from).number, name)
    cursor.next()
  }
  return byLine
}

describe('emoji-fold-extension', () => {
  // Cursor sits at position 0 (line 1); the ±1 active region covers lines 1–2.
  it('folds a known shortcode on an inactive line', () => {
    const m = widgets('# Title\n\n:smile: hi')
    expect(m.get(3)).toBe('EmojiWidget')
  })

  it('leaves the source visible on the active line', () => {
    const m = widgets(':smile: hi')
    expect(m.has(1)).toBe(false)
  })

  it('ignores unknown shortcodes', () => {
    const m = widgets('# Title\n\n:totally_not_real: hi')
    expect(m.size).toBe(0)
  })

  it('does not fold inside an inline code span', () => {
    const m = widgets('# Title\n\n`:smile:` in code')
    expect(m.size).toBe(0)
  })
})
