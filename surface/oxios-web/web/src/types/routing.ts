// ─── Routing Types (RFC-011) ─────────────────────────────────
// Types for the model routing configuration and statistics.
// Mirrors the backend structs in kernel_handle/engine_api.rs.

/** Routing configuration snapshot (read from GET /api/engine/config). */
export interface RoutingConfig {
  /** Whether automatic model routing is enabled. */
  routingEnabled: boolean
  /** Whether cost-efficient models are preferred when routing. */
  preferCostEfficient: boolean
  /** Ordered list of fallback models. */
  fallbackModels: string[]
  /** Models excluded from automatic routing. */
  excludedModels: string[]
}

/** Model usage statistics (from GET /api/engine/routing/stats). */
export interface RoutingStats {
  /** Model ID → number of calls. */
  modelCalls: Record<string, number>
  /** Model ID → estimated total cost (USD). */
  modelCost: Record<string, number>
  /** Total number of requests. */
  totalRequests: number
  /** Total estimated cost (USD). */
  totalCost: number
}

/** Single fallback event record. */
export interface FallbackEvent {
  /** ISO timestamp when fallback occurred. */
  timestamp: string
  /** Model that was skipped/replaced. */
  fromModel: string
  /** Model that was used instead. */
  toModel: string
  /** Reason for fallback (rate_limit, context_overflow, error, etc.). */
  reason: string
  /** Whether the fallback succeeded (no further fallback needed). */
  success: boolean
}

/** Response for fallback history endpoint. */
export interface FallbackHistoryResponse {
  events: FallbackEvent[]
  totalCount: number
}