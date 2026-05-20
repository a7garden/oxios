import type CodeMirror from 'codemirror'
import type { KnowledgeTreeEntry } from '@/types/knowledge'

/** A file entry with computed autocomplete key. */
interface FileEntry {
  key: string       // filename without .md
  filePath: string  // full path for insertion
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
        // Try to get subdirectory entries if available
        const subEntries = subDirEntries?.get(item.name)
        if (subEntries) {
          walk(subEntries, item.name)
        }
        continue
      }

      // File
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

/**
 * Create a CodeMirror hint function for file link autocompletion.
 * Call this once and pass to CM editor options.
 */
export function createLinkHintFn(
  getEntries: () => FileEntry[],
): (cm: CodeMirror.Editor) => CodeMirror.Hints | null {
  return (cm) => {
    const cursor = cm.getCursor()
    const line = cm.getLine(cursor.line)
    const pos = cursor.ch

    // Only trigger after '['
    if (pos === 0 || line[pos - 1] !== '[') return null

    // Don't trigger inside checkboxes or code blocks
    if (/^\s*-\s\[/.test(line)) return null

    // Get the word being typed after '['
    const unicodeWordRegex = /[\p{L}\p{N}_\s:-]/u
    let start = pos
    while (start < line.length && unicodeWordRegex.test(line[start])) start++

    const word = line.slice(pos, start).toLowerCase()
    const entries = getEntries()

    const list: CodeMirror.Hint[] = []
    for (const entry of entries) {
      if (word.length === 0 || entry.key.toLowerCase().includes(word)) {
        const displayText = entry.key
        const text = `${entry.key}](${entry.filePath.replace(/ /g, '%20')})`
        list.push({ text, displayText })
        if (list.length >= 20) break
      }
    }

    if (list.length === 0) return null

    return {
      list,
      from: { line: cursor.line, ch: pos },
      to: { line: cursor.line, ch: start },
    }
  }
}