# Oxios Agent OS — Security Guide

> Comprehensive reference for the security architecture, configuration, and
> operational practices of the Oxios Agent OS.

---

## Table of Contents

1. [Security Philosophy](#1-security-philosophy)
2. [Access Manager](#2-access-manager)
3. [RBAC System](#3-rbac-system)
4. [Audit Trail](#4-audit-trail)
5. [Circuit Breaker](#5-circuit-breaker)
6. [Execution Security](#6-execution-security)
7. [Authentication](#7-authentication)
8. [Credential Store](#8-credential-store)
9. [Budget Management](#9-budget-management)
10. [Security Configuration](#10-security-configuration)
11. [Security Best Practices](#11-security-best-practices)
12. [Incident Response](#12-incident-response)

---

## 1. Security Philosophy

Oxios is an **Agent Operating System** — AI agents execute real work on behalf of
users, including forking processes, reading/writing files, and making network
requests. This demands a rigorous security posture grounded in three principles:

### Least Privilege

Every agent starts with **minimal permissions**. No tools, no network, no
forking — nothing is granted by default. Permissions must be explicitly
assigned through configuration or authorized requests.

```
Default Agent Permissions:
  ✗ Network access
  ✗ Fork sub-agents
  ✗ Unlimited execution time
  ✓ Basic file tools (read, write, edit, bash, grep, find)
  ✓ Workspace-only file access (/workspace/**)
```

### Defense in Depth

Security is enforced at **multiple independent layers**. A failure at one layer
does not compromise the system because additional layers provide redundant
protection:

```
Layer 1: RBAC          — Role-based policy (who can do what)
Layer 2: Permissions   — Per-agent tool/path/network restrictions
Layer 3: Workspace     — Path sandboxing (filesystem confinement)
Layer 4: ExecTool      — Binary allowlist + metacharacter blocking
Layer 5: Budget        — Token/call limits prevent resource exhaustion
Layer 6: Audit Trail   — Tamper-evident logging detects post-hoc tampering
Layer 7: Circuit Breaker — Protects against cascading LLM provider failures
```

### OWASP-Inspired Design

The access control model draws from [OWASP Agentic AI Security Guidelines](https://owasp.org/www-project-top-10-for-large-language-model-applications/):

- **Agent identity** — Every agent has a unique ID and name for attribution.
- **Audit logging** — All security-relevant decisions are logged immutably.
- **Sandbox boundaries** — Path restrictions confine agents to designated areas.
- **Tool access control** — Agents can only use explicitly permitted tools.
- **Human-in-the-Loop (HitL)** — High-risk operations require explicit approval.

---

## 2. Access Manager

The `AccessManager` is the central security component in the Oxios kernel. It
manages per-agent permissions, enforces security boundaries, and maintains an
audit log of all security-relevant actions.

**Source:** `crates/oxios-kernel/src/access_manager/mod.rs`

### Agent Permissions Model

Every agent has an `AgentPermissions` object that defines its capabilities:

```toml
# Per-agent permission set (conceptual — set programmatically)
[permissions."my-agent"]
agent_name = "my-agent"
allowed_tools = ["read", "write", "bash"]
allowed_paths = ["/workspace/my-project/**"]
denied_paths = ["/workspace/my-project/.secret/**"]
network_access = false
max_execution_time_secs = 300
max_memory_mb = 512
can_fork = false
```

**Default permissions** (applied to all new agents unless overridden):

| Setting | Default | Description |
|---------|---------|-------------|
| `allowed_tools` | `read, write, edit, bash, grep, find` | Basic file manipulation tools |
| `allowed_paths` | `/workspace/**` | Workspace-only access |
| `denied_paths` | `/etc/**, /root/**, /sys/**, /proc/**, .oxios/**` | System-critical paths |
| `network_access` | `false` | No network requests |
| `max_execution_time_secs` | `300` | 5-minute execution cap |
| `max_memory_mb` | `512` | 512 MB memory cap |
| `can_fork` | `false` | Cannot spawn sub-agents |

#### Programmatic Permission Management

```rust
use oxios_kernel::access_manager::{AccessManager, AgentPermissions, PermissionUpdate};

let mut access = AccessManager::new();

// Create default permissions for a new agent
access.set_permissions(AgentPermissions::for_new_agent("code-agent"));

// Grant additional capabilities
let update = PermissionUpdate {
    network_access: Some(true),
    max_execution_time_secs: Some(600),
    allowed_tools: Some(
        ["read", "write", "edit", "bash", "grep", "find", "curl"]
            .into_iter().map(String::from).collect()
    ),
    ..Default::default()
};
access.update_permissions("code-agent", update)?;

// Validate permission set for warnings
let warnings = access.validate_permissions(access.get_permissions("code-agent").unwrap());
for warning in &warnings {
    eprintln!("⚠ {warning}");
}
```

### Path Sandboxing (Workspace Confinement)

Agents can be assigned to **workspaces** — directory-based sandboxes that confine
all file operations to a specific subtree. A path access check performs
**canonical path resolution** to prevent symlink-based escapes.

```rust
// Register workspace directories
access.register_workspace_path("project-alpha", PathBuf::from("/workspace/alpha"));
access.register_workspace_path("project-beta", PathBuf::from("/workspace/beta"));

// Assign agents to workspaces
access.assign_workspace("agent-1", "project-alpha");
access.assign_workspace("agent-2", "project-beta");

// agent-1 can ONLY access /workspace/alpha/**, NOT /workspace/beta/**
assert!(access.can_access_workspace("agent-1", "project-alpha"));
assert!(!access.can_access_workspace("agent-1", "project-beta"));
```

The full sandbox check (`can_access_path_in_workspace`) enforces three
independent layers:

1. **RBAC** — Does the agent's role permit this action?
2. **Path permissions** — Is the path in `allowed_paths` and not in `denied_paths`?
3. **Workspace boundary** — Is the path within the agent's assigned workspace?

```
can_access_path_in_workspace(agent_id, agent_name, path, workspace):
  ├── RBAC check:     rbac.check_permission(Subject::Agent(agent_id), Action::AccessPath(path))
  ├── Path check:     can_access_path(agent_name, path)
  │   ├── denied_paths takes precedence
  │   └── must match allowed_paths glob
  └── Workspace check: is_path_in_workspace(workspace, path)
      └── Canonical path comparison (prevents symlink escapes)
```

Any failure is logged as a **sandbox violation** with the reason.

### Network Restrictions

Network access is **disabled by default** and must be explicitly enabled per
agent:

```rust
// Enable network for a specific agent
let mut access = AccessManager::new();
let mut perms = AgentPermissions::for_new_agent("net-agent");
perms.enable_network();
access.set_permissions(perms);

// Check before allowing network operations
if access.can_access_network("net-agent") {
    // proceed with network request
}
```

### Execution Limits (Time, Memory)

Agents are bounded in both wall-clock time and memory usage to prevent resource
exhaustion:

```rust
// Check execution time (default: 300 seconds)
if access.can_execute_for("agent-1", 600) {
    // Allow long-running task
}

// Check memory (default: 512 MB)
if access.can_use_memory("agent-1", 1024) {
    // Allow memory-intensive operation
}

// Unlimited mode: set to 0
// max_execution_time_secs = 0  → no time limit
// max_memory_mb = 0            → no memory limit
```

---

## 3. RBAC System

Oxios implements a **Role-Based Access Control** system with three tiers and a
Human-in-the-Loop approval workflow for high-risk operations.

**Source:** `crates/oxios-kernel/src/access_manager/rbac.rs`

### Roles (3-Tier Model)

| Role | Description | Max Concurrent Agents |
|------|-------------|----------------------|
| **User** | Basic agent user. Limited tools and workspace-only paths. | 2 |
| **Superuser** | Can manage programs, skills, workspaces, and use all tools. | 10 |
| **Admin** | Full system access including RBAC management and system config. | Unlimited |

#### Default Policy by Role

**User** role defaults:
```rust
allowed_actions: [
    UseTool("read"), UseTool("write"), UseTool("edit"),
    UseTool("bash"), UseTool("grep"), UseTool("find"),
    AccessPath("/workspace/**"),
    ManageAgents,
]
resource_patterns: ["/workspace/**"]
max_concurrent_agents: 2
```

**Superuser** role defaults:
```rust
allowed_actions: [
    UseTool("*"),                        // All tools
    AccessPath("/workspace/**"),
    ManageAgents, ManagePrograms, ManageWorkspaces,
    ViewAuditLog,
]
resource_patterns: ["/workspace/**", "/tmp/**"]
max_concurrent_agents: 10
```

**Admin** role defaults:
```rust
allowed_actions: [
    UseTool("*"),                        // All tools
    AccessPath("*"),                     // All paths
    ManageAgents, ManagePrograms, ManageWorkspaces,
    ManageRBAC, ViewAuditLog, SystemConfig,
]
resource_patterns: ["*"]
max_concurrent_agents: usize::MAX
```

### Subjects

The RBAC system recognizes three types of subjects:

| Subject | Description |
|---------|-------------|
| `Subject::User(name)` | A named human user |
| `Subject::Agent(id)` | An AI agent identified by UUID |
| `Subject::System` | System-level operations (bypasses RBAC entirely) |

### Actions

The `Action` enum defines all authorizable operations:

| Action | Description | Requires Approval? |
|--------|-------------|--------------------|
| `UseTool(name)` | Use a specific tool | Yes if `*`, `osascript`, or `rm` |
| `AccessPath(pattern)` | Access files matching a glob | No |
| `ManageAgents` | Fork/exec/kill agents | No |
| `ManagePrograms` | Install/uninstall programs | No |
| `ManageWorkspaces` | Create/start/stop/remove workspaces | No |
| `ManageRBAC` | Modify RBAC policies and roles | **Yes** |
| `ViewAuditLog` | View the audit trail | No |
| `SystemConfig` | Modify system configuration | **Yes** |

### Policy Definitions

Policies support **wildcard matching** for tools and paths:

- `UseTool("*")` matches any tool name
- `AccessPath("*")` matches any path

The `allows()` method checks exact match first, then falls back to wildcard:

```rust
let policy = Role::Admin.default_policy();
assert!(policy.allows(&Action::UseTool("literally_anything".into())));
assert!(policy.allows(&Action::AccessPath("/secret/data".into())));
```

### Pending Approvals (Human-in-the-Loop)

High-risk actions (as marked by `Action::requires_approval()`) require explicit
human approval before execution. The approval lifecycle:

```
Request → Pending → Approved / Rejected / Expired
```

```rust
let mut rbac = RbacManager::new();

// Request approval for a high-risk action
let approval_id = rbac.request_approval(
    Subject::User("alice".into()),
    Action::ManageRBAC,
    "rbac".into(),
    "Need to modify RBAC policies".into(),
);

// List pending approvals
let pending: Vec<&PendingApproval> = rbac.pending_approvals();

// Approve or reject
rbac.approve(approval_id);   // → Approved
rbac.reject(approval_id);    // → Rejected
```

#### Approval Request Format

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "subject": { "User": "alice" },
  "action": { "ManageRBAC": null },
  "resource": "rbac",
  "reason": "Need to modify RBAC policies",
  "created_at": "2025-06-15T10:30:00Z"
}
```

#### RBAC Audit Log

Every authorization decision is recorded with the subject, action, result, and
reason for denial:

```rust
let audit_log: &[RbacAuditEntry] = rbac.audit_log();
// Each entry:
// {
//   "timestamp": "2025-06-15T10:30:05Z",
//   "subject": { "User": "alice" },
//   "action": { "ManageRBAC": null },
//   "resource": "rbac",
//   "allowed": false,
//   "reason": "role User does not allow ManageRBAC"
// }
```

---

## 4. Audit Trail

The `AuditTrail` provides a **tamper-evident audit log** using a cryptographic
hash chain. Every kernel event is recorded and linked to the previous entry,
making any post-hoc tampering detectable.

**Source:** `crates/oxios-kernel/src/audit_trail.rs`

### blake3 Hash-Chain Design

Each audit entry is cryptographically linked to the previous entry using
**blake3** hashing. The hash is computed over all entry fields in a
deterministic manner:

```
hash = blake3(
  "oxios-audit-v1"  ||
  seq.to_be_bytes()  ||
  timestamp_rfc3339  ||
  actor              ||
  action_json        ||
  prev_hash          ||
  resource
)
```

This produces a **64-character hex digest** (blake3's 256-bit output).

```
Entry 0:  prev_hash = "genesis"  → hash = abc123...
Entry 1:  prev_hash = abc123...  → hash = def456...
Entry 2:  prev_hash = def456...  → hash = ghi789...
```

### Entry Format

Each `AuditEntry` contains:

| Field | Type | Description |
|-------|------|-------------|
| `seq` | `u64` | Sequential entry number (starts at 1) |
| `timestamp` | `DateTime<Utc>` | When the event occurred |
| `actor` | `String` | Agent ID that performed the action |
| `action` | `AuditAction` | What happened (see below) |
| `resource` | `String` | Resource affected |
| `prev_hash` | `String` | Hash of the previous entry ("genesis" for first) |
| `hash` | `String` | blake3 hash of this entry |
| `metadata` | `Option<Value>` | Optional arbitrary metadata |

### Auditable Actions

The `AuditAction` enum captures all kernel events:

| Action | Description |
|--------|-------------|
| `AgentSpawn { task_type }` | Agent started |
| `AgentExit { reason }` | Agent stopped |
| `ToolCall { tool, args_json }` | Tool invoked |
| `ToolResult { tool, success }` | Tool returned |
| `MemoryWrite { entry_id }` | Memory entry written |
| `MemoryRead { entry_id }` | Memory entry read |
| `ConfigChange { key }` | Configuration modified |
| `ProgramInstall { program, version }` | Program installed |
| `CronTrigger { job_id }` | Scheduled job ran |
| `GitCommit { message }` | Git commit created |
| `AccessDenied { permission }` | Access was denied |
| `Other { detail }` | Unclassified event |

### Verification API

The `verify()` method walks the chain and checks:

1. **First entry** has `prev_hash` of `"genesis"` or `"pruned"`
2. **Every subsequent entry** has `prev_hash` matching the previous entry's `hash`
3. **Every entry's hash** can be independently recomputed from its fields
4. **Timestamps** are not in the future

```rust
let trail = AuditTrail::new(100_000);

// ... operations append entries ...

// Verify chain integrity
match trail.verify() {
    Ok(true) => println!("✓ Audit trail integrity verified"),
    Err(AuditError::ChainBroken { seq, expected, found }) => {
        eprintln!("✗ Chain broken at seq {seq}: expected {expected}, found {found}");
    }
    Err(AuditError::InvalidTimestamp { seq }) => {
        eprintln!("✗ Invalid timestamp at seq {seq}");
    }
    Err(e) => eprintln!("✗ Verification error: {e}"),
}
```

### Guardian Periodic Checks

The Oxios guardian program should periodically verify audit trail integrity:

```bash
# CLI verification (if exposed)
oxios audit
```

Programmatically, the guardian can schedule verification at regular intervals:

```rust
// Guardian pseudocode
loop {
    tokio::time::sleep(Duration::from_secs(300)).await; // every 5 minutes
    if let Err(e) = audit_trail.verify() {
        tracing::error!("AUDIT TRAIL INTEGRITY FAILURE: {e}");
        // Alert operators, freeze affected agents, trigger incident response
    }
}
```

### Auto-Pruning

When the trail exceeds `max_entries`, the oldest entries are pruned. The first
remaining entry has its `prev_hash` set to `"pruned"`, and verification accepts
this as a valid chain root. **Hashes are NOT recomputed** (O(1) instead of O(N)):

```
Before pruning:  [seq=1] → [seq=2] → [seq=3] → [seq=4] → [seq=5]
After pruning:                        [seq=3] → [seq=4] → [seq=5]
                                      prev="pruned"
```

### Export and Persistence

```rust
// Export as JSON from a specific sequence number
let json: String = trail.export_json(100)?;

// Export all entries
let all_json: String = trail.export_all_json()?;

// Persist to StateStore (file-backed)
trail.flush(&state_store)?;

// Restore from previously persisted entries
let entries: Vec<AuditEntry> = state_store.load_audit_entries()?;
trail.restore_from(entries);
```

The audit trail is persisted to `~/.oxios/workspace/audit/trail.json` via the
`StateStore`.

---

## 5. Circuit Breaker

The `CircuitBreaker` protects against **cascading LLM provider failures**.
When the provider is experiencing issues, the circuit opens and rejects requests
immediately instead of waiting for timeouts.

**Source:** `crates/oxios-kernel/src/circuit_breaker.rs`

### 3-State Model

```
         consecutive failures ≥ threshold
  Closed ─────────────────────────────────→ Open
    ↑                                        │
    │ success in half-open                   │ timeout elapsed
    │                                        ↓
    └──────────────────────── Half-Open ←────┘
                    failure in half-open → Open
```

| State | Behavior |
|-------|----------|
| **Closed** | Normal operation. All requests pass through. |
| **Open** | Provider is failing. Requests are **rejected immediately**. |
| **Half-Open** | Testing recovery. **Exactly one** probe request is allowed through. |

### Threshold and Timeout Configuration

```rust
use oxios_kernel::circuit_breaker::CircuitBreaker;

// Default: 5 failures → open, 30 second timeout
let cb = CircuitBreaker::default();

// Custom: 3 failures → open, 60 second timeout
let cb = CircuitBreaker::new(3, 60);
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `threshold` | 5 | Consecutive failures before opening |
| `timeout_secs` | 30 | Seconds before attempting reset |

### Usage Pattern

```rust
// Before making an LLM call
if !circuit_breaker.is_allowed() {
    return Err("Provider temporarily unavailable (circuit open)".into());
}

match llm_provider.call(prompt).await {
    Ok(response) => {
        circuit_breaker.record_success();
        Ok(response)
    }
    Err(e) => {
        circuit_breaker.record_failure();
        Err(e)
    }
}
```

### State Inspection

```rust
// Get current state as string ("closed", "open", "half_open")
let state = circuit_breaker.state();

// Get current failure count
let failures = circuit_breaker.failure_count();
```

### Thread Safety

The circuit breaker uses **lock-free atomic operations** (`AtomicU32`,
`AtomicBool`, `AtomicU64`) for all state transitions, making it safe for
concurrent use across multiple tokio tasks without locks.

---

## 6. Execution Security

The `ExecTool` is the primary interface for agents to execute commands. It
provides two modes with different security profiles.

**Source:** `crates/oxios-kernel/src/tools/exec_tool.rs`

### Shell Mode (`bash -c`, RBAC-Enforced)

Shell mode executes a raw command string via `bash -c`. It supports pipelines,
redirects, and compound commands.

**Security controls:**
- RBAC check: agent must have `bash` in `allowed_tools`
- Environment is **stripped** to a minimal set (`HOME`, `USER`, `LOGNAME`, `PATH`, `LANG`, `TERM`)
- Timeout enforcement with configurable limits
- All executions are audit-logged

```json
{
  "mode": "shell",
  "command": "cargo test --workspace 2>&1 | tail -20",
  "timeout": 120
}
```

### Structured Mode (Binary Allowlist + Metacharacter Blocking)

Structured mode executes a specific binary with explicit arguments. It enforces
stricter controls for host-sensitive operations.

**Security controls:**
1. **Binary must be a bare name** — no `/` or `..` (prevents path traversal)
2. **Binary must be in the allowlist** — configured via `[exec].allowed_commands`
3. **Arguments are validated** — shell metacharacters and `..` are blocked
4. **Environment is stripped** — same minimal set as shell mode
5. **Timeout enforcement** — capped at `max_timeout_secs`

#### Blocked Metacharacters

The following characters are **rejected** in structured-mode arguments:

```
|  &  ;  $  `  <  >  (  )  {  }  \n  \r  \0
```

Plus any argument containing `..` (path traversal).

#### Allowlist Configuration

```toml
[exec]
allowed_commands = ["git", "gh", "open", "shortcuts", "osascript"]
default_timeout_secs = 120
max_timeout_secs = 600
```

| Setting | Default | Description |
|---------|---------|-------------|
| `allowed_commands` | `git, gh, open, shortcuts, osascript` | Binaries allowed in structured mode |
| `default_timeout_secs` | 120 | Default timeout for commands |
| `max_timeout_secs` | 600 | Maximum allowed timeout |

#### Example Usage

```json
{
  "mode": "structured",
  "binary": "git",
  "args": ["status", "--short"],
  "timeout": 30
}
```

**Blocked examples:**
```json
// ✗ Path in binary name
{ "mode": "structured", "binary": "/usr/bin/rm", "args": ["-rf", "/"] }
// Error: "binary must be a bare name, not a path"

// ✗ Path traversal in binary
{ "mode": "structured", "binary": "../bin/evil", "args": [] }
// Error: "path traversal in binary name"

// ✗ Binary not in allowlist
{ "mode": "structured", "binary": "rm", "args": ["-rf", "/"] }
// Error: "binary 'rm' is not in the allowlist"

// ✗ Shell injection in arguments
{ "mode": "structured", "binary": "git", "args": ["log; rm -rf /"] }
// Error: "shell metacharacters or path traversal not allowed in arguments"

// ✗ Path traversal in arguments
{ "mode": "structured", "binary": "cat", "args": ["../../../etc/passwd"] }
// Error: "shell metacharacters or path traversal not allowed in arguments"
```

---

## 7. Authentication

The `AuthManager` provides **API key authentication** for the HTTP gateway,
using SHA-256 hashed keys stored on disk.

**Source:** `crates/oxios-kernel/src/auth.rs`

### API Key Format

All Oxios API keys use the prefix `oxios_` followed by 64 hex characters:

```
oxios_a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2
```

### Bearer Token Authentication

API keys are validated via the standard `Authorization: Bearer` header:

```bash
curl -H "Authorization: Bearer oxios_a1b2c3..." http://localhost:4200/api/v1/agents
```

### Key Management

```rust
use oxios_kernel::auth::AuthManager;

// Create with persistence (keys saved to disk)
let mut auth = AuthManager::with_persistence("~/.oxios/keys.json")?;

// Generate a new key (shown only once!)
let key: String = auth.generate_key("ci-pipeline")?;
println!("Save this key: {key}");

// Validate a token
let valid: bool = auth.validate("oxios_a1b2c3...");

// Revoke a key by name
auth.revoke_key("ci-pipeline")?;

// List all keys (metadata only — hashes never exposed)
let keys: Vec<&KeyMeta> = auth.list_keys();
for key in keys {
    println!("- {} (created: {})", key.name, key.created_at);
}
```

### Key Storage

Keys are stored as **SHA-256 hashes** — the original key material is never
persisted. The file uses atomic writes (temp file + rename) for crash safety:

```json
// ~/.oxios/keys.json
{
  "keys": [
    {
      "hash_hex": "sha256_hash_of_key...",
      "name": "ci-pipeline",
      "created_at": "2025-06-15T10:30:00Z",
      "last_used": "2025-06-15T14:22:00Z"
    }
  ]
}
```

### Configuration

```toml
[security]
auth_enabled = false  # Set to true to require API keys
```

> **Warning:** When `auth_enabled = false`, the gateway accepts all requests
> without authentication. Only use this in trusted local development.

---

## 8. Credential Store

The `CredentialStore` provides **multi-source credential resolution** for LLM
provider API keys. It follows a strict priority chain to find the best available
credential.

**Source:** `crates/oxios-kernel/src/credential.rs`

### Resolution Order

```
1. config.toml [engine].api_key     (explicit override — highest priority)
         ↓ not set or empty
2. ~/.oxi/auth.json                  (shared with oxi CLI)
         ↓ not found
3. Environment variables             (CI/CD, containers)
         ↓ not found
   ✗ No credential available
```

### Configuration

```toml
[engine]
# Option 1: Explicit API key (highest priority)
api_key = "sk-ant-api03-..."

# Option 2: Leave empty to fall through to oxi CLI or env vars
# api_key = ""

# Model in "provider/model" format
default_model = "anthropic/claude-sonnet-4-20250514"
```

### Environment Variables

When no config or auth store key is found, the system falls back to provider-specific
environment variables (via `oxi_sdk::get_env_api_key`):

```bash
# Common patterns:
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
```

### Credential Storage (Onboarding)

The onboarding wizard stores credentials to `~/.oxi/auth.json`, which is
shared with the `oxi` CLI if installed:

```rust
use oxios_kernel::credential::CredentialStore;

// Store an API key (called by onboarding wizard)
CredentialStore::store("anthropic", "sk-ant-api03-...")?;

// Check if any credential exists
let has = CredentialStore::has_credential("anthropic", None);

// Resolve the best available credential
if let Some((key, source)) = CredentialStore::resolve("anthropic", None) {
    println!("Found key from {:?}", source);
}
```

### Provider Extraction

Model IDs use the format `provider/model`. The credential store extracts the
provider name for credential lookup:

```rust
CredentialStore::provider_from_model("anthropic/claude-sonnet-4-20250514");
// → Some("anthropic")

CredentialStore::provider_from_model("openai/gpt-4o");
// → Some("openai")
```

---

## 9. Budget Management

The `BudgetManager` enforces **per-agent token and call limits** using sliding
window budgets that reset after a configurable time period. This prevents
resource exhaustion attacks and enforces fair usage policies.

**Source:** `crates/oxios-kernel/src/budget.rs`

### Budget Configuration

```rust
use oxios_kernel::budget::{BudgetManager, BudgetLimit};

let manager = BudgetManager::new();

manager.set_budget(BudgetLimit {
    agent_id: agent_uuid,
    token_budget: 100_000,    // 100K tokens per window
    calls_budget: 50,         // 50 API calls per window
    window_secs: 3600,        // 1-hour sliding window
});
```

### Token Reservation

Before making an LLM call, tokens must be **reserved**. If the budget is
exceeded, the call is rejected:

```rust
// Reserve tokens for a planned API call
match manager.reserve(&agent_id, 2000) {
    Ok(()) => {
        // Proceed with API call
    }
    Err(BudgetExceeded { kind, message, .. }) => {
        eprintln!("Budget exceeded ({:?}): {}", kind, message);
        // Queue for later or reject task
    }
}

// Release tokens back on error/retry
manager.release(&agent_id, 2000);
```

### Call Tracking

```rust
// Track each API call
match manager.track_call(&agent_id) {
    Ok(()) => { /* proceed */ }
    Err(BudgetExceeded { kind: BudgetKind::Call, .. }) => {
        eprintln!("Call budget exhausted");
    }
}
```

### Budget Inspection

```rust
let info: BudgetInfo = manager.remaining(&agent_id);
println!("Tokens remaining: {}", info.tokens_remaining);
println!("Calls remaining: {}", info.calls_remaining);
println!("Window resets in: {}s", info.window_remaining_secs);
println!("Exhausted: {}", info.is_exhausted);

// Check if agent can be scheduled
if manager.can_schedule(&agent_id) {
    // Schedule the agent
}
```

### Sliding Window Reset

Budgets use **sliding window semantics** — the usage counter resets
automatically when the window expires:

```
Window: 3600 seconds
  00:00 - 5000 tokens used
  00:30 - 12000 tokens used
  ...
  59:59 - 98000 tokens used
  01:00 - Window resets → 0 tokens used, full budget available
```

Manual reset is also available:

```rust
manager.reset_window(&agent_id);
```

### Persistence

```rust
// Persist budgets and usage to disk
manager.persist(Path::new("~/.oxios/workspace/budgets.json"))?;

// Restore from disk
manager.restore(Path::new("~/.oxios/workspace/budgets.json"))?;
```

---

## 10. Security Configuration

All security-related options in `~/.oxios/config.toml`:

```toml
[security]
# ─── Authentication ────────────────────────────────────────
# Enable API key authentication for the HTTP gateway.
# WARNING: When false, all requests are accepted without auth.
auth_enabled = false

# ─── CORS ──────────────────────────────────────────────────
# Allowed CORS origins for the web dashboard.
cors_origins = ["http://localhost:4200"]

# ─── Agent Permissions ─────────────────────────────────────
# Default allowed tools for new agents.
allowed_tools = ["read", "write", "edit", "bash", "grep", "find"]

# Whether agents can make network requests by default.
network_access = false

# Maximum execution time per agent in seconds (0 = unlimited).
max_execution_time_secs = 300

# Maximum memory per agent in MB (0 = unlimited).
max_memory_mb = 512

# Whether agents can fork (spawn sub-agents).
can_fork = false
```

### Execution Configuration

```toml
[exec]
# Binaries allowed for structured execution mode.
allowed_commands = ["git", "gh", "open", "shortcuts", "osascript"]

# Default command timeout in seconds.
default_timeout_secs = 120

# Maximum command timeout in seconds (agents cannot exceed this).
max_timeout_secs = 600
```

### Scheduler Configuration

```toml
[scheduler]
# Maximum number of agents running concurrently.
max_concurrent = 5

# Rate limit for agent scheduling (per minute).
rate_limit_per_minute = 60

# Time before an agent is considered a zombie and killed.
zombie_timeout_secs = 300
```

### Engine Configuration

```toml
[engine]
# Default model in "provider/model" format.
default_model = "anthropic/claude-sonnet-4-20250514"

# Explicit API key (highest priority, overrides all other sources).
# api_key = ""

# Provider fallback order (if configured).
# providers = ["anthropic", "openai"]
```

### Kernel Configuration

```toml
[kernel]
# Workspace directory for agent operations.
# workspace = "~/.oxios/workspace"

# Event bus channel capacity.
event_bus_capacity = 256

# Maximum number of agents.
max_agents = 10
```

### Gateway Configuration

```toml
[gateway]
# Bind address for the HTTP server.
# Use "127.0.0.1" for local-only access.
host = "0.0.0.0"

# Port for the web dashboard.
port = 4200
```

---

## 11. Security Best Practices

### Deployment Hardening

#### 1. Enable Authentication

```toml
[security]
auth_enabled = true
```

Generate and securely distribute API keys:

```bash
# API keys are managed via the onboarding wizard or config
# Save the displayed key securely — it won't be shown again
```

#### 2. Restrict Network Binding

For production deployments, bind the gateway to localhost only:

```toml
[gateway]
host = "127.0.0.1"  # Local only — use a reverse proxy for external access
```

Place behind a TLS-terminating reverse proxy (nginx, Caddy, etc.):

```nginx
server {
    listen 443 ssl;
    server_name oxios.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://127.0.0.1:4200;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
    }
}
```

#### 3. Configure CORS Strictly

Only allow trusted origins:

```toml
[security]
cors_origins = ["https://oxios.example.com"]
```

#### 4. Set Conservative Agent Permissions

```toml
[security]
allowed_tools = ["read", "write", "edit"]   # No bash by default
network_access = false
max_execution_time_secs = 120               # 2 minutes max
max_memory_mb = 256
can_fork = false
```

Grant additional permissions only to specific agents that need them.

#### 5. Restrict the Exec Allowlist

```toml
[exec]
allowed_commands = ["git"]  # Only git, nothing else
max_timeout_secs = 120      # 2 minutes maximum
```

Never include destructive commands (`rm`, `dd`, `mkfs`) in the allowlist.

#### 6. Protect Credential Files

```bash
# Restrict permissions on sensitive files
chmod 600 ~/.oxios/config.toml
chmod 600 ~/.oxios/keys.json
chmod 600 ~/.oxi/auth.json
chmod 700 ~/.oxios/
chmod 700 ~/.oxi/
```

#### 7. Regular Audit Trail Verification

Verify audit trail integrity regularly — automate with the guardian program
or a cron job:

```bash
# Manual verification
oxios audit
```

#### 8. Monitor Circuit Breaker State

Log circuit breaker state changes and set up alerts for sustained open states,
which indicate LLM provider issues.

#### 9. Use Workspace Sandboxing

Always assign agents to specific workspaces rather than allowing global file
system access:

```rust
access.register_workspace_path("my-project", PathBuf::from("/workspace/my-project"));
access.assign_workspace("agent-1", "my-project");
```

#### 10. Set Budget Limits

Configure per-agent budgets to prevent runaway token consumption:

```rust
manager.set_budget(BudgetLimit {
    agent_id,
    token_budget: 50_000,
    calls_budget: 25,
    window_secs: 3600,
});
```

### Environment-Specific Recommendations

| Environment | auth_enabled | gateway.host | exec.allowed_commands | security.network_access |
|-------------|-------------|-------------|-----------------------|------------------------|
| Development | `false` | `127.0.0.1` | `["git"]` | `false` |
| Staging | `true` | `127.0.0.1` | `["git", "gh"]` | `false` |
| Production | `true` | `127.0.0.1` + reverse proxy | `["git"]` | `false` |

---

## 12. Incident Response

### When Something Goes Wrong

#### Unauthorized Agent Activity Detected

1. **Immediately kill the agent:**
   ```bash
   oxios agent kill <agent-id>
   ```

2. **Revoke the agent's permissions:**
   ```rust
   access.remove_permissions("compromised-agent");
   ```

3. **Check the audit trail** for unauthorized actions:
   ```rust
   let denied: Vec<&AuditEntry> = access.denied_actions();
   let agent_entries: Vec<AuditEntry> = access.audit_log_for_agent("compromised-agent");
   ```

4. **Verify audit trail integrity:**
   ```rust
   match audit_trail.verify() {
       Ok(true) => println!("Trail intact"),
       Err(e) => eprintln!("⚠ TRAIL TAMPERED: {e}"),
   }
   ```

5. **Review RBAC audit log** for policy violations:
   ```rust
   let rbac_log = rbac.audit_log();
   let violations: Vec<_> = rbac_log.iter().filter(|e| !e.allowed).collect();
   ```

6. **Rotate API keys** if credential exposure is suspected:
   ```bash
   # Revoke by removing from config and restarting
   # Generate new key via onboarding wizard
   ```

#### LLM Provider Failure (Circuit Breaker Open)

1. **Check circuit breaker state:**
   ```rust
   let state = circuit_breaker.state();
   // "open" = failing, "half_open" = testing, "closed" = healthy
   ```

2. **Check failure count:**
   ```rust
   let failures = circuit_breaker.failure_count();
   ```

3. **If the provider is down**, the circuit breaker will automatically:
   - Reject requests immediately (no timeouts)
   - Attempt recovery after the configured timeout
   - Allow one probe request to test recovery

4. **Manual reset** (if confident the provider is healthy):
   ```rust
   circuit_breaker.record_success(); // Force close
   ```

#### Budget Exhaustion

1. **Check remaining budget:**
   ```rust
   let info = budget_manager.remaining(&agent_id);
   println!("Exhausted: {}", info.is_exhausted);
   ```

2. **Increase the budget** or **wait for window reset**:
   ```rust
   // Manual reset
   budget_manager.reset_window(&agent_id);

   // Or increase the limit
   budget_manager.set_budget(BudgetLimit {
       agent_id,
       token_budget: 200_000,
       calls_budget: 100,
       window_secs: 3600,
   });
   ```

#### Audit Trail Integrity Failure

1. **This is a critical security event.** The audit trail is designed to be
   tamper-evident — a verification failure means someone or something has
   modified historical entries.

2. **Immediate actions:**
   - Stop all agent operations
   - Export the current audit trail for forensic analysis:
     ```rust
     let json = audit_trail.export_all_json()?;
     std::fs::write("/secure/audit-forensics.json", json)?;
     ```
   - Identify the broken chain link:
     ```rust
     match audit_trail.verify() {
         Err(AuditError::ChainBroken { seq, expected, found }) => {
             eprintln!("Tampered entry at seq {seq}");
             eprintln!("Expected: {expected}");
             eprintln!("Found:    {found}");
         }
         _ => {}
     }
     ```

3. **Post-incident:** Restore from a known-good backup and investigate root cause.

#### Sandbox Escape

If an agent accesses files outside its workspace:

1. **The `can_access_path_in_workspace` method logs sandbox violations
   automatically.** Check the audit log for `sandbox_violation` entries.

2. **Review the agent's workspace assignment:**
   ```rust
   let workspace = access.get_workspace_for_agent("agent-name");
   let agents = access.list_agents_in_workspace("workspace-name");
   ```

3. **Tighten path restrictions:**
   ```rust
   let mut perms = access.get_or_create_permissions("agent-name");
   perms.deny_path("/etc/**");
   perms.deny_path("/root/**");
   perms.deny_path("**/.ssh/**");
   ```

---

## Quick Reference

### Security Checklist for New Deployments

- [ ] `auth_enabled = true` in `[security]`
- [ ] API keys generated and securely distributed
- [ ] `gateway.host = "127.0.0.1"` (behind reverse proxy)
- [ ] TLS configured via reverse proxy
- [ ] CORS origins restricted to trusted domains
- [ ] `exec.allowed_commands` restricted to minimum needed
- [ ] Agent permissions follow least privilege
- [ ] Workspace sandboxing enabled for all agents
- [ ] Budget limits set per agent
- [ ] Credential files have restrictive permissions (`chmod 600`)
- [ ] Audit trail verification scheduled (guardian/cron)
- [ ] Circuit breaker thresholds tuned for provider reliability

### Key File Locations

| Path | Purpose |
|------|---------|
| `~/.oxios/config.toml` | Main configuration (permissions, auth, exec) |
| `~/.oxios/keys.json` | API key hashes (SHA-256) |
| `~/.oxi/auth.json` | LLM provider credentials (shared with oxi CLI) |
| `~/.oxios/workspace/audit/trail.json` | Cryptographic audit trail |

### Emergency Commands

```bash
# Kill a runaway agent
oxios agent kill <agent-id>

# Revoke an API key
# Revoke by editing config.toml

# Verify audit trail
oxios audit

# Generate new API key
# Use oxios onboard to configure credentials
```
