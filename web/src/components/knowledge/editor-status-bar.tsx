/**
 * Bottom status bar for the knowledge-base markdown editor.
 *
 * Displays document statistics (word count, character count, line count)
 * and cursor position (Ln, Col). CJK characters are counted individually
 * as words; Latin words are whitespace-delimited.
 *
 * Visibility is controlled by `useEditorPrefs.showStatusBar`.
 */
import { useTranslation } from 'react-i18next'
import { useEditorPrefs } from '@/stores/editor-prefs'

export interface EditorStats {
  words: number
  chars: number
  lines: number
  cursorLine: number
  cursorCol: number
}

/**
 * Count words in mixed CJK / Latin text.
 *
 * CJK characters (CJK Unified Ideographs, Hiragana, Katakana, Hangul)
 * each count as one word. Latin words are whitespace-delimited tokens
 * remaining after CJK characters are stripped.
 */
export function countWords(text: string): number {
  const cjkRe = /[\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff\uac00-\ud7af]/g
  const cjkCount = (text.match(cjkRe) || []).length
  const latinText = text.replace(cjkRe, ' ')
  const latinCount = (latinText.trim().match(/\S+/g) || []).length
  return cjkCount + latinCount
}

export function EditorStatusBar({ stats }: { stats: EditorStats | null }) {
  const { t } = useTranslation()
  const show = useEditorPrefs((s) => s.showStatusBar)

  if (!show) return null

  const s = stats ?? { words: 0, chars: 0, lines: 0, cursorLine: 0, cursorCol: 0 }

  return (
    <div className="flex items-center gap-4 px-4 py-1 border-t bg-muted/30 text-xs text-muted-foreground min-h-[28px] tabular-nums select-none">
      <span>{t('knowledge.statusWords', { n: s.words.toLocaleString() })}</span>
      <span className="text-border">|</span>
      <span>{t('knowledge.statusChars', { n: s.chars.toLocaleString() })}</span>
      <span className="text-border">|</span>
      <span>{t('knowledge.statusLines', { n: s.lines.toLocaleString() })}</span>
      <div className="flex-1" />
      <span>{t('knowledge.statusCursor', { line: s.cursorLine, col: s.cursorCol })}</span>
    </div>
  )
}
