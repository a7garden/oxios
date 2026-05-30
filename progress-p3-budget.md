# Budget Page Implementation — Progress Report

**Date:** 2026-05-30
**Working Directory:** /Volumes/MERCURY/PROJECTS/oxios-p3
**Task:** Full backend + frontend budget page implementation

---

## Phase 1: Findings

### Backend Architecture

#### BudgetManager (`crates/oxios-kernel/src/budget.rs`)

- Private fields: `budgets: RwLock<HashMap<AgentId, BudgetLimit>>`, `usage: RwLock<HashMap<AgentId, Usage>>`
- Public methods: `set_budget`, `remove_budget`, `reserve`, `release`, `track_call`, `remaining` → `BudgetInfo`, `can_schedule`, `reset_window`, `persist`, `restore`
- `BudgetInfo` struct only has: `tokens_remaining`, `calls_remaining`, `window_remaining_secs`, `is_exhausted`
- **MISSING:** Methods to get limits and usage separately

#### AgentApi (`crates/oxios-kernel/src/kernel_handle/agent_api.rs`)

- Exposes budget operations via `self.budget_manager` (Arc<BudgetManager>)
- Public budget methods: `check_budget`, `set_budget`, `remove_budget`, `reserve_budget`, `reset_budget`
- `check_budget` → `BudgetManager::remaining` → `BudgetInfo`

#### Budget Routes (`surface/oxios-web/src/routes/budget_routes.rs`)

- `GET /api/budget` → `handle_budget_list`: lists agents, calls `check_budget` for each, returns `tokens_remaining`, `calls_remaining`, `window_remaining_secs`, `is_exhausted`
- **CURRENT OUTPUT:** Only "remaining" data, no limits, no used values
- Current response shape: `{ agent_id, name, tokens_remaining, calls_remaining, window_remaining_secs, is_exhausted }`

### Frontend Architecture

#### Budget page (`surface/oxios-web/web/src/routes/budget.tsx`)

- Uses old `Budget` type from `types/index.ts`
- Calls `GET /api/budget` expecting `{ items: Budget[] }`
- Shows `tokens_used`, `tokens_limit`, `cost_used`, `cost_limit` — fields that don't exist in backend response
- **PROBLEM:** Backend doesn't return `tokens_used` or `tokens_limit`

#### Old Budget type in `types/index.ts`

```typescript
export interface Budget {
  agent_id: string
  tokens_used?: number
  tokens_limit?: number
  cost_used?: number
  cost_limit?: number
}
```

- `cost_used` and `cost_limit` fields don't exist in backend — cost tracking not implemented
- `tokens_used` not returned by backend

### KernelHandle and AppState

- `AppState.kernel` → `KernelHandle`
- `KernelHandle.agents` → `AgentApi`
- AgentApi has `check_budget` but no full-info method

---

## Phase 2: Implementation Plan

### Step 1: Backend — Add FullBudgetInfo to BudgetManager

**File:** `crates/oxios-kernel/src/budget.rs`

Add struct and methods:

```rust
/// Full budget information including limits and usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullBudgetInfo {
    pub agent_id: AgentId,
    pub token_limit: u64,
    pub tokens_used: u64,
    pub tokens_remaining: u64,
    pub calls_limit: u64,
    pub calls_used: u64,
    pub calls_remaining: u64,
    pub window_secs: u64,
    pub window_remaining_secs: u64,
    pub is_exhausted: bool,
}

impl BudgetManager {
    /// Returns full budget information including limits and usage.
    pub fn full_info(&self, agent_id: &AgentId) -> Option<FullBudgetInfo> {
        let limit = self.budgets.read().get(agent_id).cloned()?;
        let usage = self.usage.read().get(agent_id).cloned();

        let (tokens_used, calls_used, window_remaining_secs) = if let Some(entry) = usage {
            let elapsed = Utc::now().signed_duration_since(entry.window_start)
                .to_std()
                .unwrap_or(Duration::ZERO);
            let window_duration = Duration::from_secs(limit.window_secs);
            let window_remaining = window_duration.saturating_sub(elapsed).as_secs();
            let elapsed_secs = elapsed.as_secs();

            // If window has expired, treat usage as 0
            if window_remaining == 0 && elapsed_secs >= limit.window_secs {
                (0u64, 0u64, 0u64)
            } else {
                (entry.tokens_used, entry.calls_used, window_remaining)
            }
        } else {
            (0u64, 0u64, limit.window_secs)
        };

        let tokens_remaining = limit.token_budget.saturating_sub(tokens_used);
        let calls_remaining = limit.calls_budget.saturating_sub(calls_used);
        let is_exhausted = tokens_remaining == 0 || calls_remaining == 0;

        Some(FullBudgetInfo {
            agent_id: *agent_id,
            token_limit: limit.token_budget,
            tokens_used,
            tokens_remaining,
            calls_limit: limit.calls_budget,
            calls_used,
            calls_remaining,
            window_secs: limit.window_secs,
            window_remaining_secs,
            is_exhausted,
        })
    }

    /// Returns full budget info for all agents.
    pub fn all_full_info(&self) -> Vec<FullBudgetInfo> {
        let budgets = self.budgets.read();
        budgets
            .keys()
            .filter_map(|id| self.full_info(id))
            .collect()
    }
}
```

### Step 2: Backend — Expose through AgentApi

**File:** `crates/oxios-kernel/src/kernel_handle/agent_api.rs`

Add:
```rust
pub fn full_budget_info(&self, agent_id: &AgentId) -> Option<FullBudgetInfo> {
    self.budget_manager.full_info(agent_id)
}
```

### Step 3: Backend — Update budget_routes.rs

**File:** `surface/oxios-web/src/routes/budget_routes.rs`

Rewrite `handle_budget_list` to use `all_full_info()`, include agent names, and add summary:

```rust
pub(crate) async fn handle_budget_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state.kernel.agents.list().await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let all_budgets = state.kernel.agents.all_budget_info();
    let agent_ids: HashSet<AgentId> = all_budgets.iter().map(|b| b.agent_id).collect();

    let mut total_tokens_used = 0u64;
    let mut total_tokens_limit = 0u64;
    let mut exhausted_count = 0usize;

    let items: Vec<serde_json::Value> = all_budgets
        .into_iter()
        .map(|b| {
            total_tokens_used += b.tokens_used;
            total_tokens_limit += b.token_limit;
            if b.is_exhausted {
                exhausted_count += 1;
            }

            let agent_name = agents.iter()
                .find(|a| a.id == b.agent_id)
                .map(|a| a.name.clone())
                .unwrap_or_default();

            serde_json::json!({
                "agent_id": b.agent_id.to_string(),
                "name": agent_name,
                "budget": {
                    "token_limit": b.token_limit,
                    "tokens_used": b.tokens_used,
                    "tokens_remaining": b.tokens_remaining,
                    "calls_limit": b.calls_limit,
                    "calls_used": b.calls_used,
                    "calls_remaining": b.calls_remaining,
                    "window_secs": b.window_secs,
                    "window_remaining_secs": b.window_remaining_secs,
                    "is_exhausted": b.is_exhausted,
                }
            })
        })
        .collect();

    let summary = serde_json::json!({
        "total_agents": agent_ids.len(),
        "total_tokens_used": total_tokens_used,
        "total_tokens_limit": total_tokens_limit,
        "exhausted_agents": exhausted_count,
    });

    let paginated = paginate(&items, &params);
    let mut response = serde_json::json!({});
    response["agents"] = paginated["items"].clone();
    response["summary"] = summary;
    if let Some(p) = paginated.get("total") { response["total"] = p.clone(); }
    if let Some(p) = paginated.get("page") { response["page"] = p.clone(); }
    if let Some(p) = paginated.get("limit") { response["limit"] = p.clone(); }

    Ok(Json(response))
}
```

Also update `handle_budget_get` to use full info format.

### Step 4: Frontend types

**File:** `surface/oxios-web/web/src/types/budget.ts`

```typescript
export interface BudgetData {
  token_limit: number
  tokens_used: number
  tokens_remaining: number
  calls_limit: number
  calls_used: number
  calls_remaining: number
  window_secs: number
  window_remaining_secs: number
  is_exhausted: boolean
}

export interface AgentBudget {
  agent_id: string
  name?: string
  budget: BudgetData
}

export interface BudgetSummary {
  total_agents: number
  total_tokens_used: number
  total_tokens_limit: number
  exhausted_agents: number
}

export interface BudgetListResponse {
  agents: AgentBudget[]
  summary: BudgetSummary
  total?: number
  page?: number
  limit?: number
}

export interface SetBudgetRequest {
  token_budget: number
  calls_budget: number
  window_secs: number
}
```

### Step 5: Frontend hooks

**File:** `surface/oxios-web/web/src/hooks/use-budget.ts`

```typescript
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { BudgetListResponse, SetBudgetRequest } from '@/types/budget'

export function useBudgetList() {
  return useQuery({
    queryKey: ['budgets'],
    queryFn: () => api.get<BudgetListResponse>('/api/budget'),
    refetchInterval: 10000,
  })
}

export function useBudgetSet() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ agentId, ...body }: { agentId: string } & SetBudgetRequest) =>
      api.post(`/api/budget/${agentId}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  })
}

export function useBudgetDelete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (agentId: string) => api.delete(`/api/budget/${agentId}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  })
}

export function useBudgetReset() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (agentId: string) => api.post(`/api/budget/${agentId}/reset`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  })
}
```

### Step 6: Frontend components

Create directory `surface/oxios-web/web/src/components/budget/`:

**`budget-summary.tsx`** — Summary card with progress bar, total agents, tokens used/limit, exhausted count.

**`agent-budget-card.tsx`** — Per-agent card with token and call progress bars, window remaining, Edit/Reset/Remove buttons.

**`set-budget-dialog.tsx`** — Dialog with token_budget, calls_budget, window_secs inputs. Used for both new and edit.

### Step 7: Rewrite budget page

**File:** `surface/oxios-web/web/src/routes/budget.tsx`

Complete rewrite using new components, hooks, and types.

### Step 8: i18n keys

Add to both `en/common.json` and `ko/common.json` under `budget`:

```json
"setBudget": "Set Budget",
"editLimit": "Edit limit",
"resetWindow": "Reset window",
"removeBudget": "Remove budget",
"tokenLimit": "Token limit",
"callLimit": "Call limit",
"windowSec": "Window (seconds)",
"windowRemaining": "Window remaining",
"setBudgetFor": "Set budget for {{agent}}",
"exhausted": "Exhausted",
"active": "Active",
"totalAgents": "Total agents",
"exhaustedCount": "Exhausted",
"save": "Save"
```

### Step 9: Cleanup

Remove old `Budget` interface from `types/index.ts`.

---

## Implementation Order

1. ✅ Read files (done)
2. ⬜ Add `FullBudgetInfo` struct + `full_info()` + `all_full_info()` to `budget.rs`
3. ⬜ Export `FullBudgetInfo` from `oxios-kernel` lib (check `lib.rs`)
4. ⬜ Add `full_budget_info` + `all_budget_info` to `AgentApi`
5. ⬜ Update `budget_routes.rs` to use full info
6. ⬜ Create `types/budget.ts`
7. ⬜ Create `hooks/use-budget.ts`
8. ⬜ Create `components/budget/` (3 components)
9. ⬜ Rewrite `routes/budget.tsx`
10. ⬜ Update i18n keys in both locale files
11. ⬜ Remove old Budget type from `types/index.ts`
12. ⬜ Build check

---

## Key Technical Decisions

1. **No kernel modification needed for getting limits**: `BudgetManager.budgets` and `BudgetManager.usage` are private, but we can add public accessor methods (`full_info`, `all_full_info`) directly to the struct — same crate, so no visibility issue.

2. **Window expiry handling**: When `elapsed_secs >= window_secs`, usage should be treated as 0 (sliding window reset). The current code in `remaining()` doesn't handle this — it just shows 0 window remaining. We need to reset usage tracking on expiry.

3. **Backend response format**: The new `handle_budget_list` returns `{ agents: [...], summary: {...} }` instead of `{ items: [...], total: ... }`. Frontend `use-budget.ts` uses this new shape.

4. **No cost tracking in this implementation**: The current backend doesn't track costs (no `cost_used`/`cost_limit`). The frontend page originally showed cost bars, but we'll focus on tokens + calls for now.

5. **Agent name resolution**: Backend resolves agent names by matching `agent_id` from agent list with budget entries. Fallback to empty string if agent not found.