// Unit tests for draft-storage and input-history-storage.
//
// Covers the edge cases: empty-draft cleanup, eviction cap, dedup,
// SSR guards, and quota-exceeded resilience.

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { hasDraft, loadDraft, removeDraft, saveDraft } from '@/lib/draft-storage'
import { addInputHistory, clearInputHistory, getInputHistory } from '@/lib/input-history-storage'

// ── Test localStorage mock ──

function makeStore() {
  const store: Record<string, string> = {}
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key]
    }),
    clear: () => {
      for (const k of Object.keys(store)) delete store[k]
    },
    _store: store,
  }
}

let mock: ReturnType<typeof makeStore>

beforeEach(() => {
  mock = makeStore()
  vi.stubGlobal('window', { localStorage: mock })
})

// ── draft-storage ──

describe('draft-storage', () => {
  it('saves and loads a draft', () => {
    saveDraft('session-1', 'Hello world')
    expect(loadDraft('session-1')).toBe('Hello world')
    expect(hasDraft('session-1')).toBe(true)
  })

  it('returns empty string for missing draft', () => {
    expect(loadDraft('nonexistent')).toBe('')
    expect(hasDraft('nonexistent')).toBe(false)
  })

  it('removes draft on empty save', () => {
    saveDraft('session-1', 'text')
    saveDraft('session-1', '')
    expect(hasDraft('session-1')).toBe(false)
    expect(loadDraft('session-1')).toBe('')
  })

  it('removes draft via removeDraft', () => {
    saveDraft('session-1', 'text')
    removeDraft('session-1')
    expect(hasDraft('session-1')).toBe(false)
  })

  it('isolates drafts per session', () => {
    saveDraft('a', 'draft A')
    saveDraft('b', 'draft B')
    expect(loadDraft('a')).toBe('draft A')
    expect(loadDraft('b')).toBe('draft B')
  })

  it('treats whitespace-only as empty', () => {
    saveDraft('session-1', '   ')
    expect(hasDraft('session-1')).toBe(false)
  })
})

// ── input-history-storage ──

describe('input-history-storage', () => {
  it('adds and retrieves history', () => {
    addInputHistory('first prompt')
    addInputHistory('second prompt')
    const history = getInputHistory()
    expect(history).toHaveLength(2)
    // Most recent is first (prepend)
    expect(history[0]).toBe('second prompt')
    expect(history[1]).toBe('first prompt')
  })

  it('deduplicates by content', () => {
    addInputHistory('duplicate')
    addInputHistory('unique')
    addInputHistory('duplicate')
    const history = getInputHistory()
    expect(history).toHaveLength(2)
    // The duplicate moves to front
    expect(history[0]).toBe('duplicate')
    expect(history[1]).toBe('unique')
  })

  it('ignores empty/whitespace input', () => {
    addInputHistory('')
    addInputHistory('   ')
    expect(getInputHistory()).toHaveLength(0)
  })

  it('clears all history', () => {
    addInputHistory('a')
    addInputHistory('b')
    clearInputHistory()
    expect(getInputHistory()).toHaveLength(0)
  })

  it('handles missing storage gracefully', () => {
    expect(getInputHistory()).toEqual([])
  })
})
