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
import {
  type FrontmatterEntry,
  findFrontmatterRange,
  parseFlowArray,
  parseFrontmatterBlock,
} from '@/lib/frontmatter'

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

/** Obsidian-style properties panel replacing a leading YAML frontmatter block.
 *
 * Display-only: clicking it moves the cursor into the block so the user edits
 * the raw YAML natively (the widget then yields to the raw text via the
 * active-region guard in `frontmatterExtension`). The block is NEVER
 * re-serialized, so nested `oxios:` metadata, quoted strings, comments, and
 * key ordering survive every edit untouched. */
class PropertiesWidget extends WidgetType {
  constructor(
    readonly entries: FrontmatterEntry[] | null,
    readonly raw: string,
    readonly rangeFrom: number,
  ) {
    super()
  }

  eq(other: PropertiesWidget) {
    return this.raw === other.raw
  }

  toDOM() {
    const wrap = document.createElement('div')
    wrap.className = 'ox-md-properties'
    wrap.setAttribute('role', 'group')
    wrap.setAttribute('aria-label', 'Properties')

    const label = document.createElement('div')
    label.className = 'ox-md-properties-label'
    label.textContent = 'Properties'
    wrap.appendChild(label)

    const body = document.createElement('dl')
    body.className = 'ox-md-properties-body'
    if (!this.entries) {
      // Unparseable — show the raw YAML verbatim rather than nothing.
      const pre = document.createElement('pre')
      pre.className = 'ox-md-properties-raw'
      pre.textContent = this.raw
      body.appendChild(pre)
    } else {
      for (const e of this.entries) {
        const dt = document.createElement('dt')
        dt.className = 'ox-md-properties-key'
        dt.textContent = e.key
        const dd = document.createElement('dd')
        dd.className = 'ox-md-properties-val'
        if (e.kind === 'array') {
          const items = parseFlowArray(e.valueText)
          if (items.length === 0) {
            dd.classList.add('ox-md-properties-empty')
          } else {
            for (const it of items) {
              const chip = document.createElement('span')
              chip.className = 'ox-md-properties-chip'
              chip.textContent = it
              dd.appendChild(chip)
            }
          }
        } else if (e.kind === 'nested') {
          // Nested mappings (e.g. the backend's `oxios:` block) render as a
          // read-only indented snippet; the user edits them via raw mode.
          const pre = document.createElement('pre')
          pre.className = 'ox-md-properties-nested'
          pre.textContent = e.raw.split('\n').slice(1).join('\n')
          dd.appendChild(pre)
        } else if (e.valueText.length === 0) {
          dd.classList.add('ox-md-properties-empty')
        } else {
          dd.textContent = e.valueText
        }
        body.appendChild(dt)
        body.appendChild(dd)
      }
    }
    wrap.appendChild(body)

    // Click → place the cursor inside the block; the active-region guard then
    // swaps the widget for the raw, editable YAML (native CodeMirror editing).
    wrap.addEventListener('mousedown', (ev) => {
      ev.preventDefault()
      const view = EditorView.findFromDOM(ev.target as HTMLElement)
      if (view) view.dispatch({ selection: { anchor: this.rangeFrom } })
    })
    return wrap
  }

  // The widget owns pointer events so the editor doesn't fight the click
  // handler; keyboard users reach the raw block by arrowing into it.
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

  // Leading YAML frontmatter is rendered exclusively by `frontmatterExtension`
  // (properties widget / raw-edit). Skip every node whose start lies inside
  // that range so the closing `---` can't promote the preceding property line
  // into a Setext heading (the "big text" rendering bug) and the delimiters
  // don't become HR widgets.
  const fmRange = findFrontmatterRange(state.sliceDoc(0, Math.min(doc.length, 8192)))
  const inFrontmatter = (from: number) => !!fmRange && from < fmRange.to

  // The document title is the first ATX H1 AFTER the frontmatter (line 1 when
  // there is none). Recorded so it can be styled larger and its `#` hidden.
  let titleRange: { from: number; to: number } | null = null
  const bodyStart = fmRange ? fmRange.to : 0

  /** Tag every line in [from,to] with a line decoration class. */
  const tagLines = (from: number, to: number, cls: string) => {
    const deco = Decoration.line({ class: cls })
    for (let n = doc.lineAt(from).number; n <= doc.lineAt(to).number; n++) {
      builder.push(deco.range(doc.line(n).from))
    }
  }

  syntaxTree(state).iterate({
    enter(node) {
      if (inFrontmatter(node.from)) return
      const level = headingLevel(node.name)
      if (level) {
        // The first ATX H1 at/after the body start is the document title —
        // render it larger than in-body headings (Obsidian inline title).
        if (!titleRange && level === 1 && node.name.startsWith('ATX') && node.from >= bodyStart) {
          titleRange = { from: node.from, to: node.to }
        }
        const isTitle = !!titleRange && node.from === titleRange.from
        tagLines(node.from, node.to, isTitle ? `ox-md-h${level} ox-md-first` : `ox-md-h${level}`)
        return
      }
      switch (node.name) {
        case 'HeaderMark': {
          // Inline title: always hide the `#` of the document title so it
          // reads clean even while the cursor is on the lines next to it.
          // (tokenHide only hides markup outside the ±1 active region, so
          // without this the `#` would flicker back in near the top.) The
          // headingEnforcer still keeps `# ` in the source; the user edits
          // the title text directly and never sees the marker.
          if (titleRange && node.from >= titleRange.from && node.to <= titleRange.to) {
            builder.push(Decoration.replace({}).range(node.from, node.to))
          }
          return
        }
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
  // Strong: semibold, not ultra-heavy.
  { tag: tags.strong, fontWeight: '600' },
  // Strikethrough (GFM): line-through. Paired with tokenHide hiding `~~`.
  { tag: tags.strikethrough, textDecoration: 'line-through' },
  // Inline code: subtle background, mono font, no border.
  {
    tag: tags.monospace,
    fontFamily: 'var(--editor-font-mono)',
    background: 'var(--muted)',
    color: 'var(--foreground)',
    borderRadius: '4px',
    padding: '0.1em 0.35em',
    fontSize: '0.875em',
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
          fontSize: '1.5rem',
          fontWeight: '600',
          lineHeight: '2rem',
          paddingTop: '1.5rem',
          paddingBottom: '0.5rem',
        },
        '.ox-md-h1.ox-md-first': {
          // Document title (Obsidian-style inline title). The note's first
          // H1 IS its title — rendered prominently so it reads as the page
          // identity. tokenHide strips the `#`; editing it renames on save.
          fontSize: '1.875rem',
          fontWeight: '700',
          lineHeight: '2.375rem',
          paddingTop: '0.75rem',
          paddingBottom: '1rem',
        },
        '.ox-md-h2': {
          fontSize: '1.25rem',
          fontWeight: '600',
          lineHeight: '1.75rem',
          paddingTop: '1.25rem',
          paddingBottom: '0.5rem',
        },
        '.ox-md-h3': {
          fontSize: '1.125rem',
          fontWeight: '600',
          lineHeight: '1.625rem',
          paddingTop: '1rem',
          paddingBottom: '0.375rem',
        },
        '.ox-md-h4': {
          fontSize: '1rem',
          fontWeight: '600',
          lineHeight: '1.5rem',
          paddingTop: '0.875rem',
          paddingBottom: '0.375rem',
        },
        '.ox-md-h5': {
          fontSize: '0.9375rem',
          fontWeight: '600',
          lineHeight: '1.375rem',
          paddingTop: '0.75rem',
          paddingBottom: '0.25rem',
        },
        '.ox-md-h6': {
          fontSize: '0.875rem',
          fontWeight: '600',
          lineHeight: '1.375rem',
          paddingTop: '0.75rem',
          paddingBottom: '0.25rem',
        },
        // ── Block elements ──
        '.ox-md-quote': {
          borderLeft: '3px solid var(--border)',
          paddingLeft: '1rem',
          paddingRight: '0.5rem',
          color: 'var(--muted-foreground)',
          fontStyle: 'italic',
        },
        '.ox-md-codeblock': {
          background: 'var(--muted)',
          borderRadius: '6px',
          paddingLeft: '1rem',
          paddingRight: '0.75rem',
        },
        '.ox-md-hr': {
          borderTop: '1px solid var(--border)',
          height: '0',
          margin: '1rem 0',
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

// ─── Frontmatter properties (always-on) ──────────────────────────────
//
// Independent of the `livePreview` preference: frontmatter rendering is a
// correctness concern (without it the closing `---` inflates the preceding
// property line into a Setext heading — the "big text" bug). When the cursor
// is OUTSIDE the block, the raw `---…---` is replaced by a PropertiesWidget;
// when the cursor moves INSIDE, the widget yields and the raw YAML shows for
// native CodeMirror editing (each line tagged plain so no Setext inflation).
// `buildDecorations` (live-preview) skips the same range to avoid conflicts.

export function buildFrontmatterDecorations(state: EditorState): DecorationSet {
  const fm = findFrontmatterRange(state.sliceDoc(0, Math.min(state.doc.length, 8192)))
  if (!fm) return Decoration.none
  const doc = state.doc
  const fmStartLine = doc.lineAt(fm.from).number
  const fmEndLine = doc.lineAt(Math.max(0, fm.to - 1)).number
  const cursorLine = doc.lineAt(state.selection.main.head).number
  // Cursor within the block → render the raw YAML for native editing.
  if (cursorLine >= fmStartLine && cursorLine <= fmEndLine) {
    const builder: Range<Decoration>[] = []
    const deco = Decoration.line({ class: 'ox-md-frontmatter-raw' })
    for (let n = fmStartLine; n <= fmEndLine; n++) builder.push(deco.range(doc.line(n).from))
    builder.sort((a, b) => a.from - b.from)
    return Decoration.set(builder, true)
  }
  // Otherwise replace the whole block with the properties widget. The range
  // is line-aligned (line-1 start → next-line start), so a BLOCK widget is
  // the correct CodeMirror tool for replacing whole lines — strictly safer
  // than a multi-line inline replace.
  const raw = state.sliceDoc(fm.from, fm.to)
  const entries = parseFrontmatterBlock(raw)
  return Decoration.set(
    [
      Decoration.replace({
        widget: new PropertiesWidget(entries, raw, fm.from),
        block: true,
      }).range(fm.from, fm.to),
    ],
    true,
  )
}

export const frontmatterExtension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorViewType) {
      this.decorations = buildFrontmatterDecorations(view.state)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || update.selectionSet) {
        this.decorations = buildFrontmatterDecorations(update.view.state)
      }
    }
  },
  {
    decorations: (v) => v.decorations,
    provide: () =>
      EditorView.baseTheme({
        '.ox-md-properties': {
          margin: '0 0 0.75rem 0',
          padding: '0.5rem 0.75rem',
          border: '1px solid var(--border)',
          borderRadius: '6px',
          background: 'var(--muted)',
          cursor: 'text',
          userSelect: 'text',
        },
        '.ox-md-properties-label': {
          fontSize: '0.7rem',
          fontWeight: '600',
          textTransform: 'uppercase',
          letterSpacing: '0.04em',
          color: 'var(--muted-foreground)',
          marginBottom: '0.35rem',
        },
        // Two-column key/value grid; dt and dd alternate into columns 1/2.
        '.ox-md-properties-body': {
          display: 'grid',
          gridTemplateColumns: 'max-content 1fr',
          columnGap: '0.75rem',
          rowGap: '0.2rem',
          margin: '0',
        },
        '.ox-md-properties-key': {
          fontSize: '0.8rem',
          color: 'var(--muted-foreground)',
          fontWeight: '500',
          alignSelf: 'center',
        },
        '.ox-md-properties-val': {
          fontSize: '0.85rem',
          margin: '0',
          alignSelf: 'center',
          color: 'var(--foreground)',
        },
        '.ox-md-properties-chip': {
          display: 'inline-block',
          padding: '0.05rem 0.4rem',
          marginRight: '0.25rem',
          borderRadius: '999px',
          background: 'var(--background)',
          border: '1px solid var(--border)',
          fontSize: '0.75rem',
        },
        '.ox-md-properties-nested': {
          margin: '0',
          fontFamily: 'var(--editor-font-mono)',
          fontSize: '0.75rem',
          color: 'var(--muted-foreground)',
          whiteSpace: 'pre-wrap',
        },
        '.ox-md-properties-raw': {
          margin: '0',
          fontFamily: 'var(--editor-font-mono)',
          fontSize: '0.78rem',
          color: 'var(--muted-foreground)',
          whiteSpace: 'pre-wrap',
        },
        '.ox-md-properties-empty': {
          opacity: '0.5',
        },
        '.ox-md-frontmatter-raw': {
          // Plain mono styling for raw-edit mode — neutralises any Setext
          // heading inflation the closing `---` would otherwise apply.
          fontFamily: 'var(--editor-font-mono)',
          fontSize: '0.85rem',
          color: 'var(--muted-foreground)',
        },
      }),
  },
)
