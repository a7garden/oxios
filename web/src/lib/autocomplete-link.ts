/**
 * Wiki link autocomplete data — shared between editor and code completers.
 *
 * No direct CodeMirror dependency. The data shape is consumed by:
 *   - markdown-editor.tsx (CM6 CompletionSource)
 *   - any future custom UI
 *
 * The original CM5 `CodeMirror.Hint` interface has been replaced by a
 * simple data structure (FileEntry) and the consumer is responsible
 * for adapting it to the active editor's completion API.
 */
import type { KnowledgeTreeEntry } from '@/types/knowledge'

/** A file entry with computed autocomplete key. */
export interface FileEntry {
  key: string // filename without .md
  filePath: string // full path for insertion
}

/**
 * Walk tree entries recursively to build a flat file list.
 * Filters out system dirs (media, archive, .hidden) and config files.
 */
export function buildAutocompleteDict(
  rootEntries: KnowledgeTreeEntry[],
  subDirEntries?: Map<string, KnowledgeTreeEntry[]>,
  currentPath?: string,
): FileEntry[] {
  const entries: FileEntry[] = []
  const systemDirs = new Set(['media', 'archive', '.config', 'insights'])

  function walk(items: KnowledgeTreeEntry[], parentDir: string) {
    for (const item of items) {
      if (item.name.startsWith('.')) continue

      if (item.is_dir) {
        if (systemDirs.has(item.name)) continue
        const subEntries = subDirEntries?.get(item.name)
        if (subEntries) {
          walk(subEntries, item.name)
        }
        continue
      }

      if (item.name === 'config.json') continue
      if (!item.name.endsWith('.md')) continue

      const fullPath = parentDir ? `${parentDir}/${item.name}` : item.name
      if (fullPath === currentPath) continue

      entries.push({
        key: item.name.replace(/\.md$/, ''),
        filePath: fullPath,
      })
    }
  }
  walk(rootEntries, '')
  return entries
}
