import { describe, it, expect } from 'vitest'
import { EditorState } from '@codemirror/state'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { Table } from '@lezer/markdown'
import { buildTableDecorations } from '@/lib/table-fold-extension'

/** Run buildTableDecorations for `doc` and return the first TableWidget found (or undefined). */
function widget(doc: string) {
  const state = EditorState.create({
    doc,
    extensions: [markdown({ base: markdownLanguage, extensions: [Table] })],
  })
  ensureSyntaxTree(state, state.doc.length)
  const set = buildTableDecorations(state)
  const cursor = set.iter()
  while (cursor.value) {
    const w = cursor.value.spec?.widget
    if (w?.constructor?.name === 'TableWidget') return w
    cursor.next()
  }
  return undefined
}

function inspect(doc: string) {
  const state = EditorState.create({
    doc,
    extensions: [markdown({ base: markdownLanguage, extensions: [Table] })],
  })
  ensureSyntaxTree(state, state.doc.length)
  const set = buildTableDecorations(state)
  const out: Array<{ line: number; kind: string }> = []
  const cursor = set.iter()
  while (cursor.value) {
    const lineNo = state.doc.lineAt(cursor.from).number
    const spec = cursor.value.spec
    if (typeof spec?.class === 'string') {
      out.push({ line: lineNo, kind: `class:${spec.class}` })
    }
    if (spec?.widget?.constructor?.name === 'TableWidget') {
      out.push({ line: lineNo, kind: 'widget:TableWidget' })
    }
    cursor.next()
  }
  return out
}

describe('table-fold-extension', () => {
  // Cursor sits at position 0 (line 1); the ±1 active region covers lines 1–2.
  it('folds a 3-column table on an inactive line', () => {
    const out = inspect('# Title\n\n| a | b | c |\n| --- | --- | --- |\n| 1 | 2 | 3 |')
    const kinds = out.map((o) => o.kind)
    expect(kinds).toContain('widget:TableWidget')
  })

  it('parses columns and rows from the source', () => {
    const w = widget('# Title\n\n| a | b | c |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |') as
      | { headers: string[]; rows: string[][]; alignments: string[] }
      | undefined
    expect(w).toBeTruthy()
    expect(w!.headers).toEqual(['a', 'b', 'c'])
    expect(w!.rows).toEqual([
      ['1', '2', '3'],
      ['4', '5', '6'],
    ])
    expect(w!.alignments).toEqual(['left', 'left', 'left'])
  })

  it('reads alignments from :-- / :-: / --: delimiters', () => {
    const w = widget(
      '# Title\n\n| L | C | R |\n| :--- | :---: | ---: |\n| a | b | c |',
    ) as { alignments: string[] } | undefined
    expect(w!.alignments).toEqual(['left', 'center', 'right'])
  })

  it('renders without throwing', () => {
    const w = widget('# Title\n\n| a | b |\n| --- | --- |\n| 1 | 2 |')
    const dom = (w as { toDOM: () => HTMLElement }).toDOM()
    expect(dom).toBeInstanceOf(window.HTMLElement)
    expect(dom.querySelector('table')).toBeTruthy()
    expect(dom.querySelectorAll('thead th').length).toBe(2)
  })

  it('keeps the source visible on the active line', () => {
    const out = inspect('| a | b |\n| --- | --- |\n| 1 | 2 |')
    expect(out.find((o) => o.kind === 'widget:TableWidget')).toBeUndefined()
  })

  it('falls back gracefully when the delimiter row is missing', () => {
    const out = inspect(
      '# Title\n\n| a | b |\n| 1 | 2 |\nno delimiter row here',
    )
    expect(out.find((o) => o.kind === 'widget:TableWidget')).toBeUndefined()
  })
})
