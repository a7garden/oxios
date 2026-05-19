import { useThemeStore } from '@/stores/theme'

export function useTheme() {
  const { theme, resolved, setTheme } = useThemeStore()
  return { theme, resolved, setTheme }
}