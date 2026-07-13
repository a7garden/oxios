/**
 * Shared utilities for tree views (knowledge, workspace, chat sessions).
 *
 * Replaces the previous plan to extract a 13-prop `<TreeNode>` component
 * (which was over-abstraction). The three tree views stay independent but
 * import these helpers so the visual and behavior primitives stay aligned.
 */

import type { CSSProperties } from 'react'

/**
 * Padding-left based on depth in a tree view, unified across the app.
 * `16px` per level keeps siblings visually distinct; `8px` base padding
 * matches the sidebar's outer `p-2`.
 */
export function indentStyle(depth: number): CSSProperties {
  return { paddingLeft: `${depth * 16 + 8}px` }
}

/**
 * Tailwind text-color tint for a file-name, by extension category. Used
 * for at-a-glance file-type identification. Matches the workspace tree.
 */
export function fileTint(name: string): string {
  const ext = (name.split('.').pop() ?? '').toLowerCase()
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'text-pink-500'
  if (['rs', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'py', 'go', 'rb'].includes(ext)) {
    return 'text-blue-500'
  }
  if (['md', 'mdx', 'markdown', 'txt', 'json', 'toml', 'yaml', 'yml'].includes(ext)) {
    return 'text-amber-500'
  }
  return 'text-muted-foreground'
}

/**
 * Counts every file nested under `node` (including across subfolders).
 * Returns 0 when `node` is not a directory.
 */
export function countFilesRecursive<T extends { is_dir: boolean; children?: T[] }>(
  node: T,
): number {
  if (!node.is_dir) return 0
  return (node.children ?? []).reduce(
    (sum, child) => sum + (child.is_dir ? countFilesRecursive(child) : 1),
    0,
  )
}

/** Minimal shape required by flattenTree / generateUniqueName. */
interface Flattenable {
  path: string
  children?: Flattenable[]
}

function walk<T extends Flattenable>(nodes: T[], out: T[]): void {
  for (const node of nodes) {
    out.push(node)
    if (node.children) walk(node.children as T[], out)
  }
}

export function flattenTree<T extends Flattenable>(nodes: T[]): T[] {
  const out: T[] = []
  walk(nodes, out)
  return out
}

/**
 * S3: detect circular drag-and-drop moves.
 *
 * Returns true when moving `fromDir` to `toDir` would create a folder that
 * is itself, or is a descendant of itself. Used by FileTree to reject
 * invalid drop targets before issuing the move API call.
 *
 * @param fromDir the folder's current path (e.g. `"brain"`)
 * @param toDir   the proposed destination (e.g. `"brain/rust"`)
 */
export function isCircularMove(fromDir: string, toDir: string): boolean {
  if (fromDir === toDir) return true
  return toDir.startsWith(`${fromDir}/`)
}

export function generateUniqueName<T extends Flattenable>(
  entries: T[],
  basePath: string,
  defaultName: string,
): string {
  // Collision check is scoped to `basePath` (e.g. "brain/"). Existing paths
  // are matched by their relative form inside that scope, not their
  // absolute root key.
  const normalizedBase = basePath && !basePath.endsWith('/') ? `${basePath}/` : basePath
  const existing = new Set(
    flattenTree(entries)
      .filter((n) => n.path.startsWith(normalizedBase))
      .map((n) => n.path.slice(normalizedBase.length)),
  )
  const basename = defaultName
  if (!existing.has(basename)) return defaultName

  const dot = basename.lastIndexOf('.')
  const stem = dot > 0 ? basename.slice(0, dot) : basename
  const ext = dot > 0 ? basename.slice(dot) : ''

  let i = 2
  while (existing.has(`${stem} ${i}${ext}`)) i++
  return `${stem} ${i}${ext}`
}
