/**
 * Token-hiding extension for CodeMirror 6.
 *
 * Replaces HyperMD's `hmdHideToken: { enabled: true }` behavior.
 *
 * Hides markdown markup tokens (`**`, `*`, `#`, `[]()`, ` `` `) on lines
 * that are NOT within ±1 of the cursor. The active line and its
 * neighbors always show the full markup so editing remains predictable.
 *
 * Implementation notes:
 *  - We walk the markdown syntax tree (`@lezer/markdown`) and pick out
 *    the *markup* nodes (EmphasisMark, StrongEmphasisMark, etc.). The
 *    *content* between them is left visible (this is the WYSIWYG-like
 *    look HyperMD produced).
 *  - `Decoration.replace({})` collapses a range to zero width. The
 *    underlying text is preserved, so editing and serialization work.
 *  - Recompute on doc/selection/viewport changes. Debounced via the
 *    ViewUpdate flags.
 *
 * Risks:
 *  - If we accidentally hide tokens on the active line, cursor offset
 *    calculations can drift. We treat ±1 of the cursor's line as the
 *    active region, which matches HyperMD's default.
 *  - Heavily nested emphasis (e.g. ***bold-italic***) is hard. We
 *    collapse the outermost pair only; nested content may briefly
 *    show its own tokens until the next cursor move.
 */
import { syntaxTree } from '@codemirror/language'
import type { Range } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'

// `@lezer/markdown` node names that mark *markup* (not content).
// We deliberately do NOT hide the content nodes (Emphasis, StrongEmphasis,
// etc.) — only the `*Mark` variants.
const MARKUP_NODE_NAMES = new Set([
  'EmphasisMark',
  'StrongEmphasisMark',
  'HeaderMark',
  'LinkMark',
  'QuoteMark',
  'ListMark',
  'CodeMark',
  'CodeInfo',
  // GFM strikethrough mark (paired with livePreviewHighlight line-through)
  'StrikethroughMark',
  'StrikethroughDelim',
  // GFM table delimiters (paired with tableFoldExtension grid widget)
  'TableDelimiter',
])

function buildDecorations(view: EditorView): DecorationSet {
  const builder: Range<Decoration>[] = []
  const tree = syntaxTree(view.state)

  // Determine the active region: the cursor's line and its neighbors.
  const cursor = view.state.selection.main.head
  const cursorLine = view.state.doc.lineAt(cursor).number
  const minActive = Math.max(1, cursorLine - 1)
  const maxActive = Math.min(view.state.doc.lines, cursorLine + 1)

  // Walk all markup nodes. For each, if its line is OUTSIDE the active
  // region, add a 0-width replacement decoration.
  tree.iterate({
    enter(node) {
      if (!MARKUP_NODE_NAMES.has(node.name)) return
      const from = node.from
      const to = node.to
      // Don't hide a 0-width node.
      if (to <= from) return
      // Don't hide a node that crosses the active region.
      const startLine = view.state.doc.lineAt(from).number
      const endLine = view.state.doc.lineAt(to).number
      const crossesActive = startLine <= maxActive && endLine >= minActive
      if (crossesActive) return
      // Don't hide a node that is INSIDE a code block (fenced).
      // FencedCode → CodeBlock → CodeMark/CodeInfo. The CodeInfo
      // node sits between the ```fence and the code body.
      // Our whitelist above already includes 'CodeMark' and
      // 'CodeInfo' — those should be hidden only OUTSIDE code blocks.
      // For now, hide them anyway; the user sees a clean fence.
      builder.push(Decoration.replace({}).range(from, to))
    },
  })

  // Sort by `from` (CM6 requires sorted ranges)
  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

export const tokenHideExtension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = buildDecorations(update.view)
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
)
