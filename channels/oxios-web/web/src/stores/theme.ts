import { create } from 'zustand'

type Theme = 'dark' | 'light' | 'system'

interface ThemeState {
  theme: Theme
  resolved: 'dark' | 'light'
  setTheme: (theme: Theme) => void
}

function getSystemTheme(): 'dark' | 'light' {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function applyTheme(resolved: 'dark' | 'light') {
  document.documentElement.classList.toggle('dark', resolved === 'dark')
}

function resolveTheme(theme: Theme): 'dark' | 'light' {
  return theme === 'system' ? getSystemTheme() : theme
}

const saved = (localStorage.getItem('oxios-theme') as Theme) || 'dark'
const resolved = resolveTheme(saved)
applyTheme(resolved)

export const useThemeStore = create<ThemeState>((set) => ({
  theme: saved,
  resolved,
  setTheme: (theme) => {
    const r = resolveTheme(theme)
    localStorage.setItem('oxios-theme', theme)
    applyTheme(r)
    set({ theme, resolved: r })
  },
}))

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  const state = useThemeStore.getState()
  if (state.theme === 'system') {
    const r = getSystemTheme()
    applyTheme(r)
    useThemeStore.setState({ resolved: r })
  }
})
