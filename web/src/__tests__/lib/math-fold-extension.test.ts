import { describe, it, expect } from 'vitest'
import { EditorState } from '@codemirror/state'
import { markdown } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { buildMathDecorations } from '@/lib/math-fold-extension'

/** Map line number → MathWidget presence for the decorations of `doc`. */
function widgets(doc: string) {
  const state = EditorState.create({ doc, extensions: [markdown()] })
  ensureSyntaxTree(state, state.doc.length)
  const set = buildMathDecorations(state)
  const byLine = new Map<number, string>()
  const cursor = set.iter()
  while (cursor.value) {
    const name = cursor.value.spec?.widget?.constructor?.name
    if (typeof name === 'string') byLine.set(state.doc.lineAt(cursor.from).number, name)
    cursor.next()
  }
  return { state, byLine }
}

describe('math-fold-extension', () => {
  // Cursor sits at position 0 (line 1); the ±1 active region covers lines 1–2.
  it('folds inline math on an inactive line', () => {
    const { byLine } = widgets('# Title\n\n$x^2$ here')
    expect(byLine.get(3)).toBe('MathWidget')
  })

  it('folds display math ($$…$$)', () => {
    const { byLine } = widgets('# Title\n\n$$\nx = y\n$$')
    expect(byLine.get(3)).toBe('MathWidget')
  })

  it('leaves the source visible on the active line', () => {
    const { byLine } = widgets('$x^2$ here')
    expect(byLine.has(1)).toBe(false)
  })

  it('does not fold currency like $5 and $10', () => {
    const { byLine } = widgets('# Title\n\nCosts $5 and $10 total')
    expect(byLine.size).toBe(0)
  })

  it('does not fold inside an inline code span', () => {
    const { byLine } = widgets('# Title\n\n`$x^2$` in code')
    expect(byLine.size).toBe(0)
  })

  it('keeps multi-line $$…$$ visible when the cursor is inside it', () => {
    // Block spans lines 3–6; cursor sits on line 5 (near the end).
    const state = EditorState.create({
      doc: '# Title\n\n$$\nx = y\nz\n$$\nafter',
      extensions: [markdown()],
      selection: { anchor: 18 }, // start of `z` (line 5)
    })
    ensureSyntaxTree(state, state.doc.length)
    const set = buildMathDecorations(state)
    let folded = false
    const cursor = set.iter()
    while (cursor.value) {
      if (cursor.value.spec?.widget?.constructor?.name === 'MathWidget') folded = true
      cursor.next()
    }
    // Cursor inside the block ⇒ source must stay visible, not be folded.
    expect(folded).toBe(false)
  })
  // Error path: invalid LaTeX must not throw out of toDOM (every keystroke
  // while typing $…$ produces intermediate broken LaTeX).
  it('renders without throwing on invalid LaTeX', () => {
    const { state } = widgets('# Title\n\n$\\notacommand$')
    const set = buildMathDecorations(state)
    const cursor = set.iter()
    let widget: { toDOM: () => unknown } | undefined
    while (cursor.value) {
      const w = cursor.value.spec?.widget
      if (w?.constructor?.name === 'MathWidget') widget = w
      cursor.next()
    }
    expect(widget).toBeDefined()
    // toDOM must not throw; katex throwOnError:false + try/catch fallback.
    const dom = widget!.toDOM()
    expect(dom).toBeInstanceOf(window.HTMLElement)
  })
})
