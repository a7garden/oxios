/**
 * YAML frontmatter detection and tolerant parsing for the knowledge editor.
 *
 * The backend (`oxios-markdown::parse_note_meta`, RFC-022) requires frontmatter
 * at the very top of the file (byte 0), delimited by `---` lines. Agent-saved
 * notes carry a nested `oxios:` block; user/Obsidian notes carry flat
 * `key: value` properties.
 *
 * These helpers power the properties widget:
 *  - {@link findFrontmatterRange} locates the block (for decoration + raw-edit).
 *  - {@link parseFrontmatter}tolerantly splits it into display entries.
 *
 * Parsing is DISPLAY-ONLY — the widget never re-serializes the block, so quoted
 * strings, comments, key ordering, and nested mappings survive untouched. The
 * user edits the raw YAML directly (cursor enters the block → raw text shows).
 */

/** A byte range [from, to) within a document. */
export interface TextRange {
  from: number
  to: number
}

/** The shape of a single frontmatter entry for display. */
export interface FrontmatterEntry {
  /** Top-level key (trimmed). */
  key: string
  /** Inline value text (the span after `key:`, trimmed). Empty for nested. */
  valueText: string
  /** Coarse kind, picked for rendering. */
  kind: 'scalar' | 'array' | 'nested'
  /** Full raw YAML text of this entry (key line + any indented body). */
  raw: string
}

/**
 * Locate a leading YAML frontmatter block in `text`.
 *
 * Returns the range covering the opening `---`, the body, the closing `---` or
 * `...`, AND the closing delimiter's trailing newline — so the body starts
 * cleanly at `to`. Returns `null` when the document doesn't begin with a
 * well-formed frontmatter block.
 *
 * `maxScan` bounds how far we look for the closer (frontmatter is always at the
 * top and tiny); past it we treat the block as absent rather than unbounded.
 */
export function findFrontmatterRange(text: string, maxScan = 8192): TextRange | null {
  // Opening delimiter: first line must be exactly `---` (trailing whitespace ok).
  if (!text.startsWith('---')) return null
  const nl = text.indexOf('\n')
  if (nl === -1) return null // single-line doc starting with `---` — not a block
  if (text.slice(0, nl).trimEnd() !== '---') return null // `---foo` etc. isn't a delimiter

  const scan = text.slice(0, Math.min(text.length, maxScan))
  const lines = scan.split('\n')
  // Scan body lines (index 1 onward) for the closing delimiter.
  let pos = nl + 1 // byte offset of the start of line index 1
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i]!
    const trimmed = line.replace(/\s+$/, '')
    if (trimmed === '---' || trimmed === '...') {
      const closeLineEnd = pos + line.length
      // Include the closing line's trailing newline when one follows, so the
      // replaced range ends at the start of the first body line.
      const to = text[closeLineEnd] === '\n' ? closeLineEnd + 1 : closeLineEnd
      return { from: 0, to }
    }
    pos += line.length + 1 // +1 for the `\n`
  }
  return null // no closing delimiter within the scan window
}

/**
 * Tolerantly split frontmatter body (the text BETWEEN the `---` delimiters)
 * into display entries. Returns `null` when the structure isn't recognisable
 * as top-level `key:` mappings — the caller then falls back to showing the
 * raw block verbatim.
 *
 * Recognises:
 *  - `key: value`            → scalar
 *  - `key: [a, b, c]`        → array (inline flow)
 *  - `key:` + indented body  → nested (raw body preserved)
 *
 * Bare comment lines (`# ...`) and blank lines are skipped. A non-comment,
 * indented line with no preceding top-level key is treated as malformed.
 */
export function parseFrontmatter(yaml: string): FrontmatterEntry[] | null {
  const lines = yaml.split('\n')
  const entries: FrontmatterEntry[] = []
  let i = 0
  while (i < lines.length) {
    const line = lines[i]!
    if (line.trim() === '' || line.trimStart().startsWith('#')) {
      i++
      continue
    }
    // A top-level entry must start at column 0.
    if (/^[ \t]/.test(line)) {
      // Indented line with no owning key — bail to raw fallback.
      return null
    }
    // `key:` with an optional inline value. Keys may not contain `:`.
    const m = line.match(/^([^:#\][{},"']*?):(.*)$/)
    if (!m) return null
    const key = m[1]!.trim()
    if (key.length === 0) return null
    const inline = m[2]!.replace(/^\s+/, '') // value text after the colon
    // Gather any indented continuation lines (nested body).
    const rawLines = [line]
    let j = i + 1
    while (j < lines.length) {
      const next = lines[j]!
      if (next.trim() === '') break
      if (!/^[ \t]/.test(next)) break
      rawLines.push(next)
      j++
    }
    let kind: FrontmatterEntry['kind'] = 'scalar'
    if (j > i + 1) {
      kind = 'nested'
    } else if (inline.startsWith('[') && inline.endsWith(']')) {
      kind = 'array'
    }
    entries.push({ key, valueText: inline, kind, raw: rawLines.join('\n') })
    i = j
  }
  return entries.length > 0 ? entries : null
}

/**
 * Parse an inline flow array `[a, b, "c d"]` into its scalar items. Used only
 * for display (chips); never feeds back into the document, so quoting/escaping
 * loss is irrelevant.
 */
export function parseFlowArray(valueText: string): string[] {
  const inner = valueText.trim().replace(/^\[/, '').replace(/\]$/, '')
  if (inner.trim() === '') return []
  return inner
    .split(',')
    .map((s) =>
      s
        .trim()
        .replace(/^"(.*)"$/, '$1')
        .replace(/^'(.*)'$/, '$1'),
    )
    .filter((s) => s.length > 0)
}

/**
 * Parse a FULL frontmatter block (including the `---` delimiters) into display
 * entries. Convenience wrapper around {@link parseFrontmatter} that strips the
 * opening/closing delimiter lines first. Returns `null` to fall back to raw.
 */
export function parseFrontmatterBlock(raw: string): FrontmatterEntry[] | null {
  const firstNl = raw.indexOf('\n')
  if (firstNl < 0) return null
  let inner = raw.slice(firstNl + 1)
  // Drop a trailing closing delimiter line (`---` / `...`), if present.
  const trimmedEnd = inner.replace(/\n+$/, '')
  const lastNl = trimmedEnd.lastIndexOf('\n')
  const closing = (lastNl >= 0 ? trimmedEnd.slice(lastNl + 1) : trimmedEnd).trim()
  if (closing === '---' || closing === '...') {
    inner = lastNl >= 0 ? trimmedEnd.slice(0, lastNl) : ''
  }
  return parseFrontmatter(inner)
}
