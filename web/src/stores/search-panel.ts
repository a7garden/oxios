// Search panel store — manual search results, browse cache, UI expansion state.
//
// The portal store owns stack navigation (push/pop view). This store owns
// the data that the SearchView displays: manually submitted search queries
// (via POST /api/search), cached browse results (via POST /api/browse),
// and per-card expand state.

import { create } from 'zustand'
import type { KnowledgeSearchHit } from '@/types/knowledge'

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

  // Knowledge tab state
  activeTab: 'web' | 'knowledge'
  knowledgeResults: KnowledgeSearchHit[]
  knowledgeLoading: boolean
  knowledgeError: string | null
  selectedKnowledgePath: string | null
  selectedKnowledgeContent: string | null
  selectedKnowledgeLoading: boolean

  // Save to Knowledge modal
  saveModalOpen: boolean
  saveUrl: string
  saveTitle: string
  saveContent: string
  savePath: string
  saveLoading: boolean
  saveError: string | null

  // Actions
  search: (query: string) => Promise<void>
  browse: (url: string) => Promise<void>
  toggleExpand: (url: string) => void
  saveToKnowledge: (url: string, title: string, content: string) => Promise<void>
  setActiveTab: (tab: 'web' | 'knowledge') => void
  searchKnowledge: (query: string) => Promise<void>
  selectKnowledge: (path: string) => Promise<void>
  openSaveModal: (url: string, title: string, content: string) => void
  closeSaveModal: () => void
  saveModalSave: () => Promise<void>
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

  activeTab: 'web',
  knowledgeResults: [],
  knowledgeLoading: false,
  knowledgeError: null,
  selectedKnowledgePath: null,
  selectedKnowledgeContent: null,
  selectedKnowledgeLoading: false,

  saveModalOpen: false,
  saveUrl: '',
  saveTitle: '',
  saveContent: '',
  savePath: '',
  saveLoading: false,
  saveError: null,

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

  setActiveTab: (tab) => {
    set({ activeTab: tab })
  },

  searchKnowledge: async (query) => {
    if (!query.trim()) {
      set({ knowledgeResults: [], knowledgeError: null })
      return
    }
    set({ knowledgeLoading: true, knowledgeError: null })
    try {
      const res = await fetch('/api/knowledge/search', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query, limit: 50 }),
      })
      if (!res.ok) throw new Error(`Knowledge search failed: ${res.status}`)
      const data = await res.json()
      set({ knowledgeResults: data.results as KnowledgeSearchHit[], knowledgeLoading: false })
    } catch (e) {
      set({ knowledgeError: (e as Error).message, knowledgeLoading: false })
    }
  },

  selectKnowledge: async (path) => {
    set({
      selectedKnowledgePath: path,
      selectedKnowledgeLoading: true,
      selectedKnowledgeContent: null,
    })
    try {
      const encoded = path
        .split('/')
        .map((seg) => encodeURIComponent(seg))
        .join('/')
      const res = await fetch(`/api/knowledge/file/${encoded}`)
      if (!res.ok) throw new Error(`Read failed: ${res.status}`)
      const data = await res.json()
      set({ selectedKnowledgeContent: data.content, selectedKnowledgeLoading: false })
    } catch (e) {
      set({ selectedKnowledgeLoading: false })
    }
  },

  openSaveModal: (url, title, content) => {
    let domain = 'web'
    try {
      domain = new URL(url).hostname
    } catch {
      /* keep default */
    }
    const date = new Date().toISOString().slice(0, 10)
    const slug = title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/(^-|-$)/g, '')
      .slice(0, 40) || 'page'
    set({
      saveModalOpen: true,
      saveUrl: url,
      saveTitle: title,
      saveContent: content,
      savePath: `web-clippings/${domain}/${date}-${slug}.md`,
      saveError: null,
    })
  },

  closeSaveModal: () => {
    set({ saveModalOpen: false })
  },

  saveModalSave: async () => {
    const state = get()
    if (!state.saveTitle.trim() || !state.savePath.trim()) return
    set({ saveLoading: true, saveError: null })
    try {
      const body = `# ${state.saveTitle}\n> **Source:** [${state.saveUrl}](${state.saveUrl})\n> **Saved:** ${new Date().toISOString().slice(0, 10)}\n\n${state.saveContent}`
      const res = await fetch('/api/knowledge/file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: state.savePath, content: body }),
      })
      if (!res.ok) throw new Error(`Save failed: ${res.status}`)
      set({ saveLoading: false, saveModalOpen: false })
    } catch (e) {
      set({ saveError: (e as Error).message, saveLoading: false })
    }
  },

  reset: () => {
    set({
      manualQuery: '',
      manualResults: [],
      manualLoading: false,
      manualError: null,
      expandedUrls: new Set<string>(),
      activeTab: 'web',
      knowledgeResults: [],
      knowledgeLoading: false,
      knowledgeError: null,
      selectedKnowledgePath: null,
      selectedKnowledgeContent: null,
      selectedKnowledgeLoading: false,
      saveModalOpen: false,
    })
  },
}))
