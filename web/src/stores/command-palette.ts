import { create } from 'zustand'

/**
 * Global command palette state.
 *
 * Kept in a store (not local component state) so any component can open the
 * palette programmatically — e.g. a future "명령 팔레트" button in the header.
 */
interface CommandPaletteState {
  open: boolean
  query: string
  openPalette: (initialQuery?: string) => void
  closePalette: () => void
  togglePalette: () => void
  setQuery: (query: string) => void
}

export const useCommandPaletteStore = create<CommandPaletteState>((set) => ({
  open: false,
  query: '',
  openPalette: (initialQuery) => set({ open: true, query: initialQuery ?? '' }),
  closePalette: () => set({ open: false }),
  togglePalette: () => set((s) => ({ open: !s.open, query: s.open ? s.query : '' })),
  setQuery: (query) => set({ query }),
}))
