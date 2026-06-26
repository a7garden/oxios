/** Token Maxing (RFC-031) — types for the live status panel + session report.
 *
 * Mirrors `oxios_kernel::token_maxing::*` and the `/api/token-maxing/*`
 * routes. The Rust serde enum uses `#[serde(tag = "state", rename_all =
 * "snake_case")]`, so an `Available` variant serializes as
 * `{ "state": "available", "snapshot": {...}, "min_remaining_percent": N }`.
 * The unit variant `Ineligible` serializes as the bare string `"ineligible"`.
 *
 * `normalizeAvailability()` accepts the internally-tagged form (real wire),
 * the externally-tagged form (`{ "Available": {...} }`, etc., per the RFC
 * description), and the bare string unit variant — so the UI is correct
 * against the running backend AND tolerates the variations the task notes.
 */

import type { RateWindow } from './cost'

// ── Provider snapshot ────────────────────────────────────────

/** Counter + reset state for one provider (RFC-031 §4). */
export interface ProviderSnapshot {
  /** Self-tracked tokens used in the current window. */
  tokens_used: number
  /** Plan allocation per window (the subscription cap). */
  token_limit: number
  /** Window length in seconds. */
  window_secs: number
  /** When the current window resets. */
  resets_at: string | null
  /** Remaining percent (0–100). Null if unknown. */
  remaining_percent: number | null
  /** Last recalibration that snapped this counter, if any. */
  recalibrated_at: string | null
  /** Last observed rate-window headers from the provider (refinement). */
  rate_windows: RateWindow[]
}

// ── Availability verdict ─────────────────────────────────────

/** One branch of `Availability` from the Rust enum. Defensive union:
 *  the variant payload fields are always present even though Rust only
 *  attaches them to the relevant branch.
 */
export interface Availability {
  /** Which branch this is: "available" | "draining" | "cooled_down". */
  state: 'available' | 'draining' | 'cooled_down'
  /** Snapshot when known (all branches except sometimes cooled_down). */
  snapshot: ProviderSnapshot | null
  /** Floor percentage; only meaningful for Available/Draining. */
  min_remaining_percent: number | null
  /** Cool-down expiry (only for `cooled_down`). */
  until: string | null
  /** Class that triggered cooldown (`429` / `quota_exhausted` / ...). */
  reason: string | null
}

/** The unit variant — a provider with no `[token-maxing.providers]` entry.
 *  Serializes as the bare string `"ineligible"` from Rust.
 */
export type IneligibleAvailability = 'ineligible'

/** Per-provider tracker snapshot. `availability` is the union of the
 *  rich object and the bare string unit variant.
 */
export interface QuotaTrackerSnapshot {
  provider: string
  availability: Availability | IneligibleAvailability
}

// ── Live status ──────────────────────────────────────────────

/** `GET /api/token-maxing/status` response. */
export interface MaxerStatus {
  running: boolean
  current_session_id: string | null
  current_provider: string | null
  current_task: string | null
  manual: boolean
  window: { start: string; end: string } | null
  tokens_this_session: number
  tasks_this_session: number
  providers: QuotaTrackerSnapshot[]
}

// ── Providers endpoint ───────────────────────────────────────

/** `GET /api/token-maxing/providers` response. */
export interface TokenMaxingProvidersResponse {
  enabled: boolean
  providers: QuotaTrackerSnapshot[]
  recalibrations: RecalibrationRecord[]
  cooldowns: CooldownRecord[]
}

export interface RecalibrationRecord {
  provider: string
  at: string
  remaining_percent: number | null
  resets_at: string | null
  /** "ok" | "fetch-failed" | "no-fetcher". */
  outcome: string
}

export interface CooldownRecord {
  provider: string
  since: string
  until: string
  reason: string
}

// ── Session report ──────────────────────────────────────────

/** A scheduled activation window. */
export interface MaxingWindow {
  start: string
  end: string
}

/** Source of a planned task (RFC-031 §7). */
export type TaskSource = 'skill' | 'project' | 'recurring'

export interface ProviderWindowRecord {
  started: string
  ended_at: string | null
}

export interface ProviderSessionRecord {
  provider: string
  models_used: string[]
  tasks_run: number
  tokens_consumed: number
  windows_drained: ProviderWindowRecord[]
}

export interface TaskRecord {
  source: TaskSource
  source_name: string
  goal: string
  provider: string
  model: string
  success: boolean
  tokens: number
  duration_secs: number
  summary: string
}

/** Why the session ended. */
export type StopReason = 'window_ended' | 'no_work' | 'cancelled' | null

export interface SessionTotals {
  tasks: number
  tokens: number
  providers_fully_drained: number
  resets_observed: number
}

/** `GET /api/token-maxing/sessions[/:id]` response. */
export interface TokenMaxingSession {
  id: string
  started_at: string
  ended_at: string | null
  window: MaxingWindow | null
  manual: boolean
  providers: ProviderSessionRecord[]
  tasks: TaskRecord[]
  totals: SessionTotals
  stop_reason: StopReason
}

// ── Start/stop payloads ──────────────────────────────────────

/** `POST /api/token-maxing/start` body. Either `window` or `manual: true`. */
export type StartRequest = { window: { start: string; end: string } } | { manual: true }

/** `POST /api/token-maxing/start` response. */
export interface StartResponse {
  session_id: string
}

/** `POST /api/token-maxing/stop` response. */
export interface StopResponse {
  stopped: boolean
}

// ── Normalizer ───────────────────────────────────────────────

/** Korean verdict label for a provider availability state. */
export type AvailabilityVerdict = 'available' | 'draining' | 'cooled_down' | 'ineligible'

export interface NormalizedAvailability {
  verdict: AvailabilityVerdict
  /** Snapshot if the backend had one (null for ineligible, sometimes for cooled_down). */
  snapshot: ProviderSnapshot | null
  /** Floor %. Set for Available/Draining. */
  min_remaining_percent: number | null
  /** Cool-down expiry (set only for CooledDown). */
  until: string | null
  /** The FailureClass that triggered cooldown (string form). */
  reason: string | null
}

const INELIGIBLE: NormalizedAvailability = {
  verdict: 'ineligible',
  snapshot: null,
  min_remaining_percent: null,
  until: null,
  reason: null,
}

/** Defensive normalizer — the Rust enum is internally tagged with
 *  `#[serde(tag="state", rename_all="snake_case")]`, so the real wire
 *  format is `{ "state": "available" | "draining" | "cooled_down" |
 *  "ineligible", ... }`. The RFC description hints at an externally
 *  tagged form `{ "Available": {...} }` and a bare string `"Ineligible"`.
 *  Accept any of those three shapes so the UI is forward/backward
 *  compatible.
 */
export function normalizeAvailability(input: unknown): NormalizedAvailability {
  if (typeof input === 'string') {
    // Bare unit variant: `"ineligible"` (or `"Ineligible"`). Unknown strings
    // are coerced to ineligible rather than dropped.
    return INELIGIBLE
  }

  if (!input || typeof input !== 'object') {
    return INELIGIBLE
  }

  const obj = input as Record<string, unknown>

  // (1) Internally tagged — the real wire format from `#[serde(tag="state")]`.
  if ('state' in obj) {
    const stateRaw = String(obj.state ?? '').toLowerCase()
    if (stateRaw === 'ineligible') {
      return INELIGIBLE
    }
    if (stateRaw === 'available' || stateRaw === 'draining' || stateRaw === 'cooled_down') {
      return {
        verdict: stateRaw,
        snapshot: (obj.snapshot as ProviderSnapshot | null | undefined) ?? null,
        min_remaining_percent: (obj.min_remaining_percent as number | null | undefined) ?? null,
        until: (obj.until as string | null | undefined) ?? null,
        reason: stringifyReason(obj.reason),
      }
    }
    // Unknown state — fall through to outer-key probing.
  }

  // (2) Externally tagged — `{ "Available": {...} }`, `{ "CooledDown": {...} }`,
  //     or `{ "Ineligible": {} }`. Probe PascalCase + snake_case keys.
  const keys = Object.keys(obj)
  for (const key of keys) {
    const lower = key.toLowerCase()
    if (lower === 'available') {
      const payload = (obj[key] as Record<string, unknown> | null) ?? {}
      return {
        verdict: 'available',
        snapshot: (payload.snapshot as ProviderSnapshot | null | undefined) ?? null,
        min_remaining_percent: (payload.min_remaining_percent as number | null | undefined) ?? null,
        until: null,
        reason: null,
      }
    }
    if (lower === 'draining') {
      const payload = (obj[key] as Record<string, unknown> | null) ?? {}
      return {
        verdict: 'draining',
        snapshot: (payload.snapshot as ProviderSnapshot | null | undefined) ?? null,
        min_remaining_percent: (payload.min_remaining_percent as number | null | undefined) ?? null,
        until: null,
        reason: null,
      }
    }
    if (lower === 'cooleddown' || lower === 'cooled_down') {
      const payload = (obj[key] as Record<string, unknown> | null) ?? {}
      return {
        verdict: 'cooled_down',
        snapshot: (payload.snapshot as ProviderSnapshot | null | undefined) ?? null,
        min_remaining_percent: null,
        until: (payload.until as string | null | undefined) ?? null,
        reason: stringifyReason(payload.reason),
      }
    }
    if (lower === 'ineligible') {
      return INELIGIBLE
    }
  }

  // Last resort — unknown shape, surface as ineligible rather than crash.
  return INELIGIBLE
}

/** `FailureClass` is itself a serde enum. Defensively render any nested
 *  tagged object back to a readable label (`rate_limited` / `quota_exhausted` / ...).
 */
function stringifyReason(reason: unknown): string | null {
  if (reason == null) return null
  if (typeof reason === 'string') return reason
  if (typeof reason !== 'object') return String(reason)
  const obj = reason as Record<string, unknown>
  // Externally-tagged form: `{ "QuotaExhausted": {} }`.
  const keys = Object.keys(obj)
  if (keys.length === 1) {
    const k = keys[0]
    if (k === undefined) return null
    const inner = obj[k]
    if (inner && typeof inner === 'object' && Object.keys(inner).length > 0) {
      return `${k}: ${JSON.stringify(inner)}`
    }
    return k
  }
  // Internally-tagged form: `{ "kind": "rate_limited", ... }`.
  if ('kind' in obj) return String(obj.kind)
  return JSON.stringify(obj)
}
