/**
 * Unified agent monitor hook.
 *
 * Joins lifecycle data (`/api/agents?status=running`) with A2A topology
 * and card data (`/api/a2a/*`) by the shared agent_id UUID. Returns
 * `MonitorData` ready for the canvas.
 *
 * The join is client-side because the two stores have different lifetimes:
 * lifecycle = SQLite (persistent), A2A = in-memory (transient). A unified
 * backend endpoint would still need to read both stores.
 */

import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import { api } from '@/lib/api-client'
import type { A2AAgentCard, A2ATopology, TopologyNode } from '@/types/a2a'
import type { AgentListItem, AgentListResponse } from '@/types/agent'
import type { MonitorData, MonitorEdge, MonitorNode } from '@/types/agent-monitor'

/** Normalise raw lifecycle status string → display status. */
function toDisplayStatus(status: string): MonitorNode['displayStatus'] {
  const s = status.toLowerCase()
  if (s === 'running' || s === 'active') return 'running'
  if (s === 'failed' || s === 'error' || s === 'crashed') return 'failed'
  if (s === 'completed' || s === 'success' || s === 'done') return 'completed'
  return 'idle'
}

/** Fetch running lifecycle agents. */
function useRunningAgents() {
  return useQuery({
    queryKey: ['agents', 'running', 'monitor'],
    queryFn: () =>
      api.get<AgentListResponse>(
        '/api/agents?status=running&per_page=100&sort_by=created_at&sort_dir=desc',
      ),
    refetchInterval: 5000,
  })
}

/** Fetch A2A topology (nodes + edges). */
function useTopology() {
  return useQuery({
    queryKey: ['a2a', 'topology'],
    queryFn: () => api.get<A2ATopology>('/api/a2a/topology'),
    refetchInterval: 5000,
  })
}

/** Fetch A2A agent cards (for capability/skill enrichment). */
function useA2ACards() {
  return useQuery({
    queryKey: ['a2a', 'cards'],
    queryFn: async () => {
      const res = await api.get<{ agents: A2AAgentCard[] }>('/api/a2a/agents')
      return Array.isArray(res?.agents) ? res.agents : []
    },
    refetchInterval: 5000,
  })
}

/**
 * The unified monitor data join.
 *
 * Nodes = running lifecycle agents (always present, persistent).
 * A2A enrichment = capabilities/skills/lastSeen (only while registered).
 * Edges = A2A message topology (last 5 min).
 */
export function useAgentMonitor(): MonitorData & {
  isFetching: boolean
  refetch: () => void
} {
  const agentsQ = useRunningAgents()
  const topologyQ = useTopology()
  const a2aQ = useA2ACards()

  const isFetching = agentsQ.isFetching || topologyQ.isFetching || a2aQ.isFetching

  const refetch = () => {
    agentsQ.refetch()
    topologyQ.refetch()
    a2aQ.refetch()
  }

  const data = useMemo<MonitorData>(() => {
    const runningAgents: AgentListItem[] = agentsQ.data?.items ?? []
    const topology: A2ATopology | undefined = topologyQ.data
    const a2aCards: A2AAgentCard[] = a2aQ.data ?? []

    // Build A2A lookup by agent_id UUID.
    const a2aMap = new Map<string, A2AAgentCard>()
    for (const card of a2aCards) {
      a2aMap.set(card.agent_id, card)
    }

    // Build a name → topology-node map (topology nodes are keyed by name).
    const topoByName = new Map<string, TopologyNode>()
    if (topology?.nodes) {
      for (const node of topology.nodes) {
        topoByName.set(node.label, node)
      }
    }

    // Build nodes from running lifecycle agents, enriched with A2A data.
    const nodes: MonitorNode[] = runningAgents.map((agent) => {
      const displayStatus = toDisplayStatus(agent.status)
      const card = a2aMap.get(agent.id)
      const topoNode = topoByName.get(agent.name)

      return {
        agentId: agent.id,
        name: agent.name,
        lifecycle: {
          status: agent.status,
          cost_usd: agent.cost_usd,
          tokens_used: agent.tokens_used,
          duration_secs: agent.duration_secs,
          model_id: agent.model_id,
          error: agent.error,
          created_at: agent.created_at,
          session_id: agent.session_id,
        },
        a2a: card
          ? {
              capabilities: card.capabilities,
              skills: card.skills,
              description: card.description,
              lastSeen: topoNode?.last_seen ?? null,
            }
          : undefined,
        displayStatus,
      }
    })

    // Also surface A2A-registered agents that aren't in the lifecycle running list
    // (brief registration window). Avoid duplicates by agent_id.
    const existingIds = new Set(nodes.map((n) => n.agentId))
    for (const card of a2aCards) {
      if (existingIds.has(card.agent_id)) continue
      nodes.push({
        agentId: card.agent_id,
        name: card.name,
        lifecycle: {
          status: card.status ?? 'running',
          cost_usd: 0,
          tokens_used: 0,
          duration_secs: null,
          model_id: '',
          error: null,
          created_at: new Date().toISOString(),
          session_id: null,
        },
        a2a: {
          capabilities: card.capabilities,
          skills: card.skills,
          description: card.description,
          lastSeen: topoByName.get(card.name)?.last_seen ?? null,
        },
        displayStatus: 'running',
      })
    }

    // Map topology edges.
    const edges: MonitorEdge[] = (topology?.edges ?? []).map((e) => ({
      from: e.from,
      to: e.to,
      messageCount5m: e.message_count_5m,
      lastKind: e.last_kind,
    }))

    // Aggregate stats.
    const stats = {
      running: nodes.filter((n) => n.displayStatus === 'running').length,
      completed: 0,
      failed: 0,
      totalCost: nodes.reduce((sum, n) => sum + n.lifecycle.cost_usd, 0),
      totalTokens: nodes.reduce((sum, n) => sum + n.lifecycle.tokens_used, 0),
    }

    return { nodes, edges, stats }
  }, [agentsQ.data, topologyQ.data, a2aQ.data])

  return { ...data, isFetching, refetch }
}
