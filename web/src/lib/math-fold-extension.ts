/**
 * Math fold extension for CodeMirror 6.
 *
 * Replaces HyperMD's `fold-math`: inline `$…$` and display `$$…$$` are
 * rendered as KaTeX when the cursor is NOT on that line (±1 active region),
 * so editing the LaTeX source still works. KaTeX rendering reuses the same
 * renderer files.md's editor shipped (see lib/katex-renderer.ts).
 *
 * Detection is regex-based (there is no `$` node in @lezer/markdown), so —
 * like emoji-fold — the whole delimited range is replaced by the widget on
 * inactive lines; no separate token-hiding is needed. To avoid folding
 * currency (`$5 and $10`), the inline form requires a non-space character
 * immediately inside each `$`. Matches inside code spans / fenced blocks are
 * skipped.
 *
 * Round-trip safe: the widget is purely visual; the source `$…$` is
 * preserved.
 */
import { syntaxTree } from '@codemirror/language'
import type { EditorState, Range } from '@codemirror/state'
import { StateField } from '@codemirror/state'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import { Decoration, type DecorationSet, EditorView, WidgetType } from '@codemirror/view'

// Display `$$…$$` (group 1) takes precedence; inline `$…$` (group 2) requires
// a non-space char at each end to dodge currency like "$5 and $10".
const MATH_RE = /\$\$([\s\S]+?)\$\$|\$([^\s$](?:[^$\n]*[^\s$])?)\$/g

class MathWidget extends WidgetType {
  constructor(
    readonly expr: string,
    readonly display: boolean,
  ) {
    super()
  }
  eq(other: MathWidget) {
    return other.expr === this.expr && other.display === this.display
  }
  toDOM() {
    const span = document.createElement('span')
    span.className = 'ox-md-math'
    try {
      span.innerHTML = katex.renderToString(this.expr, {
        displayMode: this.display,
        throwOnError: false,
      })
    } catch {
      span.textContent = this.expr
    }
    return span
  }
  ignoreEvent() {
    return false
  }
}

/** Is `pos` inside a code span or fenced block? */
function inCode(state: EditorState, pos: number): boolean {
  let node = syntaxTree(state).resolveInner(pos)
  while (node) {
    if (
      node.name === 'FencedCode' ||
      node.name === 'CodeBlock' ||
      node.name === 'InlineCode' ||
      node.name === 'CodeText'
    ) {
      return true
    }
    const parent = node.parent
    if (!parent || parent === node) break
    node = parent
  }
  return false
}

export function buildMathDecorations(state: EditorState): DecorationSet {
  const builder: Range<Decoration>[] = []
  const { doc } = state
  const cursorLine = doc.lineAt(state.selection.main.head).number
  const text = doc.toString()
  MATH_RE.lastIndex = 0
  let m: RegExpExecArray | null
  for (m = MATH_RE.exec(text); m !== null; m = MATH_RE.exec(text)) {
    const display = m[1] !== undefined
    const expr = display ? m[1] : m[2]
    if (!expr) continue
    const from = m.index
    const to = from + m[0].length
    // Active region = ±1 of ANY line the match spans (display `$$…$$` can
    // be multi-line; checking only the start line would fold the block while
    // the cursor sits inside it near the end).
    const fromLine = doc.lineAt(from).number
    const toLine = doc.lineAt(to).number
    if (cursorLine >= fromLine - 1 && cursorLine <= toLine + 1) continue
    if (inCode(state, from)) continue
    builder.push(Decoration.replace({ widget: new MathWidget(expr, display) }).range(from, to))
  }
  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

// StateField — not a ViewPlugin — because display `$$…$$` blocks can
// span line breaks, and CodeMirror 6 only permits line-break-spanning
// replace decorations from state-derived sources.
export const mathFoldExtension = StateField.define<DecorationSet>({
  create(state) {
    return buildMathDecorations(state)
  },
  update(deco, tr) {
    return tr.docChanged || tr.selection != null
      ? buildMathDecorations(tr.state)
      : deco.map(tr.changes)
  },
  provide: (f) => EditorView.decorations.from(f),
})
