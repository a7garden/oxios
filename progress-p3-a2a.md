# A2A Monitor — Step 3 Findings & Implementation Plan

**Working directory:** /Volumes/MERCURY/PROJECTS/oxios-p3
**Date:** 2026-05-30

## Findings

### Backend API Path

After reading `server.rs`, `lib.rs`, `kernel_handle/mod.rs`, and `a2a.rs`:

1. **`state.kernel.a2a`** is of type `A2aApi` (a facade), NOT `A2AProtocol` directly.
2. `A2aApi` exposes:
   - `protocol()` → `&Arc<A2AProtocol>` → `registry()` → `&AgentCardRegistry`
   - `message_bus()` → `&MessageBus`
   - `subscribe()` → broadcast receiver
3. The `AgentCardRegistry` has async methods: `list_agents()`, `get_agent(id)`, `agent_count()`, etc.
4. **No message log currently exists** — queues are per-agent in-memory, no persistent storage.
5. **No topology edges** in the kernel yet — edges can be derived once message logging is added.

### Access Path (confirmed working)

```
state.kernel.a2a.protocol().registry().list_agents().await
state.kernel.a2a.protocol().registry().get_agent(id).await
```

### Kernel API surface available

- `A2AProtocol::registry()` → `&AgentCardRegistry` (public)
- `AgentCardRegistry::list_agents()`, `get_agent()`, `find_agents_by_capability()`, etc.
- Event bus publishes `KernelEvent::MessageReceived` (can be queried in future for message log)
- `A2aApi::subscribe()` returns broadcast receiver for `InterAgentMessage` (future use)

## Implementation Plan

### Phase 1: Backend (`surface/oxios-web/src/routes/a2a.rs`)

- Endpoint: `GET /api/a2a/agents` → list all registered agent cards
- Endpoint: `GET /api/a2a/agents/{id}` → get single agent card
- Endpoint: `GET /api/a2a/messages` → placeholder (empty array, TODO when kernel adds message log)
- Endpoint: `GET /api/a2a/topology` → nodes from registry, no edges yet

### Phase 2: Frontend Types (`web/src/types/a2a.ts`)

- `A2AAgentCard`, `A2AMessage`, `TopologyNode`, `TopologyEdge`, `A2ATopology`

### Phase 3: Frontend Hooks (`web/src/hooks/use-a2a.ts`)

- `useA2AAgents()`, `useA2AMessages()`, `useA2ATopology()`
- All refetch every 10s

### Phase 4: Frontend Page (`web/src/routes/a2a.tsx`)

- 3-tab layout: Topology | Messages | Agents
- Uses existing Tabs component from `components/ui/tabs.tsx`

### Phase 5: Components

- `TopologyGraph.tsx` — SVG circle layout (matching link-graph.tsx pattern)
- `MessageLog.tsx` — table with Time/From→To/Type/Status columns
- `AgentCardList.tsx` — grid of agent cards with status indicators

### Phase 6: Sidebar & i18n

- Add `Network` icon to monitor group in sidebar
- Add i18n keys for both locales

## Status: Ready for Implementation

All required API paths are accessible via the existing kernel public API.
No kernel modification needed for Phase 1 (read-only observation).