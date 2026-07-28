// Search panel store — manual search results, browse cache, UI expansion state.
//
// The portal store owns stack navigation (push/pop view). This store owns
// the data that the SearchView displays: manually submitted search queries
// (via POST /api/search), cached browse results (via POST /api/browse),
// and per-card expand state.

import { create } from 'zustand'

// ── Types ──

export interface SearchResultItem {
  title: string
  url: string
  snippet: string
  engine: string
}

export interface BrowseResult {
  url: string
  title: string
  markdown: string
  status: number
  elapsed_ms?: number
}

interface SearchResponse {
  results: SearchResultItem[]
  elapsed_ms: number
}

interface BrowseResponse {
  url: string
  title: string
  markdown: string
  status: number
  elapsed_ms: number
}

// ── Store ──

export interface SearchPanelState {
  // Manual search state
  manualQuery: string
  manualResults: SearchResultItem[]
  manualLoading: boolean
  manualError: string | null

  // Browse cache (URL → content, survives panel close)
  browseCache: Record<string, BrowseResult>
  browseLoading: Record<string, boolean>
  browseError: Record<string, string | null>

  // UI state
  expandedUrls: Set<string>

  // Actions
  search: (query: string) => Promise<void>
  browse: (url: string) => Promise<void>
  toggleExpand: (url: string) => void
  saveToKnowledge: (url: string, title: string, content: string) => Promise<void>
  reset: () => void
}

export const useSearchPanelStore = create<SearchPanelState>((set, get) => ({
  manualQuery: '',
  manualResults: [],
  manualLoading: false,
  manualError: null,

  browseCache: {},
  browseLoading: {},
  browseError: {},

  expandedUrls: new Set<string>(),

  search: async (query: string) => {
    if (!query.trim()) return
    set({ manualQuery: query, manualLoading: true, manualError: null })
    try {
      const res = await fetch('/api/search', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query, engines: 'ddg,wiki', limit: 10 }),
      })
      if (!res.ok) throw new Error(`Search failed: ${res.status}`)
      const data: SearchResponse = await res.json()
      set({ manualResults: data.results, manualLoading: false })
    } catch (e) {
      set({ manualError: (e as Error).message, manualLoading: false })
    }
  },

  browse: async (url: string) => {
    const cached = get().browseCache[url]
    if (cached) return // already loaded

    set((s) => ({
      browseLoading: { ...s.browseLoading, [url]: true },
      browseError: { ...s.browseError, [url]: null },
    }))

    try {
      const res = await fetch('/api/browse', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url, format: 'markdown' }),
      })
      if (!res.ok) throw new Error(`Browse failed: ${res.status}`)
      const data: BrowseResponse = await res.json()
      set((s) => ({
        browseCache: { ...s.browseCache, [url]: data },
        browseLoading: { ...s.browseLoading, [url]: false },
      }))
    } catch (e) {
      set((s) => ({
        browseError: { ...s.browseError, [url]: (e as Error).message },
        browseLoading: { ...s.browseLoading, [url]: false },
      }))
    }
  },

  toggleExpand: (url: string) => {
    set((s) => {
      const next = new Set(s.expandedUrls)
      if (next.has(url)) next.delete(url)
      else next.add(url)
      return { expandedUrls: next }
    })
  },

  saveToKnowledge: async (url: string, title: string, content: string) => {
    try {
      const path = `web-clippings/${title.replace(/[^a-zA-Z0-9가-힣]/g, '_').slice(0, 50)}.md`
      const body = `# ${title}\n\n> Source: ${url}\n\n${content}`
      await fetch('/api/knowledge/file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path, content: body }),
      })
    } catch (e) {
      console.error('Failed to save to knowledge:', e)
    }
  },

  reset: () => {
    set({
      manualQuery: '',
      manualResults: [],
      manualLoading: false,
      manualError: null,
      expandedUrls: new Set<string>(),
    })
  },
}))
