import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { Strikethrough, Table, TaskList } from '@lezer/markdown'
import { describe, expect, it } from 'vitest'
import { mathFoldExtension } from '@/lib/math-fold-extension'
import { buildMermaidDecorations, mermaidExtension } from '@/lib/mermaid-extension'
import { tableFoldExtension } from '@/lib/table-fold-extension'

const md = () => markdown({ base: markdownLanguage, extensions: [Strikethrough, Table, TaskList] })

/**
 * Regression guard for the CodeMirror 6 constraint:
 *
 *   "Decorations that replace line breaks may not be specified via plugins"
 *
 * A replace decoration whose range crosses a `\n` is only legal when it is
 * state-derived (StateField). table/math/mermaid all replace multi-line
 * blocks, so their extensions must be StateFields. These tests mount a REAL
 * EditorView with a multi-line block, force the syntax tree to parse, and
 * rebuild the decorations — which throws if anyone reverts a fold extension
 * to a ViewPlugin.
 */
describe('fold extensions — line-break replace decorations stay valid', () => {
  it('renders a multi-line GFM table widget without throwing', () => {
    const doc = '# T\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |'
    const state = EditorState.create({ doc, extensions: [md(), tableFoldExtension] })
    ensureSyntaxTree(state, state.doc.length)
    const host = document.createElement('div')
    document.body.appendChild(host)
    const view = new EditorView({ state })
    host.appendChild(view.dom)
    // Cursor far from the table → inactive region → widget replaces it.
    // This dispatch also rebuilds the StateField now that the tree parsed.
    view.dispatch({ selection: { anchor: 0 } })
    expect(view.dom.querySelector('.ox-md-table')).toBeTruthy()
    view.destroy()
    host.remove()
  })

  it('renders a multi-line display-math widget without throwing', () => {
    const doc = '# T\n\n$$\na = b + c\n$$'
    const state = EditorState.create({ doc, extensions: [md(), mathFoldExtension] })
    ensureSyntaxTree(state, state.doc.length)
    const host = document.createElement('div')
    document.body.appendChild(host)
    const view = new EditorView({ state })
    host.appendChild(view.dom)
    view.dispatch({ selection: { anchor: 0 } })
    expect(view.dom.querySelector('.ox-md-math')).toBeTruthy()
    view.destroy()
    host.remove()
  })

  it('buildMermaidDecorations emits a MermaidWidget for a fenced block', () => {
    const state = EditorState.create({
      doc: '# T\n\n```mermaid\ngraph LR\nA-->B\n```\n',
      extensions: [md()],
    })
    ensureSyntaxTree(state, state.doc.length)
    const set = buildMermaidDecorations(state)
    let found = false
    const cur = set.iter()
    while (cur.value) {
      if (cur.value.spec?.widget?.constructor?.name === 'MermaidWidget') found = true
      cur.next()
    }
    expect(found).toBe(true)
  })

  it('mermaidExtension StateField creates without throwing', () => {
    expect(() =>
      EditorState.create({
        doc: '# T\n\n```mermaid\ngraph LR\nA-->B\n```\n',
        extensions: [md(), mermaidExtension],
      }),
    ).not.toThrow()
  })
})
