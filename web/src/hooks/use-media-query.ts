import { useEffect, useState } from 'react'

/**
 * SSR/CSR-safe media query hook.
 *
 * 첫 렌더 프레임은 항상 `false`를 반환한다 (CSR에서도).
 * 이는 FOUC를 의도적으로 수용하는 정책이다 — 레이아웃 전환은
 * CSS 프리픽스로 처리하고, 이 훅은 동작 분기(이벤트 핸들러 등)에만
 * 사용한다.
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(false)

  useEffect(() => {
    const mql = window.matchMedia(query)
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches)
    setMatches(mql.matches)
    mql.addEventListener('change', handler)
    return () => mql.removeEventListener('change', handler)
  }, [query])

  return matches
}
