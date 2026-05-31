# Retry Gap Analysis

**Area:** Resilience — retry/fallback coverage across all components  
**Date:** 2026-05-31

---

## Component-by-Component Analysis

### 1. LLM Provider Calls

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ✅ Yes — via oxi-sdk and OxiosEngine |
| **Mechanism** | Circuit Breaker in `agent_runtime.rs` + provider-level retries in oxi-sdk |
| **Coverage** | Transient network errors, rate limits, server errors |
| **Backoff** | Exponential with jitter (oxi-sdk managed) |
| **Circuit breaker** | 3-state (Closed → Open → Half-Open) via `a2a_circuit_breaker.rs` pattern |
| **Gap** | None identified — oxi-sdk handles provider failover comprehensively |

**Verdict:** ✅ Complete. The LLM call path is well-protected.

---

### 2. Telegram Bot Channel

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ✅ Yes — exponential backoff |
| **Mechanism** | Manual retry loop in `oxios-telegram/src/lib.rs:428-432` |
| **Coverage** | Telegram API errors, network timeouts |
| **Backoff** | `5 * 2^retry_count` seconds, capped at 4 retries (80s max delay) |
| **Code** | `let delay = Duration::from_secs(5 * 2u64.pow(retry_count.min(4)));` |
| **Gap** | No circuit breaker — continues retrying indefinitely after backoff |

**Verdict:** ✅ Adequate. Telegram polling is resilient by nature (it's a long-poll loop).

---

### 3. Web API (HTTP)

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ❌ No |
| **Rationale** | Client-side concern. HTTP clients (browsers, load balancers) handle retries. |
| **Server-side retries** | None needed — requests are idempotent (GET) or user-initiated (POST) |
| **Gap** | None — correct design. The server responds, the client retries if needed. |

**Verdict:** ✅ By design. Not a server responsibility.

---

### 4. MCP Client

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ⚠️ Partial — auto-restart on communication errors |
| **Mechanism** | Auto-restart in `oxios-mcp/src/client.rs:241-258` |
| **Coverage** | "not available", "broken pipe", "timed out", "no response" |
| **Behavior** | Restarts the MCP server process, then **bails** with "Please retry the request" |
| **Missing** | No automatic retry of the actual tool call after restart |
| **Gap** | After restart, the caller must retry manually. The `send_request` method returns an error, expecting the caller to re-invoke. |

**Analysis:**

```rust
// client.rs:243-258 (paraphrased)
if is_comm_error {
    self.restart().await?;
    bail!("MCP server '{}' restarted after error. Please retry the request.");
}
```

The restart recovers the MCP server, but the original request is lost. The caller (agent runtime) gets an error and may or may not retry.

**Impact:** MCP tool calls fail transiently. The agent's LLM may retry the tool call in its next turn, but this depends on the agent's prompt and the LLM's behavior.

**Recommendation:** Add a configurable retry count (1-2) in `send_request()` that retries the actual call after a successful restart. This is a small, low-risk change.

---

### 5. Tool Execution

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ❌ No |
| **Mechanism** | None — tools execute once and return success/error |
| **Coverage** | N/A |
| **Rationale** | Tools are stateful (file writes, shell commands). Retrying could cause side effects. |
| **Gap** | Intentional. Tool retry is the agent's responsibility (LLM decides whether to retry). |

**Verdict:** ✅ By design. Tools must be idempotent for the agent to safely retry.

**Exception:** `ExecTool` with `structured` mode is designed to be safe (allowlist + metacharacter blocking). `Shell` mode is inherently unsafe to retry.

---

### 6. Agent Runtime (oxi-sdk)

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ✅ Yes — built into oxi-sdk's agent loop |
| **Mechanism** | oxi-sdk handles provider errors and retries tool calls |
| **Coverage** | LLM call failures within the agent's tool-calling loop |
| **Gap** | If the agent itself crashes (panic, OOM), no retry. The orchestrator treats this as a failed execution. |

**Verdict:** ✅ Adequate for normal operation. Agent crashes are handled by the orchestrator's evaluate/evolve loop.

---

### 7. A2A Protocol (Inter-Agent Delegation)

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ✅ Yes — exponential backoff with circuit breaker |
| **Mechanism** | `Orchestrator.delegate_with_retry()` + `A2ACircuitBreaker` |
| **Coverage** | A2A delegation failures |
| **Config** | `max_retries: 3`, `base_delay_ms: 100`, `max_delay_ms: 5000` |
| **Fallback** | Falls back to direct lifecycle execution when A2A fails or circuit opens |
| **Gap** | None identified. Three-layer protection: retry → circuit breaker → fallback. |

**Verdict:** ✅ Complete. The A2A delegation path is the most thoroughly protected.

---

### 8. Session Recovery

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ❌ No |
| **Mechanism** | None — sessions are ephemeral |
| **Coverage** | N/A |
| **Gap** | Active sessions are lost on restart. See `SESSION-PERSISTENCE-DESIGN.md`. |

**Verdict:** ❌ Known gap. Addressed by the session persistence design.

---

### 9. Event Bus

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ❌ No (nor should it) |
| **Mechanism** | `tokio::sync::broadcast` — fire-and-forget |
| **Coverage** | N/A — events are informational |
| **Gap** | If no subscriber is listening, events are dropped. This is by design (broadcast semantics). |

**Verdict:** ✅ By design. Events are best-effort notifications.

---

### 10. Git Layer

| Aspect | Detail |
|--------|--------|
| **Has retry?** | ❌ No |
| **Mechanism** | None — git operations are fire-and-forget in most codepaths |
| **Coverage** | N/A |
| **Gap** | Git commit failures are logged as warnings but not retried. The knowledge base auto-commit channel (`kernel.rs`) sends changes asynchronously; if the consumer misses one, the change is in the filesystem but not committed to git. |

**Verdict:** 🟢 Acceptable. Git commits are non-critical (the data is in the filesystem). The guardian loop re-verifies git integrity every 5 minutes.

---

## Summary Table

| Component | Retry? | Mechanism | Gap? | Severity |
|-----------|--------|-----------|------|----------|
| LLM Provider | ✅ | Circuit Breaker + oxi-sdk | None | — |
| Telegram Bot | ✅ | Exponential backoff | No circuit breaker | 🟢 Low |
| Web API | ✅ | Client-side | By design | — |
| MCP Client | ⚠️ | Auto-restart only | No retry after restart | 🟡 Medium |
| Tool Execution | ❌ | None | By design | — |
| Agent Runtime | ✅ | oxi-sdk loop | Agent crashes not retried | 🟢 Low |
| A2A Delegation | ✅ | Retry + CB + fallback | None | — |
| Session Recovery | ❌ | None | Active sessions lost | 🔴 High |
| Event Bus | ✅ | Broadcast | By design | — |
| Git Layer | ❌ | None | Fire-and-forget | 🟢 Low |

---

## Recommended Actions

| Priority | Action | Effort | Impact |
|----------|--------|--------|--------|
| 1 | Session persistence (design only) | Done | 🔴 Critical |
| 2 | MCP client: retry tool call after restart | Small (10 lines) | 🟡 Medium |
| 3 | Telegram: add circuit breaker | Small (reuse A2ACircuitBreaker) | 🟢 Low |
| 4 | Git commit retry (at most once) | Small | 🟢 Low |
