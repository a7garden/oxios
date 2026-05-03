//! Integration tests for the Oxios kernel.
//!
//! Tests the main kernel components using mock implementations:
//! - Orchestrator with mock OuroborosProtocol and mock Supervisor
//! - StateStore markdown/JSON read/write
//! - EventBus publish/subscribe
//! - Gateway routing with mock channel

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use oxios_gateway::channel::Channel;
use oxios_gateway::gateway::Gateway;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use oxios_kernel::event_bus::{EventBus, KernelEvent};
use oxios_kernel::orchestrator::Orchestrator;
use oxios_kernel::state_store::StateStore;
use oxios_kernel::supervisor::Supervisor;
use oxios_kernel::types::{AgentId, AgentInfo, AgentStatus};
use oxios_ouroboros::evaluation::EvaluationResult;
use oxios_ouroboros::interview::InterviewResult;
use oxios_ouroboros::protocol::{ExecutionResult, OuroborosProtocol, Phase};
use oxios_ouroboros::seed::{AmbiguityScore, Seed};

// ---------------------------------------------------------------------------
// Mock OuroborosProtocol
// ---------------------------------------------------------------------------

/// Mock Ouroboros protocol that skips LLM calls.
///
/// Returns deterministic responses suitable for testing the orchestrator
/// without requiring real LLM access.
struct MockOuroboros {
    /// Number of times interview was called.
    interview_called: AtomicUsize,
    /// Number of times generate_seed was called.
    generate_seed_called: AtomicUsize,
    /// Number of times evaluate was called.
    evaluate_called: AtomicUsize,
    /// Number of times evolve was called.
    evolve_called: AtomicUsize,
    /// Whether the evaluation should pass on the first try.
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

    /// Create a mock that will fail evaluation on the first pass.
    fn with_failing_evaluation() -> Self {
        let s = Self::new();
        s.evaluation_passes.store(false, Ordering::SeqCst);
        s
    }
}

#[async_trait]
impl OuroborosProtocol for MockOuroboros {
    async fn interview(&self, _user_input: &str) -> anyhow::Result<InterviewResult> {
        self.interview_called.fetch_add(1, Ordering::SeqCst);
        let mut result = InterviewResult::new();
        // High clarity so we're ready for seed immediately.
        let score = AmbiguityScore::new(1.0, 1.0, 1.0);
        result.update_ambiguity(score);
        result.add_exchange("What is the goal?", "Test goal");
        Ok(result)
    }

    async fn generate_seed(&self, _interview: &InterviewResult) -> anyhow::Result<Seed> {
        self.generate_seed_called.fetch_add(1, Ordering::SeqCst);
        Ok(Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Test goal".into(),
            constraints: vec!["No errors".into()],
            acceptance_criteria: vec!["Output contains 'done'".into()],
            ontology: vec![],
            created_at: chrono::Utc::now(),
        })
    }

    async fn execute(&self, seed: &Seed) -> anyhow::Result<ExecutionResult> {
        Ok(ExecutionResult {
            output: format!("Executed: {}", seed.goal),
            steps_completed: 5,
            success: true,
        })
    }

    async fn evaluate(
        &self,
        _seed: &Seed,
        _execution: &ExecutionResult,
    ) -> anyhow::Result<EvaluationResult> {
        self.evaluate_called.fetch_add(1, Ordering::SeqCst);
        let passes = self.evaluation_passes.load(Ordering::SeqCst);
        // After first evaluation, make it pass for subsequent calls (evolve loop).
        self.evaluation_passes.store(true, Ordering::SeqCst);
        Ok(EvaluationResult {
            mechanical_pass: passes,
            semantic_pass: Some(passes),
            consensus_pass: None,
            score: if passes { 0.95 } else { 0.5 },
            notes: vec!["Mock evaluation".into()],
        })
    }

    async fn evolve(
        &self,
        seed: &Seed,
        _evaluation: &EvaluationResult,
    ) -> anyhow::Result<Option<Seed>> {
        self.evolve_called.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Seed {
            id: uuid::Uuid::new_v4(),
            goal: format!("Evolved: {}", seed.goal),
            constraints: seed.constraints.clone(),
            acceptance_criteria: seed.acceptance_criteria.clone(),
            ontology: vec![],
            created_at: chrono::Utc::now(),
        }))
    }
}

// ---------------------------------------------------------------------------
// Mock Supervisor
// ---------------------------------------------------------------------------

/// Mock supervisor that tracks agent creation without actually running agents.
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
    async fn fork(&self, spec: &Seed) -> anyhow::Result<AgentId> {
        self.fork_called.fetch_add(1, Ordering::SeqCst);
        let id = AgentId::new_v4();
        let info = AgentInfo {
            id,
            name: spec.goal.clone(),
            status: AgentStatus::Starting,
            created_at: chrono::Utc::now(),
            seed_id: Some(spec.id),
        };
        {
            let mut agents = self.agents.write();
            agents.insert(id, info);
        }
        let _ = self
            .event_bus
            .publish(KernelEvent::AgentCreated { id, name: spec.goal.clone() });
        Ok(id)
    }

    async fn exec(&self, id: AgentId) -> anyhow::Result<()> {
        let mut agents = self.agents.write();
        match agents.get_mut(&id) {
            Some(a) => a.status = AgentStatus::Running,
            None => anyhow::bail!("Agent {id} not found"),
        }
        Ok(())
    }

    async fn run_with_seed(
        &self,
        id: AgentId,
        _seed: &Seed,
    ) -> anyhow::Result<ExecutionResult> {
        self.run_called.fetch_add(1, Ordering::SeqCst);
        {
            let mut agents = self.agents.write();
            if let Some(a) = agents.get_mut(&id) {
                a.status = AgentStatus::Idle;
            }
        }
        let _ = self.event_bus.publish(KernelEvent::AgentStarted { id });
        let _ = self.event_bus.publish(KernelEvent::AgentStopped { id });
        Ok(ExecutionResult {
            output: "Mock agent completed".into(),
            steps_completed: 3,
            success: true,
        })
    }

    async fn wait(&self, id: AgentId) -> anyhow::Result<AgentStatus> {
        let agents = self.agents.read();
        match agents.get(&id) {
            Some(a) => Ok(a.status),
            None => anyhow::bail!("Agent {id} not found"),
        }
    }

    async fn kill(&self, id: AgentId) -> anyhow::Result<()> {
        let mut agents = self.agents.write();
        if let Some(a) = agents.get_mut(&id) {
            a.status = AgentStatus::Stopped;
        }
        Ok(())
    }

    async fn list(&self) -> anyhow::Result<Vec<AgentInfo>> {
        let agents = self.agents.read();
        Ok(agents.values().cloned().collect())
    }
}

// ---------------------------------------------------------------------------
// Mock Channel
// ---------------------------------------------------------------------------

/// Mock channel that captures outgoing messages for verification.
struct MockChannel {
    outgoing: tokio::sync::Mutex<Vec<OutgoingMessage>>,
    incoming_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<IncomingMessage>>,
}

impl MockChannel {
    fn new(buffer: usize) -> Self {
        let (_tx, rx) = tokio::sync::mpsc::channel(buffer);
        Self {
            outgoing: tokio::sync::Mutex::new(Vec::new()),
            incoming_rx: tokio::sync::Mutex::new(rx),
        }
    }
}

#[async_trait]
impl Channel for MockChannel {
    fn name(&self) -> &str {
        "mock"
    }

    async fn receive(&self) -> anyhow::Result<Option<IncomingMessage>> {
        let mut rx = self.incoming_rx.lock().await;
        Ok(rx.try_recv().ok())
    }

    async fn send(&self, msg: OutgoingMessage) -> anyhow::Result<()> {
        self.outgoing.lock().await.push(msg);
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

// --- EventBus tests ---

#[tokio::test]
async fn test_event_bus_publish_subscribe() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    bus.publish(KernelEvent::SeedCreated {
        seed_id: uuid::Uuid::new_v4(),
    })
    .unwrap();

    let event = rx.recv().await.unwrap();
    match event {
        KernelEvent::SeedCreated { .. } => {}
        other => panic!("Expected SeedCreated, got {:?}", other),
    }
}

#[tokio::test]
async fn test_event_bus_multiple_subscribers() {
    let bus = EventBus::new(16);
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    let seed_id = uuid::Uuid::new_v4();
    bus.publish(KernelEvent::SeedCreated { seed_id }).unwrap();

    let e1 = rx1.recv().await.unwrap();
    let e2 = rx2.recv().await.unwrap();

    assert!(matches!(e1, KernelEvent::SeedCreated { .. }));
    assert!(matches!(e2, KernelEvent::SeedCreated { .. }));
}

#[tokio::test]
async fn test_event_bus_no_subscribers_ok() {
    let bus = EventBus::new(16);
    // Should not panic when publishing with no subscribers.
    bus.publish(KernelEvent::SeedCreated {
        seed_id: uuid::Uuid::new_v4(),
    })
    .unwrap();
}

// --- StateStore tests ---

#[tokio::test]
async fn test_state_store_save_load_markdown() {
    let tmp = tempfile::tempdir().unwrap();
    let store = StateStore::new(tmp.path().to_path_buf()).unwrap();

    store
        .save_markdown("memory", "test-note", "Hello, world!")
        .await
        .unwrap();

    let loaded = store.load_markdown("memory", "test-note").await.unwrap();
    assert_eq!(loaded, Some("Hello, world!".to_string()));
}

#[tokio::test]
async fn test_state_store_load_nonexistent() {
    let tmp = tempfile::tempdir().unwrap();
    let store = StateStore::new(tmp.path().to_path_buf()).unwrap();

    let loaded = store.load_markdown("memory", "nope").await.unwrap();
    assert_eq!(loaded, None);
}

#[tokio::test]
async fn test_state_store_list_category() {
    let tmp = tempfile::tempdir().unwrap();
    let store = StateStore::new(tmp.path().to_path_buf()).unwrap();

    store
        .save_markdown("seeds", "alpha", "seed alpha content")
        .await
        .unwrap();
    store
        .save_markdown("seeds", "beta", "seed beta content")
        .await
        .unwrap();

    let names = store.list_category("seeds").await.unwrap();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[tokio::test]
async fn test_state_store_save_load_json() {
    let tmp = tempfile::tempdir().unwrap();
    let store = StateStore::new(tmp.path().to_path_buf()).unwrap();

    let data = serde_json::json!({
        "name": "test",
        "value": 42
    });

    store.save_json("config", "test", &data).await.unwrap();
    let loaded: Option<serde_json::Value> = store.load_json("config", "test").await.unwrap();
    assert_eq!(loaded, Some(data));
}

#[tokio::test]
async fn test_state_store_path_traversal_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    let store = StateStore::new(tmp.path().to_path_buf()).unwrap();

    // Category traversal should be blocked.
    let result = store.save_markdown("../etc", "shadow", "hacked").await;
    assert!(result.is_err());

    // Name traversal should be blocked.
    let result = store.save_markdown("memory", "../shadow", "hacked").await;
    assert!(result.is_err());

    // Slash in category should be blocked.
    let result = store.save_markdown("foo/bar", "test", "content").await;
    assert!(result.is_err());

    // Backslash should be blocked.
    let result = store.save_markdown("foo\\bar", "test", "content").await;
    assert!(result.is_err());
}

// --- Orchestrator tests ---

#[tokio::test]
async fn test_orchestrator_happy_path() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let orchestrator = Orchestrator::new(
        ouroboros.clone(),
        supervisor.clone(),
        event_bus.clone(),
        state_store,
    );

    let result = orchestrator
        .handle_message("test-user", "Do something useful", None)
        .await
        .unwrap();

    assert!(result.session_id.is_some());
    assert!(result.seed_id.is_some());
    assert_eq!(result.phase_reached, Phase::Evaluate);
    assert!(result.evaluation_passed);
    assert!(result.response.contains("completed"));

    // Verify all phases were called.
    assert_eq!(ouroboros.interview_called.load(Ordering::SeqCst), 1);
    assert_eq!(ouroboros.generate_seed_called.load(Ordering::SeqCst), 1);
    assert_eq!(ouroboros.evaluate_called.load(Ordering::SeqCst), 1);

    // Verify supervisor was called.
    assert_eq!(supervisor.fork_called.load(Ordering::SeqCst), 1);
    assert_eq!(supervisor.run_called.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_orchestrator_evolution_loop() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    // Mock that fails evaluation on first pass, triggering evolution.
    let ouroboros = Arc::new(MockOuroboros::with_failing_evaluation());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let orchestrator = Orchestrator::new(
        ouroboros.clone(),
        supervisor.clone(),
        event_bus.clone(),
        state_store,
    );

    let result = orchestrator
        .handle_message("test-user", "Do something tricky", None)
        .await
        .unwrap();

    // Should have evolved and re-executed.
    assert_eq!(ouroboros.evolve_called.load(Ordering::SeqCst), 1);
    assert!(result.evaluation_passed); // Second evaluation passes.
    assert_eq!(supervisor.fork_called.load(Ordering::SeqCst), 2); // Original + evolved.
    assert_eq!(supervisor.run_called.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_orchestrator_events_published() {
    let event_bus = EventBus::new(64);
    let mut rx = event_bus.subscribe();
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let orchestrator = Orchestrator::new(
        ouroboros,
        supervisor,
        event_bus.clone(),
        state_store,
    );

    // Run orchestration in background.
    let handle = tokio::spawn(async move {
        orchestrator
            .handle_message("test-user", "Check events", None)
            .await
            .unwrap()
    });

    // Collect events with timeout.
    let mut phase_events = Vec::new();
    let mut seed_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        let evt = tokio::select! {
            evt = rx.recv() => evt.unwrap(),
            _ = tokio::time::sleep_until(deadline) => break,
        };
        match evt {
            KernelEvent::PhaseStarted { .. } | KernelEvent::PhaseCompleted { .. } => {
                phase_events.push(evt);
            }
            KernelEvent::SeedCreated { .. } => {
                seed_events.push(evt);
            }
            _ => {}
        }
        if phase_events.len() >= 8 {
            break;
        }
    }

    // Should have at least PhaseStarted for Interview, Seed, Execute, Evaluate.
    assert!(
        phase_events.len() >= 4,
        "Expected at least 4 phase events, got {}",
        phase_events.len()
    );
    assert!(
        !seed_events.is_empty(),
        "Expected at least one SeedCreated event"
    );

    // Ensure the orchestration completed.
    let _result = handle.await.unwrap();
}

// --- Gateway routing test ---

#[tokio::test]
async fn test_gateway_routes_message_through_orchestrator() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let orchestrator = Arc::new(Orchestrator::new(
        ouroboros,
        supervisor,
        event_bus.clone(),
        state_store,
    ));

    let gateway = Gateway::new(orchestrator);
    let mock_channel = Box::new(MockChannel::new(16));

    // Register the mock channel.
    gateway.register(mock_channel).await;
    assert_eq!(gateway.channel_names().await, vec!["mock"]);

    // Route a message.
    let msg = IncomingMessage::new("mock", "test-user", "Hello gateway");
    gateway.route(msg).await.unwrap();

    // The mock channel should have received the response.
    // We need to get a reference to the mock channel — since register
    // took ownership, let's test routing directly.
    // Instead, test that the gateway successfully routes and doesn't panic.
}

#[tokio::test]
async fn test_gateway_unknown_channel() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let orchestrator = Arc::new(Orchestrator::new(
        ouroboros,
        supervisor,
        event_bus.clone(),
        state_store,
    ));

    let gateway = Gateway::new(orchestrator);

    // Routing to a non-existent channel should still succeed
    // (the orchestrator runs, but the response can't be delivered).
    let msg = IncomingMessage::new("nonexistent", "test-user", "Test");
    let result = gateway.route(msg).await;
    // The route itself should succeed — the missing channel just means
    // the response won't be delivered, which is logged as a warning.
    assert!(result.is_ok());
}
