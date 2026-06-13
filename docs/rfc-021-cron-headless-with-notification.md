# RFC-021: Cron Headless Execution + Dream Review + Notification Center

> **Status:** Draft
> **Created:** 2026-06-08
> **Author:** Won
> **Review:** 2026-06-08 — issues found (see Review section at bottom)

## Problem

Cron jobs fire at scheduled times — **no user is present to participate in the Ouroboros interview**. Currently, CronScheduler fires a `(job_id, goal)` tuple that gets executed as a Seed. But:

1. Cron job goals are often **vague** or **incomplete** because they were written in a hurry.
2. Repeated failures go unnoticed — the user only sees `last_success: false`.
3. Constraints and acceptance criteria are empty for most cron jobs — no quality gate.
4. There's no feedback loop. The user creates a cron job and never improves it.

The Ouroboros protocol is explicitly designed to refine vague goals through interview. But for cron, the user isn't there. We need a different path.

## Solution: Three-Layer Design

```
┌─────────────────────────────────────────────────────────────────────┐
│ Layer 1: Headless Cron Execution                                    │
│   CronScheduler → CronJob → Seed (direct, no interview) → Execute │
│                                                                     │
│ Layer 2: Dream Review                                               │
│   Dream Phase 2.5 → review cron jobs + execution history           │
│                    → detect quality issues                          │
│                    → generate ImprovementSuggestion                 │
│                    → write to NotificationStore                     │
│                                                                     │
│ Layer 3: Notification → Interview Bridge                            │
│   Web UI notification badge → click → Gateway creates session      │
│     → Orchestrator.handle_cron_improvement(suggestion_id)          │
│     → Interview starts with cron job context pre-loaded             │
│     → User refines the cron job through Ouroboros interview        │
│     → CronJob updated in-place                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Headless Cron Execution

### `CronExecutionMode`

```rust
/// How a cron job creates its Seed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CronExecutionMode {
    /// Skip interview, create Seed directly from CronJob fields.
    /// Used when no user is present (cron tick, API trigger).
    #[default]
    Headless,
    /// Run full Ouroboros interview (only when user clicks "improve").
    Interactive,
}
```

No changes to `CronJob` struct — the mode is implicit from the execution context. When `CronScheduler::tick_inner` fires, it always uses `Headless`. When the user clicks "improve" in the notification center, it uses `Interactive`.

### `CronJob::to_seed()`

New method on `CronJob` that converts a job definition directly into a Seed, no LLM call:

```rust
impl CronJob {
    /// Convert this cron job into a Seed for headless execution.
    ///
    /// The seed is constructed from the job's goal, constraints, and
    /// acceptance criteria. If criteria are empty, a default "completes
    /// without error" criterion is added.
    pub fn to_seed(&self) -> Seed {
        let criteria = if self.acceptance_criteria.is_empty() {
            vec!["Task completes without errors".to_string()]
        } else {
            self.acceptance_criteria.clone()
        };

        Seed {
            id: Uuid::new_v4(),
            goal: self.goal.clone(),
            constraints: if self.constraints.is_empty() {
                vec!["Automated execution — no user interaction available".to_string()]
            } else {
                self.constraints.clone()
            },
            acceptance_criteria: criteria,
            ontology: None,
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
            original_request: format!("[cron:{}]", self.name),
            output_schema: None,
        }
    }
}
```

### Execution Flow (unchanged from current, just explicit)

```
CronScheduler.tick_inner()
  → due jobs found
  → for each job: job.to_seed()
  → lifecycle.spawn_and_run(&seed, job.priority)
  → mark_job_completed(id, success, summary)
```

The `executor` closure in `CronScheduler::start()` already receives `(Uuid, String)`. We extend it to receive the full `CronJob` (or at minimum `CronJob::to_seed()` output) so the lifecycle gets a proper Seed instead of a raw goal string.

---

## Layer 2: Dream Review — Cron Quality Audit

### New Dream Phase: Phase 2.5 (Cron Review)

After the existing Phase 2 (Gather Signal) and before Phase 3 (Consolidate), we insert an optional **Cron Review** pass.

```rust
/// Result of reviewing cron jobs during Dream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronReviewResult {
    /// Suggestions generated for cron jobs.
    pub suggestions: Vec<ImprovementSuggestion>,
}
```

### `ImprovementSuggestion`

```rust
/// A suggestion that a cron job could be improved, generated during Dream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementSuggestion {
    /// Unique ID.
    pub id: Uuid,
    /// The cron job this suggestion targets.
    pub cron_job_id: Uuid,
    /// Cron job name (denormalized for display).
    pub cron_job_name: String,
    /// Category of the issue.
    pub category: SuggestionCategory,
    /// Human-readable description of the issue.
    pub description: String,
    /// Suggested fix (what the user should do).
    pub suggested_fix: String,
    /// Severity (higher = more urgent).
    pub severity: f64,
    /// When this suggestion was created.
    pub created_at: DateTime<Utc>,
    /// Execution history that triggered this suggestion.
    pub evidence: Vec<CronExecutionEvidence>,
    /// Whether the user has dismissed this suggestion.
    pub dismissed: bool,
}

/// Category of cron improvement issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionCategory {
    /// Goal is vague — could be more specific.
    VagueGoal,
    /// Missing constraints — could lead to unsafe or unwanted behavior.
    MissingConstraints,
    /// No acceptance criteria — no quality gate.
    MissingCriteria,
    /// Repeated failures — something is wrong with the prompt.
    RepeatedFailures,
    /// Goal is too broad — should be split into multiple jobs.
    TooBroad,
    /// Job hasn't been modified in a long time and might be stale.
    StaleDefinition,
    /// Execution results vary wildly — goal might be ambiguous.
    InconsistentResults,
}

/// Evidence from cron execution history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronExecutionEvidence {
    /// When this execution happened.
    pub executed_at: DateTime<Utc>,
    /// Whether it succeeded.
    pub success: bool,
    /// Result summary.
    pub summary: String,
}
```

### Detection Logic

The cron review pass examines each cron job and its execution history using **rule-based heuristics** (no LLM needed for the initial version):

| Rule | Condition | Category |
|------|-----------|----------|
| **Goal too short** | `goal.len() < 20` | `VagueGoal` |
| **No constraints** | `constraints.is_empty()` | `MissingConstraints` |
| **No criteria** | `acceptance_criteria.is_empty()` | `MissingCriteria` |
| **3+ consecutive failures** | `last 3 runs all failed` | `RepeatedFailures` |
| **Goal too broad** | `goal.len() > 500 && criteria.len() < 2` | `TooBroad` |
| **Not modified in 30+ days** | `created_at + 30 days < now && run_count > 10` | `StaleDefinition` |
| **Inconsistent results** | `last 10 runs: success rate between 30-70%` | `InconsistentResults` |

These are cheap O(1) checks — no embedding, no LLM. They run during every Dream cycle.

**Future enhancement:** Use LLM to analyze the goal text for semantic issues ("this goal contains two unrelated tasks", "this goal references a file that may not exist"). This would be a `DreamConfig::cron_review_llm_enabled` flag.

### Persistence

Suggestions are stored via `StateStore`:

```
~/.oxios/workspace/notifications/{id}.json
```

---

## Layer 3: Notification Center + Interview Bridge

### `NotificationStore`

A new kernel subsystem that stores and manages user-facing notifications.

```rust
/// Notification store — user-facing alerts generated by the system.
///
/// Currently used for cron improvement suggestions. Will expand to
/// include system alerts, approval requests, etc.
pub struct NotificationStore {
    state_store: Arc<StateStore>,
    notifications: Arc<RwLock<Vec<Notification>>>,
}

/// A user-facing notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique ID.
    pub id: Uuid,
    /// Notification type.
    pub kind: NotificationKind,
    /// Human-readable title.
    pub title: String,
    /// Human-readable body.
    pub body: String,
    /// When this notification was created.
    pub created_at: DateTime<Utc>,
    /// Whether the user has read it.
    pub read: bool,
    /// Whether the user dismissed it (won't show again).
    pub dismissed: bool,
    /// Optional action the user can take.
    pub action: Option<NotificationAction>,
    /// Priority for display ordering.
    pub priority: NotificationPriority,
}

/// What kind of notification this is.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// A cron job could be improved.
    CronImprovement,
    /// A cron job has failed repeatedly.
    CronFailure,
    /// System-level alert.
    SystemAlert,
    /// Approval requested.
    ApprovalRequired,
}

/// An actionable step the user can take from a notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAction {
    /// Open the chat with a pre-configured interview for this cron job.
    ImproveCron {
        /// The cron job to improve.
        cron_job_id: Uuid,
        /// The suggestion that triggered this.
        suggestion_id: Uuid,
    },
    /// Open settings to reconfigure.
    Configure {
        /// What to configure.
        target: String,
    },
    /// Dismiss without action.
    Dismiss,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}
```

### API (via KernelHandle)

```rust
impl NotificationStore {
    /// Get unread notification count (for badge).
    pub fn unread_count(&self) -> usize;

    /// List all active (not dismissed) notifications.
    pub fn list_notifications(&self) -> Vec<Notification>;

    /// Mark a notification as read.
    pub async fn mark_read(&self, id: Uuid);

    /// Dismiss a notification (won't show again).
    pub async fn dismiss(&self, id: Uuid);

    /// Add a new notification.
    pub async fn add(&self, notification: Notification);

    /// Clean up notifications older than N days.
    pub async fn prune(&self, max_age_days: u32);
}
```

### Gateway: Notification Click → Interview

When the user clicks "개선하기" on a notification, the flow is:

```
Frontend (click "Improve")
  → POST /api/notifications/{id}/action
    → Gateway::handle_notification_action(id)
      → Load Notification → extract NotificationAction::ImproveCron { cron_job_id, suggestion_id }
      → Load CronJob
      → Load ImprovementSuggestion
      → Construct pre-loaded interview context:

        "당신이 등록한 cron job '[job.name]'을(를) 개선하고 싶습니다.
         현재 goal: {job.goal}
         현재 constraints: {job.constraints:?}
         발견된 문제: {suggestion.description}
         제안: {suggestion.suggested_fix}

         이 cron job을 어떻게 개선할까요?"

      → Orchestrator::handle_message(user_id, context, session_id=None, ...)
        → Ouroboros interview starts with full cron context
        → User refines the job through multi-turn Q&A
        → On interview completion, the Seed's goal/constraints/criteria
          are applied back to the CronJob
        → CronJob updated via CronScheduler::update_job()
```

### Orchestrator Extension

A new method on Orchestrator specifically for cron improvement flows:

```rust
impl Orchestrator {
    /// Handle a cron improvement flow triggered from the notification center.
    ///
    /// This is a thin wrapper around `handle_message` that pre-loads
    /// the cron job context and sets up the session so the interview
    /// focuses on improving the specific job.
    pub async fn handle_cron_improvement(
        &self,
        user_id: &str,
        cron_job_id: Uuid,
        suggestion_id: Uuid,
    ) -> Result<OrchestrationResult> {
        // 1. Load the cron job and suggestion
        // 2. Build the initial message with full context
        // 3. Call handle_message with the context
        // 4. The Ouroboros interview naturally focuses on the cron job
        // 5. After successful seed generation, apply changes back to CronJob
    }
}
```

### Auto-Apply Interview Results

When the Ouroboros interview completes and produces a Seed for a cron improvement session, the `handle_cron_improvement` method:

1. Extracts `goal`, `constraints`, `acceptance_criteria` from the final Seed.
2. Calls `CronScheduler::update_job(cron_job_id, CronJobUpdate { goal, constraints, acceptance_criteria, ... })`.
3. Marks the suggestion and notification as resolved.
4. Returns the result to the user.

---

## Data Flow Diagram

```
                    ┌─────────────┐
                    │ CronScheduler│
                    │  (tick loop) │
                    └──────┬──────┘
                           │ fires due job
                           ▼
              ┌────────────────────────┐
              │  CronJob::to_seed()    │
              │  (Headless: no interview)│
              └────────────┬───────────┘
                           │ Seed
                           ▼
              ┌────────────────────────┐
              │  LifecycleManager      │
              │  spawn_and_run(seed)   │
              └────────────┬───────────┘
                           │ CronJobResult
                           ▼
              ┌────────────────────────┐
              │  mark_job_completed()  │
              │  stores last_result,   │
              │  last_success, run_count│
              └────────────────────────┘

       ──────────── Dream Cycle ──────────────

              ┌────────────────────────┐
              │  Dream Phase 2.5:      │
              │  Cron Review           │
              │                        │
              │  For each CronJob:     │
              │  • Check rules         │
              │  • Analyze history     │
              │  • Generate suggestions│
              └────────────┬───────────┘
                           │ ImprovementSuggestion[]
                           ▼
              ┌────────────────────────┐
              │  NotificationStore     │
              │  .add(Notification {   │
              │    kind: CronImprovement│
              │    action: ImproveCron │
              │  })                    │
              └────────────────────────┘

       ──────────── User Interaction ──────────

              ┌────────────────────────┐
              │  Web UI: Notification  │
              │  Badge (unread count)  │
              │  → Click "개선하기"     │
              └────────────┬───────────┘
                           │
                           ▼
              ┌────────────────────────┐
              │  Gateway               │
              │  POST /notifications/  │
              │    {id}/action         │
              └────────────┬───────────┘
                           │
                           ▼
              ┌────────────────────────┐
              │  Orchestrator          │
              │  .handle_cron_         │
              │   improvement()        │
              │                        │
              │  → Pre-loads context   │
              │  → Ouroboros interview │
              │  → User refines job    │
              │  → Updates CronJob     │
              └────────────────────────┘
```

---

## KernelEvent Extensions

New event variants for the notification system:

```rust
// In KernelEvent enum:
NotificationCreated {
    id: Uuid,
    kind: String,
    title: String,
    priority: String,
},
NotificationRead {
    id: Uuid,
},
NotificationDismissed {
    id: Uuid,
},
CronImprovementApplied {
    cron_job_id: Uuid,
    suggestion_id: Uuid,
    fields_updated: Vec<String>,
},
```

---

## Web UI

### Notification Badge

- Global bell icon in the top-right corner (like standard notification patterns).
- Badge count = `NotificationStore::unread_count()`.
- Dropdown lists notifications sorted by `priority` desc, then `created_at` desc.

### Cron Improvement Notification Card

```
┌──────────────────────────────────────────────────┐
│ ⚠️ Cron job 개선 제안                            │
│                                                    │
│ "매일 아침 뉴스 요약" 잡의 goal이 너무 모호합니다. │
│ 구체적으로 어떤 소스, 어떤 형식으로 요약할지       │
│ 명시하면 더 정확한 결과를 얻을 수 있습니다.        │
│                                                    │
│ [개선하기]  [나중에]  [무시하기]                    │
└──────────────────────────────────────────────────┘
```

- **개선하기**: Opens the chat. First message is pre-populated with the cron improvement context. The Ouroboros interview immediately starts refining the job.
- **나중에**: Marks notification as read but keeps it.
- **무시하기**: Dismisses the notification + suggestion. Won't be suggested again for 7 days.

### Chat Integration

When the user clicks "개선하기", the chat panel opens with:

```
🤖 "매일 아침 뉴스 요약" cron job을 개선해보겠습니다.

현재 설정:
- Goal: "뉴스 요약해줘"
- Schedule: 0 9 * * *
- Constraints: (없음)
- Acceptance Criteria: (없음)

발견된 문제: Goal이 너무 모호합니다.

먼저, 어떤 소스에서 뉴스를 가져올까요?
  [Naver 뉴스]  [Hacker News]  [Reddit]  [기타]
```

The interview proceeds normally through the Ouroboros protocol. When it completes, the CronJob is updated in-place with the refined goal/constraints/criteria.

---

## File Changes

### New Files

| File | Purpose |
|------|---------|
| `crates/oxios-kernel/src/notification.rs` | NotificationStore, Notification types |
| `crates/oxios-kernel/src/cron_review.rs` | Cron review logic for Dream Phase 2.5 |

### Modified Files

| File | Change |
|------|--------|
| `crates/oxios-kernel/src/cron.rs` | Add `CronJob::to_seed()`, extend `CronJob` with execution history, add `CronExecutionMode` |
| `crates/oxios-memory/src/memory/dream.rs` | Add Phase 2.5 cron review pass |
| `crates/oxios-kernel/src/event_bus.rs` | Add Notification/CronImprovement events |
| `crates/oxios-kernel/src/orchestrator.rs` | Add `handle_cron_improvement()` |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | Add NotificationApi |
| `crates/oxios-kernel/src/lib.rs` | Register new modules |
| `crates/oxios-gateway/` | Add notification endpoints, cron improvement bridge |
| `surface/oxios-web/web/` | Notification badge, improvement flow UI |

---

## Configuration

```toml
[notifications]
# Enable the notification system.
enabled = true
# Auto-prune notifications older than this (days).
max_age_days = 30

[cron_review]
# Enable cron job review during Dream.
enabled = true
# Minimum number of runs before suggesting improvements.
min_runs_for_review = 3
# Consecutive failures that trigger a RepeatedFailures suggestion.
consecutive_failure_threshold = 3
# Days without modification that trigger a StaleDefinition suggestion.
stale_days_threshold = 30
# Inconsistency window (success rate between these triggers InconsistentResults).
inconsistency_low = 0.3
inconsistency_high = 0.7
# Re-suggest after N days if previously dismissed.
re_suggest_after_days = 7
```

---

## Implementation Order

1. **`CronJob::to_seed()`** — Pure function, no deps. Makes headless execution explicit.
2. **`NotificationStore`** — Standalone subsystem, only depends on StateStore.
3. **`CronReview` rules** — Pure functions, testable in isolation.
4. **Dream Phase 2.5 integration** — Wire cron review into DreamProcess.
5. **`Orchestrator::handle_cron_improvement()`** — Uses existing handle_message + new CronScheduler::update_job.
6. **Gateway endpoint** — `POST /notifications/{id}/action`.
7. **Web UI** — Notification badge + improvement chat flow.

---

## Open Questions

1. **Should cron review use LLM?** Initially no — rule-based heuristics are cheap and deterministic. A future `cron_review_llm_enabled` flag could enable semantic analysis of the goal text. This would run during Dream when the LLM is already being used for memory compaction, so no extra cost concern.

2. **Should we store full execution history?** Currently only `last_result` and `last_success` are stored. For better review quality, we could store the last N execution results in a ring buffer per job. Suggested: `execution_history: VecDeque<CronJobResult>` with max 20 entries, pruned during Dream.

3. **Multiple suggestions per job?** Yes — a single cron job can have multiple issues (e.g., vague goal + missing constraints + repeated failures). Each generates a separate notification. The user can address them one at a time or all at once.

4. **Notification priority for repeated failures?** `Critical` — this means the job is broken right now. `VagueGoal` is `Low` — it works but could be better.

5. **Race condition: Dream writes notifications while user reads?** `NotificationStore` uses `RwLock` internally — readers never block each other, writers get exclusive access. The notification list is small (typically <50), so write contention is negligible.

---

## Review (2026-06-08)

### 🔴 Critical: Circular Dependency — Dream → CronScheduler

**Problem:** RFC proposes Dream Phase 2.5 to review cron jobs. But dependency direction is:

```
oxios-kernel → oxios-memory
```

`DreamProcess` lives in `oxios-memory`. `CronScheduler`, `CronJob` live in `oxios-kernel`. If Dream needs to access CronJob data, `oxios-memory` → `oxios-kernel` circular dependency is created.

**Fix:** Don't put cron review inside Dream. Instead, create a **separate `CronReviewDaemon`** in `oxios-kernel` that runs on the same periodic timer infrastructure. Kernel already controls when Dream runs — it can run cron review after Dream completes, on the same timer, but as an independent kernel-level concern.

```
// In kernel.rs (binary crate):
// After Dream completes:
if let Some(review) = cron_review_daemon.review_all(&cron_scheduler).await {
    for suggestion in review.suggestions {
        notification_store.add(suggestion.into()).await;
    }
}
```

This keeps Dream pure (memory consolidation only) and cron review in the kernel where it belongs.

---

### 🔴 Critical: Interview → Auto-Apply Doesn't Map to Current Ouroboros

**Problem:** RFC says after the Ouroboros interview completes, the Seed's goal/constraints/criteria are "applied back to the CronJob." But the current Ouroboros flow is:

```
Interview → Seed → Execute → Evaluate → (Evolve loop)
```

For cron improvement, we **don't want to execute the task**. We want:

```
Interview → Seed → Update CronJob (stop here, no execution)
```

Calling `handle_message` directly would execute the task, not just update the definition. A user clicking "improve" on a "매일 뉴스 요약" cron job doesn't want the LLM to actually run the news summary right now — they want to refine the cron job's definition.

**Fix:** `handle_cron_improvement` must intercept **after seed generation and before execution**. This requires a new code path:

```rust
impl Orchestrator {
    pub async fn handle_cron_improvement(&self, ...) -> Result<OrchestrationResult> {
        // 1. Build context message from CronJob + Suggestion
        // 2. Run interview phase (same as handle_message)
        // 3. Run seed generation (same as handle_message)
        // *** STOP HERE — no execute, evaluate, evolve ***
        // 4. Extract goal/constraints/criteria from Seed
        // 5. Call CronScheduler::update_job() with the Seed fields
        // 6. Return result to user showing what changed
    }
}
```

This is a **read-only interview** — interview + seed generation for configuration, not execution. The current `handle_message` can't do this without a flag to skip execution. Options:
- A) Add `skip_execution: bool` parameter to `handle_message`
- B) Extract interview + seed generation into a separate method that both `handle_message` and `handle_cron_improvement` call
- C) Duplicate the interview + seed logic in `handle_cron_improvement`

**Recommendation: B** — extract `conduct_interview()` and `generate_seed_from_interview()` as reusable methods.

---

### 🟡 Moderate: `CronExecutionMode` Enum Is Dead Code

**Problem:** The enum is defined but RFC explicitly says "the mode is implicit from the execution context." It's never stored, never passed, never matched on. It's documentation masquerading as code.

**Fix:** Remove `CronExecutionMode`. The behavior is already clear from the code path: `tick_inner` = headless, notification click = interactive. No enum needed.

---

### 🟡 Moderate: Execution History Doesn't Exist Yet

**Problem:** Most detection rules depend on execution history:
- "3+ consecutive failures" → need last N results
- "last 10 runs: success rate 30-70%" → need last 10 results

But `CronJob` only stores `last_result: Option<String>`, `last_success: Option<bool>`, `run_count: u64`. There's no history.

**Fix:** This is a prerequisite, not an afterthought. Add to `CronJob`:

```rust
#[serde(default)]
pub execution_history: VecDeque<CronExecutionRecord>,
```

Where `CronExecutionRecord { executed_at, success, summary }` is capped at ~20 entries. `mark_job_completed` appends to this ring buffer. This must be implemented **before** the cron review rules.

Move this from "Open Question" to a required step in Implementation Order (step 0).

---

### 🟡 Moderate: Dream Phase Coupling Is Wrong Abstraction

**Problem:** Even with the circular dependency fix (review daemon in kernel), running cron review "during Dream" is semantically wrong. Dream = memory consolidation. Cron review = job quality audit. These are unrelated.

Users might disable Dream (`dream_enabled = false`) but still want cron review. Or vice versa.

**Fix:** Make `CronReviewDaemon` a completely independent subsystem with its own schedule:

```toml
[cron_review]
enabled = true
interval_hours = 24  # independent of Dream interval
```

The binary crate (`kernel.rs`) runs both daemons on their own timers. No coupling.

---

### 🟡 Moderate: `executor` Closure Signature Breaking Change

**Problem:** RFC says "We extend [the executor closure] to receive the full CronJob." Current signature:

```rust
F: Fn(Uuid, String) -> Fut
```

Changing to `Fn(Uuid, Seed)` or `Fn(CronJob)` breaks all existing callers. The RFC doesn't address migration.

**Fix:** Two options:
- A) Don't change the signature. The caller's executor closure internally converts goal → seed. CronScheduler doesn't need to know about Seeds.
- B) Add a new `start_with_seed()` method with the new signature, keep `start()` as-is for backward compat.

**Recommendation: A** — simpler. The `to_seed()` conversion is the caller's responsibility. CronScheduler stays focused on scheduling.

---

### 🟢 Minor: Duplicate Dismissed State

`ImprovementSuggestion.dismissed` and `Notification.dismissed` represent the same state. If the notification is dismissed, the suggestion should also be considered dismissed. Two sources of truth will drift.

**Fix:** Remove `dismissed` from `ImprovementSuggestion`. Only `Notification.dismissed` is authoritative. When checking whether to re-suggest, query the notification store.

---

### 🟢 Minor: Two Severity/Priority Systems

`ImprovementSuggestion.severity: f64` and `Notification.priority: NotificationPriority` (enum) are two parallel systems for the same concept.

**Fix:** Drop `severity: f64` from `ImprovementSuggestion`. Map `SuggestionCategory` directly to `NotificationPriority`:

```rust
impl SuggestionCategory {
    fn notification_priority(&self) -> NotificationPriority {
        match self {
            Self::RepeatedFailures => NotificationPriority::Critical,
            Self::InconsistentResults => NotificationPriority::High,
            Self::MissingConstraints => NotificationPriority::Normal,
            Self::VagueGoal
            | Self::MissingCriteria
            | Self::TooBroad
            | Self::StaleDefinition => NotificationPriority::Low,
        }
    }
}
```

---

### 🟢 Minor: "Re-suggest after 7 days" Not Implemented

RFC mentions this in the UI section but provides no mechanism. Need either:
- A `dismissed_at` timestamp on Notification + cleanup check during cron review
- Or just don't re-suggest dismissed items (simpler, and probably better UX — if user dismissed it, respect that)

---

### 🟢 Minor: Weak Heuristic — `goal.len() > 500 → TooBroad`

Long ≠ broad. A detailed goal with specific instructions is better than a short one. This heuristic will false-positive on well-written goals.

**Fix:** Remove the `TooBroad` category entirely, or change the heuristic to count *independent tasks* (e.g., number of verbs/action words). For v1, skip `TooBroad` — it's the weakest rule.

---

### Summary

| # | Severity | Issue | Fix |
|---|----------|-------|----|
| 1 | 🔴 Critical | Circular dependency: Dream (memory) → CronScheduler (kernel) | Move cron review to independent `CronReviewDaemon` in kernel |
| 2 | 🔴 Critical | Interview auto-apply goes through execute path | Intercept after seed gen, skip execution. Extract reusable interview method |
| 3 | 🟡 Moderate | `CronExecutionMode` enum unused | Delete it |
| 4 | 🟡 Moderate | Execution history doesn't exist | Prerequisite: add `VecDeque<CronExecutionRecord>` to CronJob |
| 5 | 🟡 Moderate | Dream coupling wrong abstraction | Independent `CronReviewDaemon` with own schedule |
| 6 | 🟡 Moderate | executor closure signature breaking change | Keep signature, caller does to_seed() |
| 7 | 🟢 Minor | Duplicate dismissed state | Single source: Notification.dismissed |
| 8 | 🟢 Minor | Two severity/priority systems | Drop severity, derive priority from category |
| 9 | 🟢 Minor | Re-suggest mechanism missing | Remove or implement explicitly |
| 10 | 🟢 Minor | `TooBroad` heuristic weak | Remove for v1 |

### Revised Implementation Order

0. **Add execution history** — `VecDeque<CronExecutionRecord>` on CronJob, updated in `mark_job_completed`
1. **`CronJob::to_seed()`** — Pure function
2. **`NotificationStore`** — Standalone, kernel-internal
3. **`CronReviewDaemon`** — Independent kernel subsystem (NOT in Dream)
4. **`Orchestrator::extract_interview_methods`** — Refactor: extract interview + seed gen from `handle_message`
5. **`Orchestrator::handle_cron_improvement`** — Interview → Seed → Update CronJob (no execution)
6. **Gateway endpoint** — `POST /notifications/{id}/action`
7. **Web UI** — Notification badge + improvement chat flow
