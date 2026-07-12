/**
 * useFlattenedVisibleTree (Phase 5 / S5)
 *
 * Flatten a recursive tree into the **currently visible** node list, in
 * pre-order, honoring the expanded-paths set. Used by keyboard navigation
 * (Arrow Up/Down, Home/End) which needs to step through the items the user
 * actually sees — not the hidden descendants of collapsed folders.
 *
 * Returns `[]` when the input is null/undefined.
 */
import { useMemo } from 'react'
import type { KnowledgeTreeNode } from '@/types/knowledge'

export interface VisibleTreeItem {
  node: KnowledgeTreeNode
  depth: number
}

export function useFlattenedVisibleTree(
  nodes: KnowledgeTreeNode[] | null | undefined,
  expandedPaths: string[],
): VisibleTreeItem[] {
  return useMemo(() => {
    if (!nodes || nodes.length === 0) return []
    const expanded = new Set(expandedPaths)
    const out: VisibleTreeItem[] = []
    const walk = (items: KnowledgeTreeNode[], depth: number): void => {
      for (const node of items) {
        out.push({ node, depth })
        if (node.is_dir && expanded.has(node.path)) {
          walk(node.children, depth + 1)
        }
      }
    }
    walk(nodes, 0)
    return out
  }, [nodes, expandedPaths])
}
