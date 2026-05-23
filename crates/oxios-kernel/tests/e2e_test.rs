//! End-to-end smoke test for the Oxios pipeline.
//!
//! Tests the full Ouroboros lifecycle with fully-mocked protocol and supervisor.
//! This avoids needing real LLM API calls — we mock:
//!   - OuroborosProtocol (no LLM calls for interview/seed/evaluate/evolve)
//!   - Supervisor (no actual agent execution)
//!
//! The orchestrator pipeline (interview → seed → execute → evaluate → evolve)
//! is exercised end-to-end to verify the call chain works.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use oxios_kernel::supervisor::Supervisor;
use oxios_kernel::types::{AgentId, AgentInfo, AgentStatus};
use oxios_kernel::{
    A2AProtocol, AccessManager, AgentLifecycleManager, EventBus, KernelEvent, Orchestrator,
};
use oxios_ouroboros::{
    AmbiguityScore, EvaluationResult, ExecutionResult, InterviewResult, OuroborosProtocol, Phase,
    Seed,
};

// ---------------------------------------------------------------------------
// Mock OuroborosProtocol — deterministic, no LLM calls
// ---------------------------------------------------------------------------

/// A mock Ouroboros protocol that returns deterministic results.
/// This exercises the full orchestrator pipeline without any LLM dependency.
struct MockOuroboros {
    interview_called: AtomicUsize,
    generate_seed_called: AtomicUsize,
    evaluate_called: AtomicUsize,
    evolve_called: AtomicUsize,
    /// If false, evaluation fails on first pass (triggers evolve).
    evaluation_passes: AtomicBool,
    /// If true, evolve returns None (no evolution possible).
    no_evolution: AtomicBool,
}

impl MockOuroboros {
    fn new() -> Self {
        Self {
            interview_called: AtomicUsize::new(0),
            generate_seed_called: AtomicUsize::new(0),
            evaluate_called: AtomicUsize::new(0),
            evolve_called: AtomicUsize::new(0),
            evaluation_passes: AtomicBool::new(true),
            no_evolution: AtomicBool::new(false),
        }
    }

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
        Ok(Seed::new("Test task from e2e smoke test"))
    }

    async fn execute(&self, seed: &Seed) -> Result<ExecutionResult> {
        Ok(ExecutionResult {
            output: format!("Executed seed: {}", seed.goal),
            steps_completed: 3,
            success: true,
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
        if self.no_evolution.load(Ordering::SeqCst) {
            return Ok(None);
        }
        Ok(Some(Seed::evolved_from(seed)))
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
            created_at: Utc::now(),
            seed_id: Some(spec.id),
        };
        self.agents.write().insert(id, info);
        let _ = self.event_bus.publish(KernelEvent::AgentCreated {
            id,
            name: spec.goal.clone(),
        });
        Ok(id)
    }

    async fn exec(&self, id: AgentId) -> Result<()> {
        self.agents
            .write()
            .get_mut(&id)
            .map(|a| a.status = AgentStatus::Running);
        Ok(())
    }

    async fn run_with_seed(&self, id: AgentId, _seed: &Seed) -> Result<ExecutionResult> {
        self.run_called.fetch_add(1, Ordering::SeqCst);
        self.agents
            .write()
            .get_mut(&id)
            .map(|a| a.status = AgentStatus::Idle);
        let _ = self.event_bus.publish(KernelEvent::AgentStarted { id });
        let _ = self.event_bus.publish(KernelEvent::AgentStopped { id });
        Ok(ExecutionResult {
            output: "Mock agent completed successfully".into(),
            steps_completed: 5,
            success: true,
        })
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
        self.agents
            .write()
            .get_mut(&id)
            .map(|a| a.status = AgentStatus::Stopped);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<AgentInfo>> {
        Ok(self.agents.read().values().cloned().collect())
    }
}

// ---------------------------------------------------------------------------
// Test orchestrator builder
// ---------------------------------------------------------------------------

async fn build_test_orchestrator() -> Result<Orchestrator> {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir()?;
    let state_store = Arc::new(
        oxios_kernel::StateStore::new(tmp.path().join("state"))
            .map_err(|e| anyhow::anyhow!("StateStore: {}", e))?,
    );

    let ouroboros: Arc<dyn OuroborosProtocol> = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let scheduler = Arc::new(oxios_kernel::AgentScheduler::new(4, 60, 300));
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));

    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        scheduler,
        access_manager,
        a2a,
        event_bus.clone(),
        300, // max_execution_time_secs
    );

    let orchestrator = Orchestrator::new(ouroboros, event_bus, state_store, lifecycle);

    Ok(orchestrator)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verifies the orchestrator handles a message end-to-end.
/// The mock protocol returns low ambiguity → should reach Evaluate phase.
#[tokio::test]
async fn test_orchestrator_handles_message() -> Result<()> {
    let orchestrator = build_test_orchestrator().await?;

    let result = orchestrator
        .handle_message(
            "test-user",
            "Fix the bug in main.rs that causes crashes",
            None,
        )
        .await?;

    // Response should not be empty.
    assert!(!result.response.is_empty(), "Response should not be empty");

    // Should reach at least Execute phase (current pipeline: Interview → Seed → Execute).
    assert!(
        result.phase_reached == Phase::Execute,
        "Expected Execute phase, got {:?}",
        result.phase_reached
    );

    // Should have a seed.
    assert!(result.seed_id.is_some(), "Seed ID should be set");

    // Evaluation should pass (mock passes on first try).
    assert!(result.evaluation_passed, "Evaluation should pass");

    println!("Phase reached: {:?}", result.phase_reached);
    println!("Response: {}", result.response);

    Ok(())
}

/// Verifies the orchestrator exercises the evolution loop when evaluation fails.
#[tokio::test]
async fn test_orchestrator_evolution_loop() -> Result<()> {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir()?;
    let state_store = Arc::new(
        oxios_kernel::StateStore::new(tmp.path().join("state"))
            .map_err(|e| anyhow::anyhow!("StateStore: {}", e))?,
    );

    // Mock that fails evaluation first time → triggers evolve.
    let ouroboros: Arc<dyn OuroborosProtocol> = Arc::new(MockOuroboros::with_failing_evaluation());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));
    let scheduler = Arc::new(oxios_kernel::AgentScheduler::new(4, 60, 300));
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));

    let lifecycle = AgentLifecycleManager::new(
        supervisor.clone(),
        scheduler,
        access_manager,
        a2a,
        event_bus.clone(),
        300, // max_execution_time_secs
    );

    let orchestrator = Orchestrator::new(ouroboros, event_bus, state_store, lifecycle);

    let result = orchestrator
        .handle_message("test-user", "Something that needs evolution", None)
        .await?;

    // Should still reach Execute (evolve→re-execute).
    // Note: current pipeline stops at Execute; evaluate is not a separate phase.
    assert_eq!(result.phase_reached, Phase::Execute);
    assert!(
        result.evaluation_passed,
        "Execution should succeed after evolution"
    );

    println!("Evolution loop: {:?}", result.phase_reached);

    Ok(())
}

/// Verifies session ID is preserved across messages in the same session.
#[tokio::test]
async fn test_session_continuation() -> Result<()> {
    let orchestrator = build_test_orchestrator().await?;
    let session_id = "test-session-123";

    let result1 = orchestrator
        .handle_message("test-user", "Work on the project", Some(session_id))
        .await?;

    assert_eq!(
        result1.session_id.as_deref(),
        Some(session_id),
        "Session ID should be preserved"
    );

    let result2 = orchestrator
        .handle_message("test-user", "Make it production ready", Some(session_id))
        .await?;

    assert_eq!(
        result2.session_id.as_deref(),
        Some(session_id),
        "Session ID should be preserved in follow-up"
    );

    println!(
        "Session continuation: {:?} → {:?}",
        result1.phase_reached, result2.phase_reached
    );

    Ok(())
}

/// Verifies multiple sessions are independent.
#[tokio::test]
async fn test_multiple_sessions_independent() -> Result<()> {
    let orchestrator = build_test_orchestrator().await?;

    let result_a = orchestrator
        .handle_message("user-a", "Task A", Some("session-a"))
        .await?;

    let result_b = orchestrator
        .handle_message("user-b", "Task B", Some("session-b"))
        .await?;

    assert_eq!(result_a.session_id.as_deref(), Some("session-a"));
    assert_eq!(result_b.session_id.as_deref(), Some("session-b"));
    assert_ne!(result_a.session_id, result_b.session_id);

    println!(
        "Session A: phase={:?}, Session B: phase={:?}",
        result_a.phase_reached, result_b.phase_reached
    );

    Ok(())
}

/// Verifies session cleanup after orchestration completes.
#[tokio::test]
async fn test_session_cleaned_after_completion() -> Result<()> {
    let orchestrator = build_test_orchestrator().await?;
    let session_id = "cleanup-test-session";

    orchestrator
        .handle_message("test-user", "Simple task", Some(session_id))
        .await?;

    // New message without session ID should get a fresh session.
    let result2 = orchestrator
        .handle_message("test-user", "Another task", None)
        .await?;

    assert_ne!(
        result2.session_id.as_deref(),
        Some(session_id),
        "New message should get a fresh session ID"
    );

    Ok(())
}

/// Verifies event bus publishes phase events during orchestration.
#[tokio::test]
async fn test_phase_events_published() -> Result<()> {
    let event_bus = EventBus::new(64);
    let mut rx = event_bus.subscribe();
    let tmp = tempfile::tempdir()?;
    let state_store = Arc::new(
        oxios_kernel::StateStore::new(tmp.path().join("state"))
            .map_err(|e| anyhow::anyhow!("StateStore: {}", e))?,
    );

    let ouroboros: Arc<dyn OuroborosProtocol> = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));
    let scheduler = Arc::new(oxios_kernel::AgentScheduler::new(4, 60, 300));
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));

    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        scheduler,
        access_manager,
        a2a,
        event_bus.clone(),
        300, // max_execution_time_secs
    );

    let orchestrator = Orchestrator::new(ouroboros, event_bus.clone(), state_store, lifecycle);

    // Run orchestration in background.
    let handle = tokio::spawn(async move {
        orchestrator
            .handle_message("test-user", "Test events", None)
            .await
            .unwrap()
    });

    // Collect phase events.
    let mut phase_started = 0;
    let mut phase_completed = 0;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        let elapsed = deadline.saturating_duration_since(tokio::time::Instant::now());
        if elapsed.is_zero() {
            break;
        }

        let evt = tokio::select! {
            evt = rx.recv() => evt?,
            _ = tokio::time::sleep(elapsed) => break,
        };

        match evt {
            KernelEvent::PhaseStarted { .. } => phase_started += 1,
            KernelEvent::PhaseCompleted { .. } => phase_completed += 1,
            _ => {}
        }
    }

    // Should have at least 3 phase started events (Interview, Seed, Execute).
    assert!(
        phase_started >= 3,
        "Expected ≥3 PhaseStarted events, got {}",
        phase_started
    );
    assert!(
        phase_completed >= 3,
        "Expected ≥3 PhaseCompleted events, got {}",
        phase_completed
    );

    let _ = handle.await?;

    println!(
        "Phase events: started={}, completed={}",
        phase_started, phase_completed
    );

    Ok(())
}
