# RFC-032: Chat Error Handling, Model Selection, and Role Routing

> Status: draft
> Date: 2026-06-28

## Motivation

Three related issues in Web UI chat flow:

1. **Silent failure on provider errors** — When a provider has no budget/quota
   remaining, the chat UI shows infinite loading with no error indication.
   The backend sends `done` with `evaluation_passed: false` but no `error` chunk,
   and the response content may be empty.

2. **Model selector only shows default provider's models** — In Settings →
   Engine, the `ModelSelect` dropdown only lists models from the provider
   extracted from `default_model`. Users with multiple API keys configured can't
   see or select models from other providers.

3. **No role-based model routing** — The SDK has no role feature. The user
   wants to define roles (e.g. "coder", "writer", "researcher") that map to
   specific providers/models, enabling automatic routing decisions.

## Current State

### Error Flow

```
User → WS → Gateway → Orchestrator.handle() → RecoveryCoordinator
                                                   ↓ BudgetExceeded
                                              L1 retry → fail
                                              L2 fallback → all exhausted
                                              L5 terminal → Ok(best) [success=false]
                                                   ↓
                              OrchestrationResult { response: result.output, error: ??? }
                                                   ↓
                              Gateway → OutgoingMessage::success(…, error: None)
                                                   ↓
                              WS recv_task → token chunk (maybe empty) → done chunk
                                                   ↓
                              Frontend → case 'error' NEVER triggered
```

**Root cause:**
- `RecoveryCoordinator::execute` returns `Ok(ExecutionResult { success: false, … })` not `Err(…)`
- Gateway line 580/628: `(Ok(orchestration), …)` → `ResponseMeta { error: None, … }`
- `OrchestrationResult` has no `error` field, no `failure_class` field
- WS `handle_chat_websocket` never checks `msg.meta.as_ref().and_then(|m| m.error.as_ref())`
  to emit an `{ type: "error", … }` chunk
- Frontend `case 'error'` handler exists (L1280) but is unreachable

### Model Selection

- `web/src/routes/settings.tsx` L352-355: Extracts `defaultProvider` from `currentModel`
  using `/` split, then calls `useModels(defaultProvider)` → only one provider's models
- Backend `handle_engine_models` without `?provider=` also extracts from
  `config.engine.default_model` → single provider
- `useModels(null)` returns `[]` immediately

### Role Routing

- oxi-sdk 0.45.1 has no role-based routing concept
- Kernel has three unrelated "role" concepts: PersonaRole (capabilities), RBAC Role (access),
  AgentRole (subtask orchestration) — none affect model selection
- Current routing is purely `"provider/model"` string-based with fallback chains

## Design

### Part A: Error Propagation (backend + frontend)

**A1. Propagate `failure_class` into `OrchestrationResult`**

Add field to `OrchestrationResult`:

```rust
pub struct OrchestrationResult {
    // … existing fields …
    /// If execution failed at the provider level, the failure class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<oxios_ouroboros::FailureClass>,
}
```

Populate in `handle_response_to_orchestration_result`:

```rust
HandleResponse::Task { ref result, .. } => OrchestrationResult {
    failure_class: result.failure_class,
    // If result.success is false and content is empty, generate a user-friendly message
    response: if result.output.trim().is_empty() {
        failure_response(&result.failure_class)
    } else {
        response_text
    },
    ..
}
```

**A2. Gateway: map `OrchestrationResult.failure_class` to `ResponseMeta.error`**

In `gateway.rs` L628, instead of `error: None`:

```rust
let response_meta = ResponseMeta {
    error: orchestration.failure_class.map(|fc| UserFacingError {
        message: orchestration.response.clone(),
        kind: failure_class_to_error_kind(fc),
        suggestion: suggestion_for(fc),
    }),
    ..
};
```

**A3. WS handler: emit `type: "error"` chunk when `meta.error` is present**

In `chat.rs` recv_task, before the token/done flow, check for error:

```rust
if let Some(ref error) = msg.meta.as_ref().and_then(|m| m.error.as_ref()) {
    let error_chunk = serde_json::json!({
        "type": "error",
        "seq": msg.seq,
        "message": error.message,
        "kind": error.kind,
        "suggestion": error.suggestion,
        "session_id": session_id,
        "project_id": project_id,
    });
    // Send error chunk + done to cleanly close the stream
    ws_tx.lock().await.send(…error_chunk…).await?;
    ws_tx.lock().await.send(…done_chunk…).await?;
    continue;
}
```

**A4. Frontend: enhance `case 'error'` handler**

Currently only sets `isStreaming: false`. Add error state display:

```typescript
case 'error': {
    set({
        isStreaming: false,
        lastError: {
            message: chunk.message as string,
            kind: chunk.kind as string,
            suggestion: chunk.suggestion as string | undefined,
        },
    })
    // Also append an error activity to the last assistant message
    // so the error is visible inline in the chat
    break
}
```

Add an `ErrorMessage` component that renders inline in the chat bubble
when `lastError` is set on a message.

### Part B: Cross-Provider Model Listing (frontend + backend)

**B1. Backend: `GET /api/engine/models` without `?provider=` returns ALL connected models**

Modify `handle_engine_models` at `src/api/routes/engine_routes.rs:90-114`:

```rust
(None, None) => {
    // Collect models from ALL providers that have credentials configured
    let config = state.config.read();
    let store = oxios_kernel::credential::CredentialStore::load()
        .unwrap_or_default();
    let connected_providers: Vec<&str> = store.provider_names();
    
    let all_models: Vec<ModelInfo> = connected_providers
        .iter()
        .flat_map(|p| state.kernel.engine.models(p, None))
        .collect();
    all_models
}
```

Fall back to the current behavior (default provider only) if no credentials are configured.

**B2. Frontend: use `useModels(null)` for multi-provider model list**

In `web/src/hooks/use-engine.ts`, modify `useModels`:

```typescript
export function useModels(provider: string | null) {
    return useQuery({
        queryKey: ['engine', 'models', provider],
        queryFn: async (): Promise<ModelInfo[]> => {
            if (provider) {
                // Single provider — current behavior
                try {
                    const res = await api.get<ProviderModelsResponse>(
                        `/api/engine/models?provider=${provider}`,
                    )
                    return res.models
                } catch {
                    return MODEL_CATALOG[provider] ?? []
                }
            }
            // All providers — NEW
            try {
                const res = await api.get<ProviderModelsResponse>('/api/engine/models')
                return res.models
            } catch {
                return []
            }
        },
        enabled: true,  // allow null provider
        staleTime: 5 * 60 * 1000,
    })
}
```

**B3. Frontend: show models from ALL connected providers in Settings**

In `web/src/routes/settings.tsx` EnginePanel:

```typescript
// OLD: const defaultProvider = currentModel.includes('/') ? …
// NEW: fetch all models regardless of default
const { data: models = [] } = useModels(null)  // null = all providers
```

The `ModelSelect` component already groups by reasoning/standard. Add a provider label
to each model row (prepend provider name before model name).

### Part C: Role-Based Model Routing

**C1. Data model: `RoleConfig` in config.toml**

```toml
[engine.roles]
[engine.roles.coder]
model = "anthropic/claude-sonnet-4-20250514"

[engine.roles.writer]
model = "openai/gpt-4o"

[engine.roles.researcher]
model = "google/gemini-pro"
```

Corresponding Rust type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoleRoutingConfig {
    /// Map of role name → model override.
    pub roles: HashMap<String, String>,
}
```

Added to `EngineConfig`:

```rust
pub struct EngineConfig {
    // … existing fields …
    #[serde(default)]
    pub role_routing: RoleRoutingConfig,
}
```

**C2. Routing logic in `AgentRuntime`**

When building the agent, before calling the LLM:

```rust
fn resolve_model_for_message(
    engine: &OxiosEngine,
    role: Option<&str>,
) -> String {
    let config = engine.config(); // or read from live config
    if let Some(role) = role {
        if let Some(model) = config.role_routing.roles.get(role) {
            return model.clone();
        }
    }
    engine.default_model_id.clone()
}
```

**C3. Web UI: role selector in chat sidebar**

Add a `RoleSelect` dropdown beside the model selector in the chat sidebar (or in
a compact header bar). The role persists per-session.

Frontend sends role in WS message:

```json
{
    "type": "message",
    "content": "…",
    "role": "coder",
    "session_id": "…"
}
```

Backend WS handler extracts `role` from incoming message and passes it through
to the orchestrator → agent runtime.

**C4. API: role CRUD in engine routes**

- `GET /api/engine/roles` → return current role→model mappings
- `PUT /api/engine/roles` → update mappings
- `DELETE /api/engine/roles/{name}` → remove a role

**C5. Default role: when no role specified, use `default_model`**

A bare `default_role` key in the config maps to the catch-all. If absent,
all un-routed messages use the global `default_model`.

### Part D: `defaultProvider` Removal

The concept of "default provider" is confusing. Replace with:

1. **`default_model`** stays — the actual model ID used when no role matches
2. **UI**: Provider cards in Settings show "connected" status, not "default" badge
3. **Onboarding**: First provider configured → pick a model → that becomes `default_model`

## Implementation Plan

### Phase 1: Error Visibility (Issue 1)

1. Add `failure_class` to `OrchestrationResult` (`orchestrator.rs`)
2. Generate user-friendly error text when `result.success == false` and output is empty
3. Map `failure_class` → `ResponseMeta.error` in gateway (`gateway.rs`)
4. Emit `type: "error"` chunk in WS handler (`chat.rs` recv_task)
5. Frontend: enhance error handling with inline error display

### Phase 2: Cross-Provider Models (Issue 2)

1. Backend: `GET /api/engine/models` without provider → all connected
2. Frontend: `useModels(null)` → multi-provider fetch
3. UI: remove `defaultProvider` concept, show all models
4. UI: add provider label to model rows

### Phase 3: Role Routing (Issue 3)

1. Config: `EngineConfig.role_routing`
2. Routing: `resolve_model_for_message()` in `AgentRuntime`
3. API: role CRUD endpoints
4. Frontend: role selector in chat UI
5. WS protocol: add `role` field to client→server messages
6. Integration test: role routes to correct model

## Verification

- **Error visibility**: Send message with exhausted provider → error chunk reaches
  frontend within 10s → inline error message displayed
- **Model listing**: Configure 2+ providers → model dropdown shows models from all
- **Role routing**: Set `role.coder = "anthropic/claude-…"` → send message with
  `role: "coder"` → agent uses Claude
- **Regression**: Normal chat flow (default model, no role, healthy provider)
  continues to work unchanged
