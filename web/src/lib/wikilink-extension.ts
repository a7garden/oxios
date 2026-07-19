/**
 * Wiki-link linkify extension for CodeMirror 6.
 *
 * Replaces HyperMD's `hmdReadLink` + `hmdClick` behavior for `[[X]]`:
 *  - `[[PageName]]` and `[[PageName|alias]]` are rendered as `<a>` widgets
 *  - Clicking a RESOLVED widget dispatches `knowledge:open-file` with the
 *    resolved file path (see `web/src/lib/wikilink-resolve.ts`). Resolving
 *    bare names like `[[Rust]]` to `brain/Rust.md` here is what makes
 *    wikilinks actually navigable — previously the bare target was passed
 *    through and 404'd on open.
 *  - UNRESOLVED targets (ambiguous stem, no match) render in a muted style
 *    and do not navigate, so the user sees they need to fix the link.
 *  - Active-line behavior: on the cursor's own line (and its
 *    ±1 neighbors — the same active region used by the token-hide
 *    extension), the `[[X]]` source is left visible so the user can
 *    edit the link. This mirrors HyperMD's "you can edit what you
 *    see" rule.
 *
 * Round-trip:
 *  - The underlying text is never modified — the widget is purely
 *    visual. Saving the document yields exactly the same markdown
 *    source the user typed.
 */
import type { Range } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view'

/**
 * Match `[[PageName]]` or `[[PageName|alias1|alias2|...]]`.
 *
 * Capture group 1 = target (the part used for resolution).
 * Capture group 2 = the LAST alias group (regex backreference holds the
 * last iteration of the `*` group); when absent, the target itself is
 * the display text.
 */
const WIKILINK_RE = /\[\[([^[\]\n|]+)(?:\|([^[\]\n]+))*\]\]/g

// ─────────────────────────────────────────────────────────────────────────
// Resolver injection. The ViewPlugin is constructed once per editor
// instance, but the file tree it resolves against changes over time
// (notes created/renamed/deleted). markdown-editor.tsx calls
// `configureWikilinkResolver` whenever the tree or current file path
// changes; the version counter forces a decoration rebuild so existing
// wikilinks re-resolve without the user having to retype them.
// ─────────────────────────────────────────────────────────────────────────
type Resolver = (target: string) => string | null
let _resolver: Resolver | null = null
let _resolverVersion = 0

/**
 * Install (or clear) the wikilink resolver. Pass `null` to disable
 * resolution (all links render as unresolved). Bumping the version on
 * every call forces the ViewPlugin to rebuild decorations so links
 * re-resolve against the new tree state.
 */
export function configureWikilinkResolver(fn: Resolver | null): void {
  _resolver = fn
  _resolverVersion++
}

class WikiLinkWidget extends WidgetType {
  constructor(
    readonly target: string,
    readonly display: string,
    /** Resolved canonical path, or null if unresolved/ambiguous. */
    readonly resolved: string | null,
  ) {
    super()
  }

  eq(other: WikiLinkWidget): boolean {
    return (
      other instanceof WikiLinkWidget &&
      other.target === this.target &&
      other.display === this.display &&
      other.resolved === this.resolved
    )
  }

  toDOM(): HTMLElement {
    const a = document.createElement('a')
    a.className = this.resolved ? 'cm-wikilink' : 'cm-wikilink cm-wikilink-unresolved'
    if (this.resolved) a.setAttribute('href', `#${this.resolved}`)
    else a.removeAttribute('href')
    a.setAttribute('data-wikilink-target', this.target)
    if (this.resolved) a.setAttribute('data-wikilink-resolved', this.resolved)
    a.textContent = this.display
    // Resolved: blue dotted underline (clickable). Unresolved: muted dashed
    // (signals "this link goes nowhere") — see also the theme styles in
    // markdown-editor.tsx which can override per light/dark.
    a.style.cssText = this.resolved
      ? 'color: #79c0ff; text-decoration: underline dotted; cursor: pointer;'
      : 'color: #f0883e; text-decoration: underline dashed; cursor: help;'
    a.addEventListener('click', (e) => {
      e.preventDefault()
      if (!this.resolved) return // unresolved → no navigation
      // Reuse the same CustomEvent the linkClickHandler listens for so
      // open-file handling stays in one place.
      document.dispatchEvent(
        new CustomEvent('knowledge:open-file', { detail: { path: this.resolved } }),
      )
    })
    return a
  }

  ignoreEvent(): boolean {
    return false
  }
}

/**
 * Build a decoration set for the visible viewport.
 *
 * On the cursor's own line and its ±1 neighbors we leave the underlying
 * `[[X]]` text visible. On every other line the entire range is replaced
 * by a widget that resolves the target and renders it as a clickable link.
 */
function buildDecorations(view: EditorView): DecorationSet {
  const builder: Range<Decoration>[] = []
  const cursorLine = view.state.doc.lineAt(view.state.selection.main.head).number
  const minActive = Math.max(1, cursorLine - 1)
  const maxActive = Math.min(view.state.doc.lines, cursorLine + 1)
  const resolver = _resolver

  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to)
    WIKILINK_RE.lastIndex = 0
    let m: RegExpExecArray | null
    // biome-ignore lint/suspicious/noAssignInExpressions: standard regex exec loop pattern
    while ((m = WIKILINK_RE.exec(text)) !== null) {
      const matchStart = from + m.index
      const matchEnd = matchStart + m[0].length
      const startLine = view.state.doc.lineAt(matchStart).number
      const endLine = view.state.doc.lineAt(matchEnd).number
      const crossesActive = startLine <= maxActive && endLine >= minActive
      if (crossesActive) continue
      const target = m[1]!.trim()
      // m[2] is the LAST alias group (regex backreference holds the last
      // iteration of the `*` group). If absent, fall back to target.
      const display = (m[2] ?? m[1]!).trim()
      if (!target) continue
      const resolved = resolver ? resolver(target) : null
      builder.push(
        Decoration.replace({ widget: new WikiLinkWidget(target, display, resolved) }).range(
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
    lastResolverVersion = _resolverVersion

    constructor(view: EditorView) {
      this.decorations = buildDecorations(view)
    }

    update(u: ViewUpdate): void {
      // Re-resolve when the resolver itself changes (tree refresh), even
      // if the document and viewport are unchanged.
      const resolverChanged = this.lastResolverVersion !== _resolverVersion
      if (u.docChanged || u.viewportChanged || resolverChanged) {
        this.decorations = buildDecorations(u.view)
        this.lastResolverVersion = _resolverVersion
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
)
