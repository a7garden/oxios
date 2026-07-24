// Draft persistence — per-session input draft saved to localStorage.
//
// LobeHub analogue: features/ChatInput/draftStorage.ts. Oxios stores a plain
// string (not editor JSON) because the chat input value is a simple string.
//
// The draft is keyed by sessionId. On session switch the hook restores the
// saved draft; on send it is cleared. This prevents losing half-typed prompts
// when navigating between sessions.

const STORAGE_KEY = 'oxios:chat-drafts'
const MAX_DRAFTS = 50

type DraftMap = Record<string, string>

function readAll(): DraftMap {
  if (typeof window === 'undefined') return {}
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return parsed && typeof parsed === 'object' ? (parsed as DraftMap) : {}
  } catch {
    return {}
  }
}

function writeAll(map: DraftMap): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
  } catch {
    /* quota exceeded — silently drop */
  }
}

/** Save (or overwrite) the draft for a session. Empty values remove it. */
export function saveDraft(sessionId: string, value: string): void {
  const map = readAll()
  if (!value) {
    delete map[sessionId]
  } else {
    map[sessionId] = value
  }
  // Evict oldest entries if over the cap (oldest = first inserted; we can't
  // track insertion order in a plain object reliably, so just cap by count).
  const keys = Object.keys(map)
  if (keys.length > MAX_DRAFTS) {
    for (const k of keys.slice(0, keys.length - MAX_DRAFTS)) delete map[k]
  }
  writeAll(map)
}

/** Load the draft for a session (empty string if none). */
export function loadDraft(sessionId: string): string {
  return readAll()[sessionId] ?? ''
}

/** Remove the draft for a session (called on send). */
export function removeDraft(sessionId: string): void {
  const map = readAll()
  delete map[sessionId]
  writeAll(map)
}

/** Whether a non-empty draft exists for a session. */
export function hasDraft(sessionId: string): boolean {
  return !!readAll()[sessionId]?.trim()
}
