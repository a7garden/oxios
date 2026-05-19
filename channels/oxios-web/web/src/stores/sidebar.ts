import { create } from 'zustand'

interface SidebarState {
  collapsed: boolean
  mobileOpen: boolean
  toggle: () => void
  setMobileOpen: (open: boolean) => void
}

export const useSidebarStore = create<SidebarState>((set) => ({
  collapsed: localStorage.getItem('oxios-sidebar-collapsed') === 'true',
  mobileOpen: false,
  toggle: () =>
    set((s) => {
      localStorage.setItem('oxios-sidebar-collapsed', String(!s.collapsed))
      return { collapsed: !s.collapsed }
    }),
  setMobileOpen: (open) => set({ mobileOpen: open }),
}))