/**
 * Wikilink resolution — maps a `[[target]]` string to a real file path.
 *
 * Mirrors the design in `docs/designs/2026-07-18-wikilink-rename-resolution-design.md` §3:
 * basename resolution with a directory hint (Obsidian-flavored).
 *
 *   - `[[brain/Rust.md]]` → exact match
 *   - `[[brain/Rust]]`     → `brain/Rust.md` (path + `.md`)
 *   - `[[Rust]]`           → the unique file whose stem is "Rust"; on
 *                            collision, prefer the one in the same
 *                            directory as the note containing the link;
 *                            still ambiguous → `null` (unresolved).
 *
 * The alias half of `[[target|alias]]` is stripped by the caller before
 * resolution (the widget passes only the target).
 */
import type { KnowledgeTreeNode } from '@/types/knowledge'
import { flattenTree } from './tree-utils'

/** Map of lowercase filename stem → list of full note paths. */
export type WikilinkIndex = Map<string, string[]>

/**
 * Build a stem → paths[] index from the recursive file tree. Callers
 * should memoize on the tree reference so we don't re-walk on every
 * decoration build. Directories are skipped (only `.md` files indexed).
 */
export function buildWikilinkIndex(tree: KnowledgeTreeNode[]): WikilinkIndex {
  const index: WikilinkIndex = new Map()
  for (const node of flattenTree(tree)) {
    if (node.is_dir) continue
    if (!node.path.toLowerCase().endsWith('.md')) continue
    const basename = node.path.split('/').pop() ?? node.path
    const stem = (basename.toLowerCase().endsWith('.md') ? basename.slice(0, -3) : basename).toLowerCase()
    const bucket = index.get(stem)
    if (bucket) bucket.push(node.path)
    else index.set(stem, [node.path])
  }
  return index
}

/**
 * Resolve a wikilink target to a canonical note path, or `null` if it
 * can't be resolved unambiguously.
 *
 * @param target  Raw target between `[[` and `|`/`]]` (alias already stripped).
 * @param sourcePath  Path of the note containing the link, for same-directory
 *                     disambiguation of bare-stem collisions. May be null.
 * @param index   The stem index from `buildWikilinkIndex`.
 */
export function resolveWikilink(
  target: string,
  sourcePath: string | null,
  index: WikilinkIndex,
): string | null {
  const t = target.trim()
  if (t.length === 0) return null

  // Form 1: full path with extension — exact membership check.
  if (t.toLowerCase().endsWith('.md')) {
    return pathExists(t, index) ? t : null
  }

  // Form 2: path with a directory separator — append `.md`, exact check.
  if (t.includes('/')) {
    const withExt = `${t}.md`
    return pathExists(withExt, index) ? withExt : null
  }

  // Form 3: bare stem — basename lookup with optional same-dir preference.
  const candidates = index.get(t.toLowerCase())
  if (!candidates || candidates.length === 0) return null
  if (candidates.length === 1) return candidates[0]!
  // Multiple: prefer same-directory-as-source; if none or many, unresolved.
  if (sourcePath) {
    const slash = sourcePath.lastIndexOf('/')
    const sourceDir = slash >= 0 ? sourcePath.slice(0, slash) : ''
    const sameDir = candidates.filter((p) => {
      const pSlash = p.lastIndexOf('/')
      return (pSlash >= 0 ? p.slice(0, pSlash) : '') === sourceDir
    })
    if (sameDir.length === 1) return sameDir[0]!
  }
  return null
}

/**
 * Whether a full path actually exists in the index. Stem lookup + bucket
 * membership; O(1)-ish for personal KBs.
 */
function pathExists(path: string, index: WikilinkIndex): boolean {
  const basename = path.split('/').pop() ?? path
  const stem = (basename.toLowerCase().endsWith('.md') ? basename.slice(0, -3) : basename).toLowerCase()
  const bucket = index.get(stem)
  return Boolean(bucket && bucket.includes(path))
}
