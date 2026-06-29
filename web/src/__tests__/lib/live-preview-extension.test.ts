import { describe, it, expect, vi } from 'vitest'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree, syntaxTree } from '@codemirror/language'
import { buildDecorations } from '@/lib/live-preview-extension'
import { Strikethrough, Table, TaskList } from '@lezer/markdown'

/**
 * Parse `doc` (GFM-enabled, matching the editor config) and collect, per line:
 * the line-class decoration (keyed in `lines`) and the widget type name
 * (keyed in `widgets`). `ensureSyntaxTree` forces a synchronous full parse so
 * nodes exist before `buildDecorations` walks the tree.
 */
function inspect(doc: string) {
  const state = EditorState.create({
    doc,
    extensions: [markdown({ base: markdownLanguage, extensions: [Strikethrough, Table, TaskList] })],
  })
  // Canary: prove the parse completed synchronously before we walk it.
  expect(ensureSyntaxTree(state, state.doc.length)).toBeTruthy()
  const set = buildDecorations(state)
  const lines = new Map<number, string>()
  const widgets = new Map<number, string>()
  const cursor = set.iter()
  while (cursor.value) {
    // value.spec is typed `any` by @codemirror/view; narrow, don't cast.
    const spec = cursor.value.spec
    const lineNo = state.doc.lineAt(cursor.from).number
    const cls = spec?.class
    if (typeof cls === 'string') lines.set(lineNo, cls)
    const widgetName = spec?.widget?.constructor?.name
    if (typeof widgetName === 'string') widgets.set(lineNo, widgetName)
    cursor.next()
  }
  return { lines, widgets }
}

describe('live-preview-extension buildDecorations — headings', () => {
  it('tags each heading line with its per-level class', () => {
    const { lines } = inspect('# Title\n\n## Section\n\n### Sub\n\nplain paragraph')
    expect(lines.get(1)).toBe('ox-md-h1 ox-md-first')
    expect(lines.get(3)).toBe('ox-md-h2')
    expect(lines.get(5)).toBe('ox-md-h3')
    expect(lines.has(7)).toBe(false) // body untouched
  })

  it('marks only the line-1 heading as the document title', () => {
    const { lines } = inspect('# First\n\n# Second')
    expect(lines.get(1)).toBe('ox-md-h1 ox-md-first')
    expect(lines.get(3)).toBe('ox-md-h1')
  })

  it('distinguishes all six heading levels', () => {
    const { lines } = inspect('# a\n## b\n### c\n#### d\n##### e\n###### f')
    expect(lines.get(1)).toBe('ox-md-h1 ox-md-first')
    expect(lines.get(2)).toBe('ox-md-h2')
    expect(lines.get(3)).toBe('ox-md-h3')
    expect(lines.get(4)).toBe('ox-md-h4')
    expect(lines.get(5)).toBe('ox-md-h5')
    expect(lines.get(6)).toBe('ox-md-h6')
  })
})

describe('live-preview-extension buildDecorations — block elements', () => {
  it('replaces a horizontal rule with the HrWidget', () => {
    const { widgets } = inspect('# Title\n\n---\n\ntext')
    expect(widgets.get(3)).toBe('HrWidget')
  })

  it('styles blockquote lines with ox-md-quote', () => {
    const { lines } = inspect('# Title\n\n> quoted text')
    expect(lines.get(3)).toBe('ox-md-quote')
  })

  it('styles fenced-code lines with ox-md-codeblock', () => {
    const { lines } = inspect('# Title\n\n```\ncode\n```')
    expect(lines.get(3)).toBe('ox-md-codeblock')
    expect(lines.get(5)).toBe('ox-md-codeblock')
  })

  it('replaces task markers with TaskWidget', () => {
    const { widgets } = inspect('# Title\n\n- [ ] todo\n- [x] done')
    expect(widgets.get(3)).toBe('TaskWidget')
    expect(widgets.get(4)).toBe('TaskWidget')
  })
  it('toggles the marker on checkbox click', () => {
    const state = EditorState.create({
      doc: '# Title\n\n- [ ] todo',
      extensions: [markdown({ base: markdownLanguage, extensions: [Strikethrough, Table, TaskList] })],
    })
    ensureSyntaxTree(state, state.doc.length)
    const set = buildDecorations(state)
    let widget: { toDOM: () => HTMLInputElement } | undefined
    const cursor = set.iter()
    while (cursor.value) {
      const w = cursor.value.spec?.widget
      if (w?.constructor?.name === 'TaskWidget') widget = w
      cursor.next()
    }
    expect(widget).toBeTruthy()
    let inserted: string | undefined
    const findSpy = vi.spyOn(EditorView, 'findFromDOM').mockReturnValue({
      dispatch: (spec: { changes?: { insert?: string } }) => {
        inserted = spec.changes?.insert
      },
    } as unknown as EditorView)
    widget!.toDOM().click()
    expect(inserted).toBe('[x]') // [ ] → [x]
    findSpy.mockRestore()
  })

  it('leaves plain paragraphs undecorated', () => {
    const { lines, widgets } = inspect('first paragraph\n\nsecond paragraph')
    expect(lines.size).toBe(0)
    expect(widgets.size).toBe(0)
  })
})

/** Does the GFM-enabled parse of `doc` contain a node named `name`? */
function hasNode(doc: string, name: string): boolean {
  const state = EditorState.create({
    doc,
    extensions: [markdown({ base: markdownLanguage, extensions: [Strikethrough, Table, TaskList] })],
  })
  ensureSyntaxTree(state, state.doc.length)
  let found = false
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === name) found = true
    },
  })
  return found
}

describe('GFM parser (markdown() extensions)', () => {
  it('parses strikethrough', () => {
    expect(hasNode('~~struck~~', 'Strikethrough')).toBe(true)
  })

  it('parses task list markers', () => {
    expect(hasNode('- [ ] todo\n- [x] done', 'TaskMarker')).toBe(true)
  })

  it('parses tables', () => {
    expect(hasNode('| a | b |\n| --- | --- |\n| 1 | 2 |', 'Table')).toBe(true)
  })
})
