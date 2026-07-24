// Input history — terminal-style prompt history navigated with ArrowUp/Down.
//
// LobeHub analogue: features/ChatInput/inputHistoryStorage.ts. Oxios version
// is single-user so the key is simpler (no user/agent scoping needed).

const STORAGE_KEY = 'oxios:chat-input-history'
const MAX_ITEMS = 50

export function addInputHistory(text: string): void {
  const trimmed = text.trim()
  if (!trimmed) return
  if (typeof window === 'undefined') return
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    const items: string[] = raw ? JSON.parse(raw) : []
    // Dedupe: remove any existing identical entry, then prepend.
    const filtered = items.filter((x) => x !== trimmed)
    filtered.unshift(trimmed)
    const capped = filtered.slice(0, MAX_ITEMS)
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(capped))
  } catch {
    /* ignore */
  }
}

export function getInputHistory(): string[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

export function clearInputHistory(): void {
  if (typeof window === 'undefined') return
  window.localStorage.removeItem(STORAGE_KEY)
}
