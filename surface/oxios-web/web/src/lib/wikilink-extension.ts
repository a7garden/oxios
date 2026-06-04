/**
 * Wiki-link linkify extension for CodeMirror 6.
 *
 * Replaces HyperMD's `hmdReadLink` + `hmdClick` behavior for `[[X]]`:
 *  - `[[PageName]]` and `[[PageName|alias]]` are rendered as `<a>` widgets
 *  - Clicking the widget dispatches `knowledge:open-file` (the same
 *    CustomEvent the `linkClickHandler` listens for, dispatched from
 *    a separate useEffect in `markdown-editor.tsx`)
 *  - Active-line behavior: on the cursor's own line (and its
 *    ±1 neighbors — the same active region used by the token-hide
 *    extension), the `[[X]]` source is left visible so the user can
 *    edit the link. This mirrors HyperMD's "you can edit what you
 *    see" rule.
 *
 * Interaction with token-hide:
 *  - The `[[` and `]]` brackets are NOT in `MARKUP_NODE_NAMES` of the
 *    token-hide extension, so they are not hidden there. Good — the
 *    brackets are part of OUR syntax, not standard markdown.
 *  - On inactive lines, the entire `[[X]]` range is replaced by the
 *    widget. The widget renders the target (or alias) as a clickable
 *    link, hiding the source entirely.
 *  - On active lines, no decoration is applied; the user sees and
 *    edits the raw `[[X]]` text.
 *
 * Round-trip:
 *  - The underlying text is never modified — the widget is purely
 *    visual. Saving the document yields exactly the same markdown
 *    source the user typed. This is the same round-trip invariant
 *    HyperMD provided.
 */
import {
  Decoration,
  EditorView,
  MatchDecorator,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view'

/**
 * Match `[[PageName]]` or `[[PageName|alias]]`.
 * The target is the part before `|` (or the whole inner text).
 * The display label is the part after `|` (or the target if no alias).
 */
const WIKILINK_RE = /\[\[([^\[\]\n|]+)(?:\|([^\[\]\n]+))?\]\]/g

class WikiLinkWidget extends WidgetType {
  constructor(
    readonly target: string,
    readonly display: string,
  ) {
    super()
  }

  toDOM(): HTMLElement {
    const a = document.createElement('a')
    a.className = 'cm-wikilink'
    a.setAttribute('href', '#' + this.target)
    a.setAttribute('data-wikilink-target', this.target)
    a.textContent = this.display
    a.style.cssText =
      'color: #79c0ff; text-decoration: underline dotted; cursor: pointer;'
    a.addEventListener('click', (e) => {
      e.preventDefault()
      // Reuse the same CustomEvent the linkClickHandler listens for.
      // This keeps open-file handling in one place.
      document.dispatchEvent(
        new CustomEvent('knowledge:open-file', { detail: { path: this.target } }),
      )
    })
    return a
  }

  ignoreEvent() {
    return false
  }

  eq(other: WikiLinkWidget): boolean {
    return other instanceof WikiLinkWidget && other.target === this.target && other.display === this.display
  }
}

/**
 * Build a decoration set for the visible viewport.
 *
 * On the cursor's own line and its ±1 neighbors we leave the
 * underlying `[[X]]` text visible. On every other line the entire
 * range is replaced by a widget that renders the target/alias as
 * a clickable link.
 */
function buildDecorations(view: EditorView): DecorationSet {
  const builder: Decoration[] = []
  const cursorLine = view.state.doc.lineAt(view.state.selection.main.head).number
  const minActive = Math.max(1, cursorLine - 1)
  const maxActive = Math.min(view.state.doc.lines, cursorLine + 1)

  // Walk the visible text and match wikilinks.
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to)
    WIKILINK_RE.lastIndex = 0
    let m: RegExpExecArray | null
    while ((m = WIKILINK_RE.exec(text)) !== null) {
      const matchStart = from + m.index
      const matchEnd = matchStart + m[0].length
      const startLine = view.state.doc.lineAt(matchStart).number
      const endLine = view.state.doc.lineAt(matchEnd).number
      const crossesActive = startLine <= maxActive && endLine >= minActive
      if (crossesActive) continue
      const target = m[1].trim()
      const display = (m[2] ?? m[1]).trim()
      if (!target) continue
      builder.push(
        Decoration.replace({ widget: new WikiLinkWidget(target, display) }).range(
          matchStart,
          matchEnd,
        ),
      )
    }
  }

  // CM6 requires sorted, non-overlapping ranges.
  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

export const wikilinkExtension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view)
    }
    update(update: ViewUpdate) {
      if (
        update.docChanged ||
        update.selectionSet ||
        update.viewportChanged
      ) {
        this.decorations = buildDecorations(update.view)
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
)
