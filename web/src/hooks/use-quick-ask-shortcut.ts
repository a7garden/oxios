import { useEffect } from 'react'
import { useQuickAskStore } from '@/stores/quick-ask'

/**
 * Registers the global ⌘J shortcut that opens the QuickAsk dialog from any
 * route. Mounted once in AppLayout alongside the other global hooks.
 */
export function useQuickAskShortcut(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'j') {
        e.preventDefault()
        useQuickAskStore.getState().openQuickAsk()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
}
