//! Integration tests for the Oxios kernel.
//!
//! Tests the main kernel components using mock implementations:
//! - Orchestrator with mock OuroborosProtocol and mock Supervisor
//! - StateStore markdown/JSON read/write
//! - EventBus publish/subscribe
//! - Gateway routing with mock channel

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;

use oxios_gateway::channel::Channel;
use oxios_gateway::gateway::Gateway;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use oxios_kernel::a2a::A2AProtocol;
use oxios_kernel::access_manager::{AccessManager, AgentPermissions};
use oxios_kernel::agent_lifecycle::AgentLifecycleManager;
use oxios_kernel::config::OrchestratorConfig;
use oxios_kernel::event_bus::{EventBus, KernelEvent};
use oxios_kernel::orchestrator::Orchestrator;
use oxios_kernel::scheduler::AgentScheduler;
use oxios_kernel::state_store::StateStore;
use oxios_kernel::supervisor::Supervisor;
use oxios_ouroboros::evaluation::EvaluationResult;
use oxios_ouroboros::interview::InterviewResult;
use oxios_ouroboros::protocol::{ExecutionResult, OuroborosProtocol, Phase};
use oxios_ouroboros::seed::{AmbiguityScore, Seed};

use oxios_kernel::types::{AgentId, AgentInfo, AgentStatus};
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

    async fn interview_structured(
        &self,
        _user_input: &str,
    ) -> anyhow::Result<Option<Vec<oxios_ouroboros::ouroboros_engine::InterviewQuestionOutput>>>
    {
        Ok(None)
    }

    async fn generate_seed(&self, _interview: &InterviewResult) -> anyhow::Result<Seed> {
        self.generate_seed_called.fetch_add(1, Ordering::SeqCst);
        let mut seed = Seed::new("Test goal");
        seed.constraints = vec!["No errors".into()];
        seed.acceptance_criteria = vec!["Output contains 'done'".into()];
        Ok(seed)
    }

    async fn execute(&self, seed: &Seed) -> anyhow::Result<ExecutionResult> {
        Ok(ExecutionResult {
            output: format!("Executed: {}", seed.goal),
            steps_completed: 5,
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
        let evolved = Seed::evolved_from(seed);
        Ok(Some(evolved))
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
        {
            let mut agents = self.agents.write();
            agents.insert(id, info);
        }
        let _ = self.event_bus.publish(KernelEvent::AgentCreated {
            id,
            name: spec.goal.clone(),
        });
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

    async fn run_with_seed(&self, id: AgentId, _seed: &Seed) -> anyhow::Result<ExecutionResult> {
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
            tool_calls: vec![],
            tokens_input: 0,
            tokens_output: 0,
            model_id: String::new(),
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
    incoming_rx: tokio::sync::Mutex<Option<tokio::sync::mpsc::Receiver<IncomingMessage>>>,
}

impl MockChannel {
    fn new(buffer: usize) -> Self {
        let (_tx, rx) = tokio::sync::mpsc::channel(buffer);
        Self {
            outgoing: tokio::sync::Mutex::new(Vec::new()),
            incoming_rx: tokio::sync::Mutex::new(Some(rx)),
        }
    }
}

#[async_trait]
impl Channel for MockChannel {
    fn name(&self) -> &str {
        "mock"
    }

    async fn start(
        &self,
        _tx: tokio::sync::mpsc::Sender<oxios_gateway::GatewayInbox>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        // Take the receiver so it can't be started twice.
        self.incoming_rx.lock().await.take();
        let handle = tokio::spawn(async move {
            // Just wait for shutdown.
            let _ = shutdown.changed().await;
        });
        Ok(handle)
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
        other => panic!("Expected SeedCreated, got {other:?}"),
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

    // Slash in category is allowed (sub-directory categories like memory/knowledge).
    let result = store.save_markdown("foo/bar", "test", "content").await;
    assert!(result.is_ok());

    // Backslash should be blocked.
    let result = store.save_markdown("foo\\bar", "test", "content").await;
    assert!(result.is_err());

    // Empty category should be blocked.
    let result = store.save_markdown("", "test", "content").await;
    assert!(result.is_err());

    // Leading slash should be blocked.
    let result = store.save_markdown("/foo", "test", "content").await;
    assert!(result.is_err());

    // Trailing slash should be blocked.
    let result = store.save_markdown("foo/", "test", "content").await;
    assert!(result.is_err());

    // Double slash should be blocked.
    let result = store.save_markdown("foo//bar", "test", "content").await;
    assert!(result.is_err());
}

fn make_evolution_config(max_iterations: u32) -> OrchestratorConfig {
    OrchestratorConfig {
        max_evolution_iterations: max_iterations,
        min_evaluation_score: 0.8,
        eval_cache_enabled: true,
        spec_keywords: vec!["#spec".into(), "#plan".into()],
        default_mode: "spec".into(),
    }
}

// --- Orchestrator tests ---

#[tokio::test]
async fn test_orchestrator_happy_path() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::default());
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let lifecycle = AgentLifecycleManager::new(
        supervisor.clone(),
        scheduler.clone(),
        access_manager.clone(),
        a2a.clone(),
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    );
    let orchestrator = Orchestrator::with_config(
        ouroboros.clone(),
        event_bus.clone(),
        state_store,
        lifecycle,
        make_evolution_config(0), // evaluate only, no evolve loop
    );

    let result = orchestrator
        .handle_message(
            "test-user",
            "Do something useful",
            None,
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    assert!(result.session_id.is_some());
    assert!(result.seed_id.is_some());
    // With acceptance_criteria → should_evaluate=true → reaches Evaluate.
    assert_eq!(result.phase_reached, Phase::Evaluate);
    assert_eq!(result.evaluation_passed, Some(true));
    assert!(!result.response.is_empty());

    // Verify phases called: Interview, Seed, Execute, Evaluate.
    assert_eq!(ouroboros.interview_called.load(Ordering::SeqCst), 1);
    assert_eq!(ouroboros.generate_seed_called.load(Ordering::SeqCst), 1);
    assert_eq!(ouroboros.evaluate_called.load(Ordering::SeqCst), 1); // evaluate IS called
    assert_eq!(ouroboros.evolve_called.load(Ordering::SeqCst), 0); // evolve NOT called

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

    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::default());
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let lifecycle = AgentLifecycleManager::new(
        supervisor.clone(),
        scheduler.clone(),
        access_manager.clone(),
        a2a.clone(),
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    );
    let orchestrator = Orchestrator::with_config(
        ouroboros.clone(),
        event_bus.clone(),
        state_store,
        lifecycle,
        make_evolution_config(3), // evolve loop enabled
    );

    let result = orchestrator
        .handle_message(
            "test-user",
            "Do something tricky",
            None,
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    // Evaluation fails first → evolve → re-execute → evaluate(pass).
    assert_eq!(result.phase_reached, Phase::Evolve);
    assert_eq!(result.evaluation_passed, Some(true));

    // Verify the evolution loop was exercised.
    assert_eq!(ouroboros.evaluate_called.load(Ordering::SeqCst), 2); // fail + pass
    assert_eq!(ouroboros.evolve_called.load(Ordering::SeqCst), 1);
    assert_eq!(supervisor.fork_called.load(Ordering::SeqCst), 2); // initial + re-exec
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

    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::default());
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        scheduler.clone(),
        access_manager.clone(),
        a2a.clone(),
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    );
    let orchestrator = Orchestrator::with_config(
        ouroboros,
        event_bus.clone(),
        state_store,
        lifecycle,
        make_evolution_config(0),
    );

    // Run orchestration in background.
    let handle = tokio::spawn(async move {
        orchestrator
            .handle_message("test-user", "Check events", None, None, None, "test-req")
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

    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::default());
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let orchestrator = Arc::new({
        let lifecycle = AgentLifecycleManager::new(
            supervisor,
            scheduler.clone(),
            access_manager.clone(),
            a2a.clone(),
            event_bus.clone(),
            300,
            vec![],
            true,
            "/tmp/oxios-test-workspace".to_string(),
        );
        Orchestrator::with_config(
            ouroboros,
            event_bus.clone(),
            state_store,
            lifecycle,
            make_evolution_config(0),
        )
    });

    let gateway = Gateway::new(orchestrator);
    let mock_channel = Box::new(MockChannel::new(16));

    // Register the mock channel (start() will be called internally).
    gateway.register(mock_channel).await.unwrap();
    assert_eq!(gateway.channel_names().await, vec!["mock"]);

    // Test that channel registration works and the gateway can send_to.
    let outgoing = OutgoingMessage::new("mock", "test-user", "Hello from gateway");
    let result = gateway.send_to("mock", outgoing).await;
    assert!(result.is_ok());

    // Clean shutdown.
    gateway.signal_shutdown();
}

#[tokio::test]
async fn test_gateway_unknown_channel() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(MockSupervisor::new(event_bus.clone()));

    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::default());
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let orchestrator = Arc::new({
        let lifecycle = AgentLifecycleManager::new(
            supervisor,
            scheduler.clone(),
            access_manager.clone(),
            a2a.clone(),
            event_bus.clone(),
            300,
            vec![],
            true,
            "/tmp/oxios-test-workspace".to_string(),
        );
        Orchestrator::with_config(
            ouroboros,
            event_bus.clone(),
            state_store,
            lifecycle,
            make_evolution_config(0),
        )
    });

    let gateway = Gateway::new(orchestrator);

    // Sending to a non-existent channel should succeed (logged as warning).
    let outgoing = OutgoingMessage::new("nonexistent", "test-user", "Test");
    let result = gateway.send_to("nonexistent", outgoing).await;
    // send_to succeeds even if channel doesn't exist — just logs a warning.
    assert!(result.is_ok());
}

// ===========================================================================
// Scheduler + Orchestrator Integration
// ===========================================================================

use oxios_kernel::scheduler::{Priority, ScheduledTask};
use std::sync::atomic::AtomicU32;

/// Mock Supervisor that integrates with the scheduler.
struct SchedulerAwareSupervisor {
    scheduler: Arc<AgentScheduler>,
    agents: parking_lot::RwLock<HashMap<AgentId, AgentInfo>>,
    event_bus: EventBus,
    tasks_claimed: AtomicU32,
}

impl SchedulerAwareSupervisor {
    fn new(scheduler: Arc<AgentScheduler>, event_bus: EventBus) -> Self {
        Self {
            scheduler,
            agents: parking_lot::RwLock::new(HashMap::new()),
            event_bus,
            tasks_claimed: AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl Supervisor for SchedulerAwareSupervisor {
    async fn fork(&self, spec: &Seed) -> anyhow::Result<AgentId> {
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
        {
            let mut agents = self.agents.write();
            agents.insert(id, info);
        }
        let _ = self.event_bus.publish(KernelEvent::AgentCreated {
            id,
            name: spec.goal.clone(),
        });
        Ok(id)
    }

    async fn exec(&self, id: AgentId) -> anyhow::Result<()> {
        let mut agents = self.agents.write();
        if let Some(a) = agents.get_mut(&id) {
            a.status = AgentStatus::Running;
        }
        Ok(())
    }

    async fn run_with_seed(&self, id: AgentId, _seed: &Seed) -> anyhow::Result<ExecutionResult> {
        // Submit to the scheduler before running.
        let task = ScheduledTask::for_agent(id, format!("Agent {id} execution"), Priority::Normal);
        self.scheduler.submit(task)?;

        self.tasks_claimed.fetch_add(1, Ordering::SeqCst);

        {
            let mut agents = self.agents.write();
            if let Some(a) = agents.get_mut(&id) {
                a.status = AgentStatus::Idle;
            }
        }
        let _ = self.event_bus.publish(KernelEvent::AgentStarted { id });
        let _ = self.event_bus.publish(KernelEvent::AgentStopped { id });
        Ok(ExecutionResult {
            output: "Task completed".into(),
            steps_completed: 1,
            success: true,
            tool_calls: vec![],
            tokens_input: 0,
            tokens_output: 0,
            model_id: String::new(),
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

#[tokio::test]
async fn test_scheduler_orchestrator_integration() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    // Create a scheduler with a reasonable concurrent limit.
    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::new(3, 100, 60));
    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(SchedulerAwareSupervisor::new(
        scheduler.clone(),
        event_bus.clone(),
    ));
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        scheduler.clone(),
        access_manager.clone(),
        a2a.clone(),
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    );
    let orchestrator = Orchestrator::with_config(
        ouroboros,
        event_bus.clone(),
        state_store,
        lifecycle,
        make_evolution_config(0),
    );

    // Run a single orchestration.
    let result = orchestrator
        .handle_message(
            "test-user",
            "Build a simple thing",
            None,
            None,
            None,
            "test-req",
        )
        .await
        .unwrap();

    assert!(result.session_id.is_some());
    assert!(result.seed_id.is_some());
    // With acceptance_criteria → should_evaluate=true, max_iterations=0 → evaluate only.
    assert_eq!(result.phase_reached, Phase::Evaluate);

    // Scheduler stats - tasks were submitted by the supervisor.
    // They may still be queued if next_task was never called, or running if called.
    let stats = scheduler.stats();
    // Tasks were submitted, so at least some may exist in queue or running.
    assert!(
        stats.queued + stats.running >= 1,
        "Expected at least one task"
    );
}

#[tokio::test]
async fn test_scheduler_priority_ordering_in_orchestration() {
    let event_bus = EventBus::new(64);
    let tmp = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmp.path().to_path_buf()).unwrap());

    let a2a = Arc::new(A2AProtocol::new(event_bus.clone()));
    let scheduler = Arc::new(AgentScheduler::new(10, 10_000, 60));

    // Submit tasks of varying priorities.
    scheduler
        .submit(ScheduledTask::new(
            "Low priority task".into(),
            Priority::Low,
        ))
        .unwrap();
    scheduler
        .submit(ScheduledTask::new(
            "High priority task".into(),
            Priority::High,
        ))
        .unwrap();
    scheduler
        .submit(ScheduledTask::new(
            "Normal priority task".into(),
            Priority::Normal,
        ))
        .unwrap();
    scheduler
        .submit(ScheduledTask::new(
            "Critical task".into(),
            Priority::Critical,
        ))
        .unwrap();

    // Drain in priority order.
    let task1 = scheduler.next_task().unwrap();
    assert_eq!(task1.priority, Priority::Critical);

    let task2 = scheduler.next_task().unwrap();
    assert_eq!(task2.priority, Priority::High);

    let task3 = scheduler.next_task().unwrap();
    assert_eq!(task3.priority, Priority::Normal);

    let task4 = scheduler.next_task().unwrap();
    assert_eq!(task4.priority, Priority::Low);

    // Verify the scheduler and orchestrator can coexist.
    let ouroboros = Arc::new(MockOuroboros::new());
    let supervisor = Arc::new(SchedulerAwareSupervisor::new(
        scheduler.clone(),
        event_bus.clone(),
    ));
    let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));

    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        scheduler.clone(),
        access_manager.clone(),
        a2a.clone(),
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    );
    let _orchestrator = Orchestrator::with_config(
        ouroboros,
        event_bus,
        state_store,
        lifecycle,
        make_evolution_config(0),
    );
    // Orchestrator is created successfully — shared state is fine.
}

// ===========================================================================
// Access Manager Enforcing Permissions
// ===========================================================================

#[tokio::test]
async fn test_access_manager_blocks_dangerous_tools() {
    let mut access = AccessManager::new();

    // Create a restrictive permission set (no dangerous tools).
    let mut perms = AgentPermissions::for_new_agent("safe-agent");
    perms.allowed_tools.clear(); // Start with nothing.
    perms.allow_tool("read"); // Only safe tools.
    perms.allow_tool("grep");
    access.set_permissions(perms);

    // Verify dangerous tools are blocked.
    assert!(!access.can_use_tool("safe-agent", "bash")); // bash not allowed.
    assert!(!access.can_use_tool("safe-agent", "rm")); // rm not allowed.
    assert!(!access.can_use_tool("safe-agent", "sudo")); // sudo not allowed.
    assert!(access.can_use_tool("safe-agent", "read")); // read is allowed.
    assert!(access.can_use_tool("safe-agent", "grep")); // grep is allowed.

    // Unknown agent gets no tools.
    assert!(!access.can_use_tool("unknown-agent", "read"));
}

#[tokio::test]
async fn test_access_manager_enforces_path_restrictions() {
    let mut access = AccessManager::new();

    let mut perms = AgentPermissions::for_new_agent("file-agent");
    perms.allowed_paths = vec![
        "/workspace/**".to_string(),
        "/home/user/projects/**".to_string(),
    ];
    perms.denied_paths = vec![
        "/workspace/secrets/**".to_string(),
        "/workspace/.oxios/**".to_string(),
    ];
    access.set_permissions(perms);

    // Allowed paths.
    assert!(access.can_access_path("file-agent", "/workspace/file.txt"));
    assert!(access.can_access_path("file-agent", "/workspace/subdir/code.rs"));
    assert!(access.can_access_path("file-agent", "/home/user/projects/app/main.rs"));

    // Blocked: outside allowed paths.
    assert!(!access.can_access_path("file-agent", "/etc/passwd"));
    assert!(!access.can_access_path("file-agent", "/root/.ssh/id_rsa"));

    // Blocked: denied pattern matches.
    assert!(!access.can_access_path("file-agent", "/workspace/secrets/api-key.txt"));
    assert!(!access.can_access_path("file-agent", "/workspace/.oxios/config.toml"));
}

#[tokio::test]
async fn test_access_manager_audit_log_on_denied_access() {
    let mut access = AccessManager::new();

    let perms = AgentPermissions::for_new_agent("audited-agent");
    access.set_permissions(perms);

    // Attempt denied operations.
    access.can_use_tool("audited-agent", "network"); // not in allowed set.
    access.can_access_path("audited-agent", "/etc/shadow"); // path not allowed.

    let log = access.audit_log();
    assert_eq!(log.len(), 2);

    // Both should be marked as denied.
    assert!(!log[0].allowed);
    assert!(!log[1].allowed);

    // Check reason is recorded.
    assert!(log[0].reason.is_some());
    assert!(log[1].reason.is_some());

    // Check denied_actions filter.
    let denied = access.denied_actions();
    assert_eq!(denied.len(), 2);
}

#[tokio::test]
async fn test_access_manager_network_and_fork_permissions() {
    let mut access = AccessManager::new();

    // Agent with network but no fork.
    let mut perms = AgentPermissions::for_new_agent("web-agent");
    perms.enable_network();
    // can_fork stays false by default.
    access.set_permissions(perms);

    assert!(access.can_access_network("web-agent"));
    assert!(!access.can_fork("web-agent"));

    // Agent with fork but no network.
    let mut perms2 = AgentPermissions::for_new_agent("fork-agent");
    perms2.enable_forking();
    // network_access stays false.
    access.set_permissions(perms2);

    assert!(!access.can_access_network("fork-agent"));
    assert!(access.can_fork("fork-agent"));

    // Agent with execution time and memory limits.
    let mut perms3 = AgentPermissions::for_new_agent("limited-agent");
    perms3.max_execution_time_secs = 60;
    perms3.max_memory_mb = 256;
    access.set_permissions(perms3);

    assert!(access.can_execute_for("limited-agent", 30));
    assert!(access.can_execute_for("limited-agent", 60));
    assert!(!access.can_execute_for("limited-agent", 61));
    assert!(access.can_use_memory("limited-agent", 128));
    assert!(!access.can_use_memory("limited-agent", 257));
}

#[tokio::test]
async fn test_access_manager_permission_lifecycle() {
    let mut access = AccessManager::new();

    // Create permissions.
    access.set_permissions(AgentPermissions::for_new_agent("lifecycle-agent"));

    // Check agent is listed.
    assert!(
        access
            .list_agents()
            .contains(&"lifecycle-agent".to_string())
    );

    // Grant a tool.
    let perms = access.get_or_create_permissions("lifecycle-agent");
    perms.allow_tool("custom-tool");

    // Verify tool is now allowed.
    assert!(access.can_use_tool("lifecycle-agent", "custom-tool"));

    // Remove permissions.
    access.remove_permissions("lifecycle-agent");

    // Agent no longer listed.
    assert!(
        !access
            .list_agents()
            .contains(&"lifecycle-agent".to_string())
    );

    // All access denied for removed agent.
    assert!(!access.can_use_tool("lifecycle-agent", "custom-tool"));
    assert!(!access.can_access_path("lifecycle-agent", "/workspace/test.txt"));
    assert!(!access.can_access_network("lifecycle-agent"));
}
