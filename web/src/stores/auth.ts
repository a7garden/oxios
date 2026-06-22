import { create } from 'zustand'

interface AuthState {
  token: string | null
  isAuthenticated: boolean
  setToken: (token: string | null) => void
  logout: () => void
}

// F2: API token stored in sessionStorage (not localStorage) to limit XSS
// exposure — the token is scoped to the tab and cleared on tab close rather
// than persisting across browser restarts. A one-time migration moves any
// pre-existing localStorage token so existing sessions survive the upgrade.
const TOKEN_KEY = 'oxios-api-key'

function readToken(): string | null {
  const fromSession = sessionStorage.getItem(TOKEN_KEY)
  if (fromSession) return fromSession
  // Migrate legacy localStorage token → sessionStorage, then wipe localStorage.
  const legacy = localStorage.getItem(TOKEN_KEY)
  if (legacy) {
    sessionStorage.setItem(TOKEN_KEY, legacy)
    localStorage.removeItem(TOKEN_KEY)
    return legacy
  }
  return null
}

export const useAuthStore = create<AuthState>((set) => ({
  token: readToken(),
  isAuthenticated: readToken() !== null,
  setToken: (token) => {
    if (token) {
      sessionStorage.setItem(TOKEN_KEY, token)
    } else {
      sessionStorage.removeItem(TOKEN_KEY)
    }
    // Always wipe any legacy localStorage copy.
    localStorage.removeItem(TOKEN_KEY)
    set({ token, isAuthenticated: !!token })
  },
  logout: () => {
    sessionStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(TOKEN_KEY)
    set({ token: null, isAuthenticated: false })
  },
}))
