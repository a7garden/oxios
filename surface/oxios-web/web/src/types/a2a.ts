/** A2A observation types aligned with kernel A2A protocol. */

export interface A2AAgentCard {
  agent_id: string
  name: string
  description: string
  capabilities: string[]
  skills: string[]
  status: string
  endpoint: string
}

export interface A2AMessage {
  request_id: string
  from_agent: string
  to_agent: string
  message_type: string
  payload_summary: string
  accepted: boolean
  timestamp: string
}

export interface TopologyNode {
  id: string
  label: string
  status: string
}

export interface TopologyEdge {
  from: string
  to: string
  type: string
  count: number
}

export interface A2ATopology {
  nodes: TopologyNode[]
  edges: TopologyEdge[]
}
