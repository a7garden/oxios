import { create } from 'zustand'

interface AuthState {
  token: string | null
  isAuthenticated: boolean
  setToken: (token: string | null) => void
  logout: () => void
}

export const useAuthStore = create<AuthState>((set) => ({
  token: localStorage.getItem('oxios-api-key') || null,
  isAuthenticated: !!localStorage.getItem('oxios-api-key'),
  setToken: (token) => {
    if (token) {
      localStorage.setItem('oxios-api-key', token)
    } else {
      localStorage.removeItem('oxios-api-key')
    }
    set({ token, isAuthenticated: !!token })
  },
  logout: () => {
    localStorage.removeItem('oxios-api-key')
    set({ token: null, isAuthenticated: false })
  },
}))