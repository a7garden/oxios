/**
 * H1-driven rename helpers for the knowledge editor.
 *
 * The web editor treats the note's first `# Heading` line as the note's
 * title (the CodeMirror `headingEnforcer` keeps line 1 an H1 at all
 * times). When the user edits that H1, we rename the underlying file so
 * the sidebar file tree, search, wikilinks, and URL all stay consistent
 * with the visible title.
 *
 * Mirrors the backend's special-file rules (oxios-markdown `types.rs`):
 * the chat/inbox system files, the journal/habits/archive/insights/media
 * directories, and any non-`.md` file are off-limits for H1 rename
 * because their filenames are load-bearing for other subsystems.
 */

/** Backend `SYSTEM_FILES` — root files with structural meaning. */
const SYSTEM_FILES: Record<string, true> = {
  'Chat.md': true,
  'Later.md': true,
  'Done.md': true,
  'Shop.md': true,
  'Watch.md': true,
  'Read.md': true,
}

/** Backend reserved directory names (top-level segment). */
const RESERVED_DIRS: Record<string, true> = {
  archive: true,
  journal: true,
  habits: true,
  insights: true,
  media: true,
}

/** Max filename stem length — leaves headroom under the 255-byte FS limit. */
const MAX_STEM_LEN = 100

/**
 * Extract the H1 text from a markdown document. The heading enforcer
 * guarantees line 1 starts with `# `; we still tolerate a bare `#` or a
 * missing prefix defensively. Returns `null` when there is no usable
 * (non-empty) heading text — the caller treats that as "no rename".
 */
export function extractH1(content: string): string | null {
  const firstLine = content.split('\n', 1)[0] ?? ''
  const match = firstLine.match(/^#{0,6}\s+(.*)$/)
  const text = match?.[1]?.trim() ?? ''
  return text.length > 0 ? text : null
}

/**
 * Convert an arbitrary H1 string into a safe filename stem (no extension,
 * no path separators). Strips characters that are illegal or hostile in
 * filenames across macOS/Linux/Windows, trims, and caps length. Returns
 * an empty string if nothing usable remains.
 */
export function sanitizeFilenameStem(name: string): string {
  // Strip path separators and characters reserved by common filesystems.
  const cleaned = name
    .replace(/[\\/:*?"<>|]/g, '')
    // biome-ignore lint/suspicious/noControlCharactersInRegex: intentionally strip control chars for safe filenames
    .replace(/[\u0000-\u001f]/g, '')
    .trim()
  if (cleaned.length === 0) return ''
  // Collapse internal runs of whitespace into single spaces so the file
  // list stays tidy; the user's word boundaries are preserved.
  const collapsed = cleaned.replace(/\s+/g, ' ')
  return collapsed.length > MAX_STEM_LEN ? collapsed.slice(0, MAX_STEM_LEN).trim() : collapsed
}

/**
 * Whether a path is protected from H1-driven rename. Mirrors the
 * backend's special-file rules — renaming `Chat.md` or a journal entry
 * would break the chat / habits / journal subsystems that key off those
 * exact filenames.
 */
export function isProtectedPath(path: string): boolean {
  if (!path) return true
  const trimmed = path.trim().replace(/^\/+/, '')
  if (!trimmed) return true
  // Non-markdown files are never rename candidates.
  if (!trimmed.toLowerCase().endsWith('.md')) return true
  const topSegment = trimmed.split('/', 1)[0] ?? trimmed
  // Root-level system file?
  if (trimmed in SYSTEM_FILES) return true
  // Inside a reserved directory at any depth.
  if (topSegment in RESERVED_DIRS) return true
  return false
}

/** Split a POSIX-style note path into (dir, basename, stem, ext). */
function decomposePath(path: string): { dir: string; basename: string; stem: string } {
  const trimmed = path.replace(/^\/+/, '')
  const slash = trimmed.lastIndexOf('/')
  const dir = slash >= 0 ? trimmed.slice(0, slash) : ''
  const basename = slash >= 0 ? trimmed.slice(slash + 1) : trimmed
  const stem = basename.toLowerCase().endsWith('.md') ? basename.slice(0, -3) : basename
  return { dir, basename, stem }
}

/** Join a directory and filename into a POSIX path (no leading slash). */
function joinPath(dir: string, filename: string): string {
  return dir ? `${dir}/${filename}` : filename
}

/**
 * Compute the target rename path for the given current path and H1 text.
 * Returns `null` when:
 *  - the path is protected (system file / reserved dir / non-md),
 *  - the H1 is empty or sanitizes to nothing,
 *  - the desired stem already equals the current stem (no-op).
 *
 * The comparison is case-sensitive on the stem so that fixing the case
 * of a title (e.g. `rust.md` → `Rust.md`) still triggers a rename.
 */
export function desiredRenamePath(currentPath: string, h1: string | null): string | null {
  if (isProtectedPath(currentPath)) return null
  if (!h1) return null
  const desiredStem = sanitizeFilenameStem(h1)
  if (desiredStem.length === 0) return null
  const { dir, stem } = decomposePath(currentPath)
  if (desiredStem === stem) return null
  return joinPath(dir, `${desiredStem}.md`)
}
