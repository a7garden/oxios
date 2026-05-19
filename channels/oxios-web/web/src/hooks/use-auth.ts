import { useAuthStore } from '@/stores/auth'

export function useAuth() {
  const { token, isAuthenticated, setToken, logout } = useAuthStore()
  return { token, isAuthenticated, setToken, logout }
}