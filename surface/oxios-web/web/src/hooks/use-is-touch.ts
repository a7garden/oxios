import { useEffect, useState } from 'react'

/**
 * Returns `true` when the primary input mechanism includes a touch screen.
 * Uses `matchMedia('(pointer: coarse)')` and falls back to `ontouchstart`
 * detection for environments where the media query isn't supported.
 */
export function useIsTouch(): boolean {
  const [isTouch, setIsTouch] = useState(() => {
    if (typeof window === 'undefined') return false
    const mq = window.matchMedia('(pointer: coarse)')
    if (mq.matches) return true
    return 'ontouchstart' in window || navigator.maxTouchPoints > 0
  })

  useEffect(() => {
    const mq = window.matchMedia('(pointer: coarse)')
    const handler = (e: MediaQueryListEvent) => setIsTouch(e.matches)
    mq.addEventListener('change', handler)
    return () => mq.removeEventListener('change', handler)
  }, [])

  return isTouch
}
