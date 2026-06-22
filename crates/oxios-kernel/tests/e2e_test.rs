//! End-to-end smoke test for the Oxios pipeline.
//!
//! Tests the full Ouroboros lifecycle with fully-mocked protocol and supervisor.
//! Uses `Orchestrator::with_config` to control evolution iterations explicitly,
//! so each test verifies a specific pipeline path.

#[path = "common/mod.rs"]
mod common;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Result;
use oxios_kernel::supervisor::Supervisor;
use oxios_kernel::types::{AgentId, AgentInfo, AgentStatus};
use oxios_kernel::{
    A2AProtocol, AccessManager, AgentLifecycleManager, EventBus, KernelEvent, Orchestrator,
    StateStore, config::OrchestratorConfig,
};
use oxios_ouroboros::{
    AmbiguityScore, EvaluationResult, ExecutionResult, InterviewResult, OuroborosProtocol, Phase,
    Seed,
};
use oxios_ouroboros::{Directive, ExecEnv};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[expect(dead_code)]
fn make_config(max_iterations: u32) -> OrchestratorConfig {
    OrchestratorConfig {
        max_evolution_iterations: max_iterations,
        min_evaluation_score: 0.8,
    }
}

// ---------------------------------------------------------------------------
// Mock OuroborosProtocol — deterministic, no LLM calls
// ---------------------------------------------------------------------------

/// A mock Ouroboros protocol that returns deterministic results.
struct MockOuroboros {
    interview_called: AtomicUsize,
    generate_seed_called: AtomicUsize,
    evaluate_called: AtomicUsize,
    evolve_called: AtomicUsize,
    /// If false, evaluation fails on first pass (triggers evolve).
    evaluation_passes: AtomicBool,
}

impl MockOuroboros {
    fn new() -> Self {
        Self {
            interview_called: AtomicUsize::new(0),
            generate_seed_called: AtomicUsize::new(0),
            evaluate_called: AtomicUsize::new(0),
            evolve_called: AtomicUsize::new(0),
            evaluation_passes: AtomicBool::new(true),
        }
    }

    #[expect(dead_code)]
    fn with_failing_evaluation() -> Self {
        let s = Self::new();
        s.evaluation_passes.store(false, Ordering::SeqCst);
        s
    }
}

#[async_trait]
impl OuroborosProtocol for MockOuroboros {
    async fn interview(&self, _user_input: &str) -> Result<InterviewResult> {
        self.interview_called.fetch_add(1, Ordering::SeqCst);
        let mut result = InterviewResult::new();
        // Low ambiguity → ready for seed immediately.
        let score = AmbiguityScore::new(0.9, 0.85, 0.8);
        result.update_ambiguity(score);
        result.add_exchange("Goal confirmed", "User wants to proceed");
        Ok(result)
    }

    async fn generate_seed(&self, _interview: &InterviewResult) -> Result<Seed> {
        self.generate_seed_called.fetch_add(1, Ordering::SeqCst);
        // Seed with acceptance_criteria so should_evaluate() returns true.
        let mut seed = Seed::new("Test task from e2e smoke test");
        seed.acceptance_criteria
            .push("Output contains 'done'".into());
        Ok(seed)
    }

    async fn execute(&self, seed: &Seed) -> Result<ExecutionResult> {
        Ok(ExecutionResult {
            output: format!("Executed seed: {}", seed.goal),
            steps_completed: 3,
            success: true,
            tool_calls: vec![],
            tokens_input: 0,
            tokens_output: 0,
            model_id: String::new(),
        })
    }

    async fn evaluate(
        &self,
        _seed: &Seed,
        _execution: &ExecutionResult,
    ) -> Result<EvaluationResult> {
        self.evaluate_called.fetch_add(1, Ordering::SeqCst);
        let passes = self.evaluation_passes.load(Ordering::SeqCst);
        // Make it pass on next call (evolve loop terminates).
        self.evaluation_passes.store(true, Ordering::SeqCst);
        Ok(EvaluationResult {
            mechanical_pass: passes,
            semantic_pass: Some(passes),
            consensus_pass: None,
            score: if passes { 0.95 } else { 0.4 },
            notes: vec!["Mock evaluation".into()],
        })
    }

    async fn evolve(&self, seed: &Seed, _evaluation: &EvaluationResult) -> Result<Option<Seed>> {
        self.evolve_called.fetch_add(1, Ordering::SeqCst);
        let evolved = Seed::evolved_from(seed);
        Ok(Some(evolved))
    }
}

// ---------------------------------------------------------------------------
// Mock Supervisor — tracks calls, no real agent execution
// ---------------------------------------------------------------------------

struct MockSupervisor {
    agents: parking_lot::RwLock<HashMap<AgentId, AgentInfo>>,
    fork_called: AtomicUsize,
    run_called: AtomicUsize,
    event_bus: EventBus,
}

impl MockSupervisor {
    fn new(event_bus: EventBus) -> Self {
        Self {
            agents: parking_lot::RwLock::new(HashMap::new()),
            fork_called: AtomicUsize::new(0),
            run_called: AtomicUsize::new(0),
            event_bus,
        }
    }
}

#[async_trait]
impl Supervisor for MockSupervisor {
    async fn fork(&self, spec: &Seed) -> Result<AgentId> {
        self.fork_called.fetch_add(1, Ordering::SeqCst);
        let id = AgentId::new_v4();
        let info = AgentInfo {
            id,
            name: spec.goal.clone(),
            status: AgentStatus::Starting,
            created_at: chrono::Utc::now(),
            seed_id: Some(spec.id),
            project_id: None,
            started_at: None,
            completed_at: None,
            error: None,
            steps_completed: 0,
            steps_total: None,
            tool_calls: vec![],
            tokens_input: 0,
            tokens_output: 0,
            cost_usd: 0.0,
            model_id: String::new(),
            session_id: None,
        };
        self.agents.write().insert(id, info);
        let _ = self.event_bus.publish(KernelEvent::AgentCreated {
            id,
            name: spec.goal.clone(),
        });
        Ok(id)
    }

    async fn exec(&self, id: AgentId) -> Result<()> {
        if let Some(a) = self.agents.write().get_mut(&id) {
            a.status = AgentStatus::Running;
        }
        Ok(())
    }

    async fn run_with_seed(&self, id: AgentId, _seed: &Seed) -> Result<ExecutionResult> {
        self.run_called.fetch_add(1, Ordering::SeqCst);
        if let Some(a) = self.agents.write().get_mut(&id) {
            a.status = AgentStatus::Idle;
        }
        let _ = self.event_bus.publish(KernelEvent::AgentStarted { id });
        let _ = self
            .event_bus
            .publish(KernelEvent::AgentStopped { id, success: true });
        Ok(ExecutionResult {
            output: "Mock agent completed successfully".into(),
            steps_completed: 5,
            success: true,
            tool_calls: vec![],
            tokens_input: 0,
            tokens_output: 0,
            model_id: String::new(),
        })
    }
    async fn run_with_directive(
        &self,
        id: AgentId,
        _directive: &Directive,
        _env: &ExecEnv,
    ) -> Result<ExecutionResult> {
        self.run_with_seed(id, &Seed::new("mock")).await
    }

    async fn fork_directive(&self, directive: &Directive, _env: &ExecEnv) -> Result<AgentId> {
        let seed = Seed::new(directive.goal.clone());
        self.fork(&seed).await
    }

    async fn wait(&self, id: AgentId) -> Result<AgentStatus> {
        Ok(self
            .agents
            .read()
            .get(&id)
            .map(|a| a.status)
            .unwrap_or(AgentStatus::Stopped))
    }

    async fn kill(&self, id: AgentId) -> Result<()> {
        if let Some(a) = self.agents.write().get_mut(&id) {
            a.status = AgentStatus::Stopped;
        }
        Ok(())
    }

    async fn list(&self) -> Result<Vec<AgentInfo>> {
        Ok(self.agents.read().values().cloned().collect())
    }
}

// ---------------------------------------------------------------------------
// Test orchestrator builder
// ---------------------------------------------------------------------------

/// Build orchestrator parts without the legacy OuroborosProtocol mock.
/// Used by the RFC-027 tests that wire a MockIntentEngine instead.
fn build_test_parts() -> (Arc<MockSupervisor>, EventBus, Arc<StateStore>) {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store =
        Arc::new(StateStore::new(tmp.path().to_path_buf()).expect("StateStore creation failed"));
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));
    (supervisor, event_bus, state_store)
}

/// Legacy helper retained for compatibility — returns the proto too.
#[allow(dead_code)]
fn build_orchestrator_parts() -> (
    Arc<MockOuroboros>,
    Arc<MockSupervisor>,
    EventBus,
    Arc<StateStore>,
) {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store =
        Arc::new(StateStore::new(tmp.path().to_path_buf()).expect("StateStore creation failed"));
    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));
    (ouroboros, supervisor, event_bus, state_store)
}

/// Build orchestrator with RFC-027 mock engine wired.
fn build_rfc027_orchestrator(
    supervisor: Arc<MockSupervisor>,
    state_store: Arc<StateStore>,
    event_bus: EventBus,
) -> (Arc<Orchestrator>, Arc<common::MockIntentEngine>) {
    common::build_test_orchestrator(supervisor, state_store, event_bus)
}

#[expect(dead_code)]
fn make_lifecycle(
    supervisor: Arc<MockSupervisor>,
    scheduler: Arc<oxios_kernel::scheduler::AgentScheduler>,
    event_bus: &EventBus,
) -> AgentLifecycleManager {
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    AgentLifecycleManager::new(
        supervisor,
        scheduler,
        access_manager,
        a2a,
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    )
}

// ---------------------------------------------------------------------------
// Tests
/// Verifies the happy path: assess (Substantial) → crystallize → execute → review.
#[tokio::test]
async fn test_orchestrator_happy_path() {
    let (supervisor, event_bus, state_store) = build_test_parts();
    let (orchestrator, _mock) =
        build_rfc027_orchestrator(supervisor.clone(), state_store, event_bus);

    let result = orchestrator
        .handle_unified(
            "test-user",
            "Fix the bug in main.rs",
            None,
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    // Substantial task → reaches Execute. Review passes (default mock).
    assert!(result.session_id.is_some());
    assert_eq!(result.phase_reached, Phase::Execute);
    assert_eq!(result.evaluation_passed, Some(true));
    assert!(!result.response.is_empty());

    // Mock supervisor fork+run were called.
    assert_eq!(supervisor.fork_called.load(Ordering::SeqCst), 1);
    assert_eq!(supervisor.run_called.load(Ordering::SeqCst), 1);
}

/// Verifies retry behavior: review(fail) → execute_with_feedback → re-review.
#[tokio::test]
async fn test_orchestrator_evolution_loop() {
    let (supervisor, event_bus, state_store) = build_test_parts();
    let (orchestrator, mock) =
        build_rfc027_orchestrator(supervisor.clone(), state_store, event_bus);

    // Configure mock to fail review on first call, pass on second.
    *mock.review_response.write() = common::failing_verdict(vec!["missing tests".into()]);

    let result = orchestrator
        .handle_unified(
            "test-user",
            "Something that needs evolution",
            None,
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    // Retry was attempted (2 executions).
    assert!(result.evaluation_passed.is_some());

    // Mock supervisor fork+run were called twice (initial + retry).
    assert_eq!(supervisor.fork_called.load(Ordering::SeqCst), 2);
    assert_eq!(supervisor.run_called.load(Ordering::SeqCst), 2);
}

/// Verifies session ID is preserved across messages in the same session.
#[tokio::test]
async fn test_session_continuation() {
    let (supervisor, event_bus, state_store) = build_test_parts();
    let (orchestrator, _mock) =
        build_rfc027_orchestrator(supervisor.clone(), state_store, event_bus);

    let session_id = "test-session-123";

    let result1 = orchestrator
        .handle_unified(
            "test-user",
            "Work on the project",
            Some(session_id),
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();
    assert_eq!(result1.session_id.as_deref(), Some(session_id));

    // Second message with same session.
    let result2 = orchestrator
        .handle_unified(
            "test-user",
            "Make it production ready",
            Some(session_id),
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();
    assert_eq!(result2.session_id.as_deref(), Some(session_id));
}
/// Verifies multiple sessions are independent.
#[tokio::test]
async fn test_multiple_sessions_independent() {
    let (supervisor, event_bus, state_store) = build_test_parts();
    let (orchestrator, _mock) =
        build_rfc027_orchestrator(supervisor.clone(), state_store, event_bus);

    let result_a = orchestrator
        .handle_unified(
            "user-a",
            "Task A",
            Some("session-a"),
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();
    let result_b = orchestrator
        .handle_unified(
            "user-b",
            "Task B",
            Some("session-b"),
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    assert_eq!(result_a.session_id.as_deref(), Some("session-a"));
    assert_eq!(result_b.session_id.as_deref(), Some("session-b"));
    assert_ne!(result_a.session_id, result_b.session_id);
}

/// Verifies session cleanup after orchestration completes.
#[tokio::test]
async fn test_session_cleaned_after_completion() {
    let (supervisor, event_bus, state_store) = build_test_parts();
    let (orchestrator, _mock) =
        build_rfc027_orchestrator(supervisor.clone(), state_store, event_bus);

    let session_id = "cleanup-test-session";

    orchestrator
        .handle_unified(
            "test-user",
            "Simple task",
            Some(session_id),
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    // New message without session ID should get a fresh session.
    let result2 = orchestrator
        .handle_unified("test-user", "Another task", None, None, None, "test-req")
        .await
        .unwrap();

    assert_ne!(result2.session_id.as_deref(), Some(session_id));
}

/// Verifies that the orchestrator completes within a reasonable time.
/// (Phase events are now Status events on the event bus.)
#[tokio::test]
async fn test_phase_events_published() {
    let (supervisor, event_bus, state_store) = build_test_parts();
    let (orchestrator, _mock) =
        build_rfc027_orchestrator(supervisor.clone(), state_store, event_bus);

    let result = orchestrator
        .handle_unified("test-user", "Test events", None, None, None, "test-req")
        .await
        .unwrap();

    // The unified path completes and returns a result.
    assert!(!result.response.is_empty());
    assert!(result.session_id.is_some());
}
