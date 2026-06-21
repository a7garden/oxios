import { useMediaQuery } from './use-media-query'

export const useIsMobile = () => !useMediaQuery('(min-width: 768px)')
export const useIsTablet = () => {
  const md = useMediaQuery('(min-width: 768px)')
  const lg = useMediaQuery('(min-width: 1024px)')
  return md && !lg
}
export const useIsDesktop = () => useMediaQuery('(min-width: 1024px)')

export type DeviceTier = 'mobile' | 'tablet' | 'desktop'

/** 단일 구독으로 디바이스 티어 결정. */
export function useDevice(): DeviceTier {
  const isDesktop = useMediaQuery('(min-width: 1024px)')
  const isTablet = useMediaQuery('(min-width: 768px)')
  if (isDesktop) return 'desktop'
  if (isTablet) return 'tablet'
  return 'mobile'
}
