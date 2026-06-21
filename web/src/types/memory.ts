/** Types for the Memory system hooks (use-memory.ts) */

// ── Stats ──
export interface MemoryStats {
  total: number
  by_tier: Record<string, number>
  by_type: Record<string, number>
  total_size_bytes: number
  oldest_created: string | null
  newest_created: string | null
}

// ── Detail ──
export interface MemoryDetail {
  id: string
  key: string
  tier: string
  memory_type: string
  content: string
  summary: string | null
  project_ids: string[]
  created_at: string
  updated_at: string
  last_accessed: string | null
  access_count: number
  pinned: boolean
  protected: boolean
  protection_reason: string | null
  tags: string[]
  metadata: Record<string, unknown>
}

// ── Dream ──
export interface DreamReport {
  id: string
  started_at: string
  completed_at: string | null
  status: string
  memories_processed: number
  memories_consolidated: number
  memories_decayed: number
  summary: string | null
}

export interface DreamStatus {
  running: boolean
  last_run: string | null
  next_run: string | null
  cycles_completed: number
}

// ── Semantic search ──
export interface SemanticSearchResult {
  id: string
  key: string
  content: string
  summary: string | null
  tier: string
  memory_type: string
  score: number
  distance: number
}

// ── Map (RFC-T1-B) ──
/** One memory neighbour edge on the map. */
export interface MemoryMapNeighbor {
  id: string
  similarity: number
}

/** One node on the memory map (RFC-T1-B). */
export interface MemoryMapEntry {
  id: string
  tier: string
  mem_type: string
  content_preview: string
  created_at: string
  access_count: number
  coords_2d: [number, number]
  top_neighbors: MemoryMapNeighbor[]
}

/** Response from GET /api/memory/map. */
export interface MemoryMapResponse {
  count: number
  /** Cache epoch in 5-minute resolution. */
  epoch: number
  entries: MemoryMapEntry[]
}
