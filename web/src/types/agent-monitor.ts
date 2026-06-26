/**
 * Unified agent monitor types.
 *
 * Merges lifecycle data (`/api/agents` — SQLite, persistent) with A2A
 * relationship data (`/api/a2a/*` — in-memory, transient) by joining on
 * the shared `agent_id` UUID.
 *
 * The canvas shows currently-running agents (lifecycle) enriched with
 * their A2A capabilities and connections. Single-agent runs show one
 * node with no edges; multi-agent delegation shows nodes + message edges.
 */

import type { TopologyEdge, TopologyNode } from './a2a'
import type { AgentListItem } from './agent'

/** A2A enrichment data for a running agent (may be absent for completed). */
export interface AgentA2AInfo {
  capabilities: string[]
  skills: string[]
  description: string
  lastSeen: string | null
}

/**
 * A unified agent node for the canvas — the join of lifecycle + A2A.
 *
 * `lifecycle` is always present (from the persistent agent log).
 * `a2a` is present only while the agent is running and registered
 * in the A2A card registry.
 */
export interface MonitorNode {
  /** UUID — the shared join key. */
  agentId: string
  /** Display name. */
  name: string
  /** Lifecycle record (cost, tokens, status, etc.). */
  lifecycle: Pick<
    AgentListItem,
    | 'status'
    | 'cost_usd'
    | 'tokens_used'
    | 'duration_secs'
    | 'model_id'
    | 'error'
    | 'created_at'
    | 'session_id'
  >
  /** A2A enrichment (capabilities, skills, lastSeen). Undefined when not in A2A registry. */
  a2a?: AgentA2AInfo
  /** Normalized display status for color-coding. */
  displayStatus: 'running' | 'completed' | 'failed' | 'idle'
}

/** A connection between two agents (A2A message flow). */
export interface MonitorEdge {
  from: string
  to: string
  messageCount5m: number
  lastKind: string
}

/** Result of the unified monitor data join. */
export interface MonitorData {
  nodes: MonitorNode[]
  edges: MonitorEdge[]
  /** Aggregate stats for the header bar. */
  stats: {
    running: number
    completed: number
    failed: number
    totalCost: number
    totalTokens: number
  }
}

/** Raw inputs for the join (fetched by the hook). */
export interface MonitorRawData {
  runningAgents: AgentListItem[]
  topology: { nodes: TopologyNode[]; edges: TopologyEdge[] } | undefined
  a2aAgents: { agent_id: string; capabilities: string[]; skills: string[]; description: string }[]
}
