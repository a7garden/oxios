// Reactions persistence — per-message emoji reactions, localStorage-backed.
//
// Scope: single-user desktop app. Reactions are per-browser, ephemeral across
// reinstalls (localStorage scope). If Oxios ever ships multi-channel sync,
// this module is the single point to swap to a backend API.
//
// Data shape: { [messageId]: { [emoji]: true } }. We store toggled-on
// emojis per message; absence = no reaction. This keeps the payload tiny
// and the toggle logic O(1).

const STORAGE_KEY = 'oxios:message-reactions'

type ReactionsMap = Record<string, Record<string, true>>

function readAll(): ReactionsMap {
  if (typeof window === 'undefined') return {}
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return parsed && typeof parsed === 'object' ? (parsed as ReactionsMap) : {}
  } catch {
    return {}
  }
}

function writeAll(map: ReactionsMap): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
  } catch {
    /* quota exceeded — silently drop */
  }
}

/** Get the reaction set for a single message (emoji → true map). */
export function getReactions(messageId: string): Record<string, true> {
  return readAll()[messageId] ?? {}
}

/** Toggle a reaction on a message. Returns the new reaction set. */
export function toggleReaction(messageId: string, emoji: string): Record<string, true> {
  const map = readAll()
  const current = { ...(map[messageId] ?? {}) }
  if (current[emoji]) {
    delete current[emoji]
  } else {
    current[emoji] = true
  }
  if (Object.keys(current).length === 0) {
    delete map[messageId]
  } else {
    map[messageId] = current
  }
  writeAll(map)
  return current
}

/** Aggregate summary: [{ emoji, count }] sorted by count desc. */
export function listReactions(messageId: string): { emoji: string; count: number }[] {
  const set = getReactions(messageId)
  return Object.keys(set)
    .map((emoji) => ({ emoji, count: 1 }))
    .sort((a, b) => b.count - a.count)
}
