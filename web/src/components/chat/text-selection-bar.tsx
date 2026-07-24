// TextSelectionBar — floating "Copy" button that appears when the user selects
// text inside the messages area.
//
// LobeHub analogue: Messages/components/TextSelectionActionLayer.
// Oxios version: a lightweight component that listens for selection changes
// within a container ref and shows a Copy button at the selection's bounding rect.

import { Check, Copy } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

interface TextSelectionBarProps {
  /** Ref to the container that holds selectable message content. */
  containerRef: React.RefObject<HTMLElement | null>
}

export function TextSelectionBar({ containerRef }: TextSelectionBarProps) {
  const { t } = useTranslation()
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null)
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const checkSelection = useCallback(() => {
    const sel = window.getSelection()
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
      setPos(null)
      return
    }
    const container = containerRef.current
    if (!container) return
    // Only show if the selection is inside our messages container.
    const range = sel.getRangeAt(0)
    if (!container.contains(range.commonAncestorContainer)) {
      setPos(null)
      return
    }
    const text = sel.toString().trim()
    if (text.length < 2) {
      setPos(null)
      return
    }
    const rect = range.getBoundingClientRect()
    setPos({ x: rect.left + rect.width / 2, y: rect.top - 8 })
  }, [containerRef])
  useEffect(() => {
    document.addEventListener('selectionchange', checkSelection)
    return () => document.removeEventListener('selectionchange', checkSelection)
  }, [checkSelection])

  const handleCopy = () => {
    const text = window.getSelection()?.toString() ?? ''
    navigator.clipboard.writeText(text)
    setCopied(true)
    clearTimeout(timer.current!)
    timer.current = setTimeout(() => {
      setCopied(false)
      setPos(null)
    }, 1500)
  }

  if (!pos) return null

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="fixed z-50 flex -translate-x-1/2 -translate-y-full items-center gap-1 rounded-md border bg-popover px-2 py-1 text-xs shadow-md transition-colors hover:bg-accent"
      style={{ left: pos.x, top: pos.y }}
    >
      {copied ? (
        <>
          <Check className="size-3" />
          {t('common.copied')}
        </>
      ) : (
        <>
          <Copy className="size-3" />
          {t('common.copy')}
        </>
      )}
    </button>
  )
}
