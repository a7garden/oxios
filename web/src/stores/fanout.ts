// fanout store — list of currently-running worktree fan-out agents.
//
// RFC-044 Phase 3: when a user submits a worktree fan-out, the backend
// returns N spawned agents. The chat substrate tracks them here so the
// inline AgentFanoutCardGrid in the transcript can render status updates,
// and so the composer can show a "fan-out in progress" badge.
//
// This is intentionally tiny — separate from the chat store because the
// fan-out lifecycle is independent of the active chat message stream.

import { create } from 'zustand'
import type { AgentFanoutStatus } from '@/components/chat/AgentFanoutCard'

export interface FanoutAgent {
  agentId: string
  name?: string
  worktreePath?: string
  status: AgentFanoutStatus
  detail?: string
  updatedAt: number
}

export interface FanoutGroup {
  /** Stable id for the fan-out invocation (so React can key the grid). */
  groupId: string
  /** Original prompt the user submitted. */
  prompt: string
  /** When the fan-out was created (epoch ms). */
  createdAt: number
  /** All spawned agents. */
  agents: FanoutAgent[]
}

interface FanoutState {
  groups: FanoutGroup[]
}

interface FanoutActions {
  addGroup: (group: Omit<FanoutGroup, 'createdAt'>) => string
  updateAgent: (
    groupId: string,
    agentId: string,
    patch: Partial<Omit<FanoutAgent, 'agentId'>>,
  ) => void
  removeGroup: (groupId: string) => void
  clear: () => void
}

export type FanoutStore = FanoutState & FanoutActions

export const useFanoutStore = create<FanoutStore>()((set) => ({
  groups: [],
  addGroup: (group) => {
    const groupId = group.groupId
    set((state) => ({
      groups: [...state.groups, { ...group, createdAt: Date.now() }],
    }))
    return groupId
  },
  updateAgent: (groupId, agentId, patch) =>
    set((state) => ({
      groups: state.groups.map((g) =>
        g.groupId !== groupId
          ? g
          : {
              ...g,
              agents: g.agents.map((a) =>
                a.agentId !== agentId ? a : { ...a, ...patch, updatedAt: Date.now() },
              ),
            },
      ),
    })),
  removeGroup: (groupId) =>
    set((state) => ({ groups: state.groups.filter((g) => g.groupId !== groupId) })),
  clear: () => set({ groups: [] }),
}))

/** Convenience selector — flat list of all fan-out agents across groups. */
export function selectAllFanoutAgents(state: FanoutStore): FanoutAgent[] {
  return state.groups.flatMap((g) => g.agents)
}
