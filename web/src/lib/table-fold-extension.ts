/**
 * Table fold extension for CodeMirror 6.
 *
 * Renders a GFM markdown table as an HTML grid (`<table>`) on inactive lines
 * (cursor outside ±1 of the table's line range). The whole table is replaced
 * by a single widget, so the `|` delimiters and `---` alignment row are all
 * visual — no editing weirdness inside an active source view.
 *
 * Column alignment comes from the delimiter row (`:--` left, `:-:` centre,
 * `--:` right; default left). Rows that don't match the header count are
 * dropped (the widget just renders whatever parses cleanly). The Table node
 * contains rows as text — the parser splits the row text on `|`.
 *
 * `TableDelimiter` is added to `tokenHideExtension`'s MARKUP_NODE_NAMES so
 * the `|` markers and `---` alignment row disappear on inactive lines too
 * (paired change: hide lands with the widget).
 *
 * Round-trip safe: purely visual; the markdown source is preserved.
 */
import { syntaxTree } from '@codemirror/language'
import type { EditorState, Extension, Range } from '@codemirror/state'
import { StateField } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, WidgetType } from '@codemirror/view'

function arraysEqual<T>(a: readonly T[], b: readonly T[]): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false
  }
  return true
}

function setAlign(col: HTMLElement, value: string): void {
  col.style.textAlign = value
}

function splitRow(line: string): string[] {
  const t = line.trim().replace(/^\|/, '').replace(/\|$/, '')
  if (!t) return []
  const parts = t.split('|').map((c) => c.trim())
  return parts.some((p) => p === '') ? [] : parts
}

function parseAlignments(delims: string[]): string[] {
  return delims.map((d) => {
    const left = d.startsWith(':')
    const right = d.endsWith(':')
    if (left && right) return 'center'
    if (right) return 'right'
    return 'left'
  })
}

interface ParsedTable {
  headers: string[]
  rows: string[][]
  alignments: string[]
}

function parseTable(text: string): ParsedTable | null {
  const lines = text.split('\n')
  if (lines.length < 2) return null
  const headers = splitRow(lines[0] ?? '')
  const delims = splitRow(lines[1] ?? '')
  if (headers.length === 0 || delims.length !== headers.length) return null
  const alignments = parseAlignments(delims)
  const rows = lines
    .slice(2)
    .map((l) => splitRow(l))
    .filter((r) => r.length === headers.length)
  return { headers, rows, alignments }
}

class TableWidget extends WidgetType {
  constructor(
    readonly headers: string[],
    readonly rows: string[][],
    readonly alignments: string[],
  ) {
    super()
  }
  eq(other: TableWidget): boolean {
    return (
      arraysEqual(this.headers, other.headers) &&
      this.rows.length === other.rows.length &&
      this.rows.every((r, i) => arraysEqual(r, other.rows[i] ?? []))
    )
  }
  toDOM() {
    const wrap = document.createElement('div')
    wrap.className = 'ox-md-table-wrap'
    const table = document.createElement('table')
    table.className = 'ox-md-table'
    const colgroup = document.createElement('colgroup')
    this.alignments.forEach((a) => {
      const col = document.createElement('col')
      setAlign(col, a)
      colgroup.appendChild(col)
    })
    table.appendChild(colgroup)
    const thead = document.createElement('thead')
    const headRow = document.createElement('tr')
    this.headers.forEach((h) => {
      const th = document.createElement('th')
      th.textContent = h
      headRow.appendChild(th)
    })
    thead.appendChild(headRow)
    table.appendChild(thead)
    const tbody = document.createElement('tbody')
    this.rows.forEach((row) => {
      const tr = document.createElement('tr')
      row.forEach((cell) => {
        const td = document.createElement('td')
        td.textContent = cell
        tr.appendChild(td)
      })
      tbody.appendChild(tr)
    })
    table.appendChild(tbody)
    wrap.appendChild(table)
    return wrap
  }
  ignoreEvent() {
    return false
  }
}

export function buildTableDecorations(state: EditorState): DecorationSet {
  const builder: Range<Decoration>[] = []
  const { doc } = state
  const cursorLine = doc.lineAt(state.selection.main.head).number
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name !== 'Table') return
      // Per-table active region: cursor near (±1) any table line → show
      // source everywhere so the user can edit any row coherently.
      const fromLine = doc.lineAt(node.from).number
      const toLine = doc.lineAt(node.to).number
      if (cursorLine >= fromLine - 1 && cursorLine <= toLine + 1) return false
      const text = state.doc.sliceString(node.from, node.to)
      const parsed = parseTable(text)
      if (!parsed) return false
      builder.push(
        Decoration.replace({
          widget: new TableWidget(parsed.headers, parsed.rows, parsed.alignments),
        }).range(node.from, node.to),
      )
      return false
    },
  })
  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

// StateField — not a ViewPlugin — because the table widget replaces a
// range that spans line breaks, and CodeMirror 6 only permits
// line-break-spanning replace decorations from state-derived sources.
// Public API is unchanged: `tableFoldExtension` is an `Extension`
// (theme + field) that drops straight into the editor's extensions[].
export const tableFoldExtension: Extension = [
  EditorView.baseTheme({
    '.ox-md-table-wrap': {
      display: 'block',
      margin: '0.6em 0',
      overflowX: 'auto',
    },
    '.ox-md-table': {
      borderCollapse: 'collapse',
      width: '100%',
      fontSize: '0.9em',
    },
    '.ox-md-table th, .ox-md-table td': {
      border: '1px solid var(--border)',
      padding: '0.35em 0.6em',
    },
    '.ox-md-table thead th': {
      background: 'var(--muted)',
      fontWeight: '600',
    },
    '.ox-md-table tbody tr:nth-child(even)': {
      background: 'color-mix(in srgb, var(--muted) 40%, transparent)',
    },
  }),
  StateField.define<DecorationSet>({
    create(state) {
      return buildTableDecorations(state)
    },
    update(deco, tr) {
      return tr.docChanged || tr.selection != null
        ? buildTableDecorations(tr.state)
        : deco.map(tr.changes)
    },
    provide: (f) => EditorView.decorations.from(f),
  }),
]
