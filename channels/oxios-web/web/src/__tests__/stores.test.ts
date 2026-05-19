import { describe, it, expect, beforeEach } from 'vitest'
import { useAuthStore } from '@/stores/auth'
import { useSidebarStore } from '@/stores/sidebar'

describe('useAuthStore', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('starts unauthenticated when no token', () => {
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(false)
    expect(state.token).toBeNull()
  })

  it('sets token and authenticates', () => {
    useAuthStore.getState().setToken('test-key')
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(true)
    expect(state.token).toBe('test-key')
    expect(localStorage.getItem('oxios-api-key')).toBe('test-key')
  })

  it('logout clears token', () => {
    useAuthStore.getState().setToken('test-key')
    useAuthStore.getState().logout()
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(false)
    expect(state.token).toBeNull()
    expect(localStorage.getItem('oxios-api-key')).toBeNull()
  })

  it('setToken(null) clears authentication', () => {
    useAuthStore.getState().setToken('test-key')
    useAuthStore.getState().setToken(null)
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
  })
})

describe('useSidebarStore', () => {
  beforeEach(() => {
    localStorage.clear()
    useSidebarStore.setState({ collapsed: false, mobileOpen: false })
  })

  it('toggles collapsed state', () => {
    expect(useSidebarStore.getState().collapsed).toBe(false)
    useSidebarStore.getState().toggle()
    expect(useSidebarStore.getState().collapsed).toBe(true)
    useSidebarStore.getState().toggle()
    expect(useSidebarStore.getState().collapsed).toBe(false)
  })

  it('sets mobile open state', () => {
    useSidebarStore.getState().setMobileOpen(true)
    expect(useSidebarStore.getState().mobileOpen).toBe(true)
    useSidebarStore.getState().setMobileOpen(false)
    expect(useSidebarStore.getState().mobileOpen).toBe(false)
  })
})
