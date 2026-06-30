/**
 * Markdown live-preview extension for CodeMirror 6.
 *
 * Restores the visual rendering lost when the knowledge editor migrated from
 * HyperMD (CodeMirror 5) to CodeMirror 6. Token hiding (token-hide-extension)
 * collapses markup on inactive lines; THIS extension supplies the rest of
 * files.md's live rendering:
 *
 *   - Headings: per-level font-size/weight (H1–H6) — files.md brutal theme.
 *   - Horizontal rule (`---`): replaced by a thin rule widget on inactive lines.
 *   - Blockquote (`>`): muted background + left border per line.
 *   - Fenced code (```): muted background per line.
 *   - Task list (`- [ ]` / `- [x]`): checkbox widget on inactive lines.
 *   - Bold (800), strikethrough (line-through), inline code (boxed): via the
 *     companion `livePreviewHighlight` HighlightStyle.
 *
 * Values are expressed in rem (heading sizes, equal to files.md px at a 16px
 * root) and oxios design tokens (--muted, --border, …) so light/dark both
 * adapt. Colour for syntax tokens otherwise stays with defaultHighlightStyle /
 * oneDark; this HighlightStyle overrides only the properties it names, so it
 * never clobbers the dark-mode heading/link colours.
 *
 * Round-trip safe: purely visual — the markdown source is never modified.
 */
import { HighlightStyle, syntaxTree } from '@codemirror/language'
import type { EditorState, Range } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  EditorView,
  type EditorView as EditorViewType,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view'
import { tags } from '@lezer/highlight'

const HEADING_RE = /(?:ATX|Setext)Heading(\d)$/

/** Return the heading level (1–6) for a node name, or null. */
function headingLevel(name: string): number | null {
  const m = HEADING_RE.exec(name)
  return m ? Number(m[1]) : null
}

// ─── Widgets ──────────────────────────────────────────────────────────

/** Replaces a `---` horizontal-rule line with a thin rule. */
class HrWidget extends WidgetType {
  eq() {
    return true
  }
  toDOM() {
    const el = document.createElement('div')
    el.className = 'ox-md-hr'
    el.setAttribute('aria-hidden', 'true')
    return el
  }
  ignoreEvent() {
    return false
  }
}

/** Replaces a `- [ ]` / `- [x]` task marker with a checkbox that toggles on click. */
class TaskWidget extends WidgetType {
  constructor(
    readonly checked: boolean,
    readonly from: number,
    readonly to: number,
  ) {
    super()
  }
  eq(other: TaskWidget) {
    return other.checked === this.checked && other.from === this.from && other.to === this.to
  }
  toDOM() {
    const el = document.createElement('input')
    el.type = 'checkbox'
    el.className = 'ox-md-task'
    el.checked = this.checked
    el.title = this.checked ? 'Mark incomplete' : 'Mark complete'
    el.addEventListener('click', (event) => {
      event.preventDefault()
      // Widgets don't receive the view at construction — resolve the owning
      // editor from the DOM and dispatch the `[ ]`↔`[x]` replacement. The
      // marker is always 3 chars, so `from`/`to` stay valid post-dispatch.
      EditorView.findFromDOM(el)?.dispatch({
        changes: { from: this.from, to: this.to, insert: this.checked ? '[ ]' : '[x]' },
        selection: { anchor: this.from },
      })
    })
    return el
  }
  // Widget owns the click (toggle); the editor should not also act on it.
  ignoreEvent() {
    return true
  }
}

// ─── Decorations ──────────────────────────────────────────────────────

export function buildDecorations(state: EditorState): DecorationSet {
  const builder: Range<Decoration>[] = []
  const { doc } = state

  // Active region (cursor line ±1): widgets that hide source (HR, task) are
  // suppressed here so the user can edit the raw markup. Line styling
  // (quote/codeblock/heading) applies everywhere, matching files.md.
  const cursorLine = doc.lineAt(state.selection.main.head).number
  const inActiveRegion = (from: number, to: number) => {
    const s = doc.lineAt(from).number
    const e = doc.lineAt(to).number
    return s <= cursorLine + 1 && e >= cursorLine - 1
  }

  /** Tag every line in [from,to] with a line decoration class. */
  const tagLines = (from: number, to: number, cls: string) => {
    const deco = Decoration.line({ class: cls })
    for (let n = doc.lineAt(from).number; n <= doc.lineAt(to).number; n++) {
      builder.push(deco.range(doc.line(n).from))
    }
  }

  syntaxTree(state).iterate({
    enter(node) {
      const level = headingLevel(node.name)
      if (level) {
        const startLine = doc.lineAt(node.from).number
        const cls = startLine === 1 ? `ox-md-h${level} ox-md-first` : `ox-md-h${level}`
        tagLines(node.from, node.to, cls)
        return
      }
      switch (node.name) {
        case 'HorizontalRule':
          if (!inActiveRegion(node.from, node.to)) {
            builder.push(Decoration.replace({ widget: new HrWidget() }).range(node.from, node.to))
          }
          return
        case 'TaskMarker':
          if (!inActiveRegion(node.from, node.to)) {
            const text = state.doc.sliceString(node.from, node.to)
            const checked = /\[[xX]\]/.test(text)
            builder.push(
              Decoration.replace({ widget: new TaskWidget(checked, node.from, node.to) }).range(
                node.from,
                node.to,
              ),
            )
          }
          return
        case 'Blockquote':
          tagLines(node.from, node.to, 'ox-md-quote')
          return
        case 'FencedCode':
          tagLines(node.from, node.to, 'ox-md-codeblock')
          return
      }
    },
  })

  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

// ─── Inline token styling (HighlightStyle) ────────────────────────────
//
// Registered AFTER defaultHighlightStyle / oneDark (see markdown-editor), so
// these win for the properties they name without clobbering the colours the
// dark theme provides. text-decoration/background/border flow through
// style-mod, so they render even though TagStyle only advertises colour.

export const livePreviewHighlight = HighlightStyle.define([
  // files.md brutal: "Heavy bold" = 800.
  { tag: tags.strong, fontWeight: '800' },
  // Strikethrough (GFM): line-through. Paired with tokenHide hiding `~~`.
  { tag: tags.strikethrough, textDecoration: 'line-through' },
  // Inline code: boxed, mono, slightly smaller — files.md brutal treatment.
  {
    tag: tags.monospace,
    fontFamily: 'var(--editor-font-mono)',
    background: 'var(--muted)',
    color: 'var(--foreground)',
    border: '1px solid var(--border)',
    borderRadius: '3px',
    padding: '0.1em 0.35em',
    fontSize: '0.9em',
  },
])

// ─── ViewPlugin + theme ───────────────────────────────────────────────

export const livePreviewExtension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorViewType) {
      this.decorations = buildDecorations(view.state)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || update.selectionSet) {
        this.decorations = buildDecorations(update.view.state)
      }
    }
  },
  {
    decorations: (v) => v.decorations,
    // Base theme applies in both light and dark (lowest priority). Sizes are
    // mode-agnostic — colour comes from the highlight styles, never here.
    provide: () =>
      EditorView.baseTheme({
        // ── Headings (files.md brutal theme, rem = px/16) ──
        '.ox-md-h1': {
          fontSize: '2rem',
          fontWeight: '700',
          lineHeight: '2.375rem',
          paddingTop: '1.125rem',
          paddingBottom: '0.25rem',
        },
        '.ox-md-h1.ox-md-first': {
          paddingTop: '0',
          paddingBottom: '1rem',
        },
        '.ox-md-h2': {
          fontSize: '1.5rem',
          fontWeight: '700',
          lineHeight: '1.875rem',
          paddingTop: '1.125rem',
          paddingBottom: '0.375rem',
        },
        '.ox-md-h3': {
          fontSize: '1.375rem',
          fontWeight: '700',
          lineHeight: '1.75rem',
          paddingTop: '1rem',
          paddingBottom: '0.3125rem',
        },
        '.ox-md-h4': {
          fontSize: '1.25rem',
          fontWeight: '700',
          lineHeight: '1.625rem',
          paddingTop: '0.875rem',
          paddingBottom: '0.25rem',
        },
        '.ox-md-h5': {
          fontSize: '1.125rem',
          fontWeight: '700',
          lineHeight: '1.5rem',
          paddingTop: '0.75rem',
          paddingBottom: '0.25rem',
        },
        '.ox-md-h6': {
          fontSize: '1rem',
          fontWeight: '700',
          lineHeight: '1.375rem',
          paddingTop: '0.75rem',
          paddingBottom: '0.25rem',
        },
        // ── Block elements ──
        '.ox-md-quote': {
          background: 'var(--muted)',
          borderLeft: '3px solid var(--border)',
          paddingLeft: '0.75rem',
          paddingRight: '0.5rem',
          borderRadius: '0 4px 4px 0',
          color: 'var(--muted-foreground)',
          fontStyle: 'italic',
        },
        '.ox-md-codeblock': {
          background: 'var(--muted)',
          borderRadius: '4px',
          paddingLeft: '0.75rem',
          paddingRight: '0.5rem',
        },
        '.ox-md-hr': {
          borderTop: '2px solid var(--border)',
          height: '0',
          margin: '0.75rem 0',
        },
        '.ox-md-task': {
          marginRight: '0.4em',
          verticalAlign: 'middle',
          cursor: 'default',
          width: '1em',
          height: '1em',
        },
      }),
  },
)
