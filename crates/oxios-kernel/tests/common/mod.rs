//! Shared test helpers for oxios-kernel integration and e2e tests (RFC-027).
//!
//! Provides a mock IntentEngine that returns predictable responses without
//! making real LLM calls.

#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use oxios_ouroboros::{
    Assessment, Directive, ExecutionResult, IntentEngineOps, InterviewResult, EvaluationResult,
    MsgCtx, OuroborosProtocol, Scope, Seed, Verdict,
};

/// Mock IntentEngine that returns configurable responses.
pub struct MockIntentEngine {
    pub assess_response: RwLock<Assessment>,
    pub crystallize_response: RwLock<Directive>,
    pub review_response: RwLock<Verdict>,
}

impl MockIntentEngine {
    pub fn new() -> Self {
        Self {
            assess_response: RwLock::new(Assessment::Task(Scope::Substantial)),
            crystallize_response: RwLock::new(Directive::from_message("")),
            review_response: RwLock::new(Verdict {
                passed: true,
                score: 1.0,
                notes: vec!["Mock review passed".into()],
                gaps: vec![],
            }),
        }
    }

    pub fn with_assess(self, assessment: Assessment) -> Self {
        *self.assess_response.write() = assessment;
        self
    }

    pub fn with_crystallize(self, directive: Directive) -> Self {
        *self.crystallize_response.write() = directive;
        self
    }

    pub fn with_review(self, verdict: Verdict) -> Self {
        *self.review_response.write() = verdict;
        self
    }

    pub fn into_arc(self) -> Arc<dyn IntentEngineOps> {
        Arc::new(self)
    }
}

impl Default for MockIntentEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IntentEngineOps for MockIntentEngine {
    async fn assess(&self, msg: &str, _ctx: &MsgCtx) -> anyhow::Result<Assessment> {
        let resp = self.assess_response.read().clone();
        Ok(match resp {
            Assessment::Clarify { questions } if questions.is_empty() => Assessment::Clarify {
                questions: vec![oxios_ouroboros::Question {
                    id: "q1".into(),
                    text: format!("Clarify: {msg}"),
                    kind: oxios_ouroboros::QuestionKind::FreeText,
                    options: vec![],
                }],
            },
            other => other,
        })
    }

    async fn crystallize(&self, msg: &str, _ctx: &MsgCtx) -> anyhow::Result<Directive> {
        let mut resp = self.crystallize_response.read().clone();
        if resp.goal.is_empty() {
            resp.goal = msg.to_string();
        }
        if resp.original_request.is_empty() {
            resp.original_request = msg.to_string();
        }
        Ok(resp)
    }

    async fn review(
        &self,
        _directive: &Directive,
        _result: &ExecutionResult,
    ) -> anyhow::Result<Verdict> {
        Ok(self.review_response.read().clone())
    }
}

/// A no-op OuroborosProtocol for the `Orchestrator::new` legacy field.
struct MockOuroborosProtocol;

#[async_trait]
impl OuroborosProtocol for MockOuroborosProtocol {
    async fn interview(&self, _user_input: &str) -> anyhow::Result<InterviewResult> {
        unimplemented!("handle_unified path does not use interview")
    }
    async fn generate_seed(
        &self,
        _interview: &InterviewResult,
    ) -> anyhow::Result<Seed> {
        unimplemented!("handle_unified path does not use generate_seed")
    }
    async fn execute(&self, _seed: &Seed) -> anyhow::Result<ExecutionResult> {
        unimplemented!("handle_unified path does not use execute")
    }
    async fn evaluate(
        &self,
        _seed: &Seed,
        _result: &ExecutionResult,
    ) -> anyhow::Result<EvaluationResult> {
        unimplemented!("handle_unified path does not use evaluate")
    }
    async fn evolve(
        &self,
        _seed: &Seed,
        _evaluation: &EvaluationResult,
    ) -> anyhow::Result<Option<Seed>> {
        unimplemented!("handle_unified path does not use evolve")
    }
    fn set_persona_prompt(&self, _prompt: Option<String>) {}
}

/// Build a test orchestrator wired with a MockIntentEngine.
pub fn build_test_orchestrator(
    supervisor: Arc<dyn oxios_kernel::supervisor::Supervisor>,
    state_store: Arc<oxios_kernel::state_store::StateStore>,
    event_bus: oxios_kernel::event_bus::EventBus,
) -> (Arc<oxios_kernel::Orchestrator>, Arc<MockIntentEngine>) {
    let scheduler = Arc::new(oxios_kernel::scheduler::AgentScheduler::default());
    let access_manager = Arc::new(parking_lot::Mutex::new(
        oxios_kernel::access_manager::AccessManager::new(),
    ));
    let a2a = Arc::new(oxios_kernel::a2a::A2AProtocol::new(event_bus.clone()));
    let lifecycle = oxios_kernel::agent_lifecycle::AgentLifecycleManager::new(
        supervisor,
        scheduler,
        access_manager,
        a2a,
        event_bus.clone(),
        300,
        vec![],
        true,
        "/tmp/oxios-test-workspace".to_string(),
    );

    let mock = Arc::new(MockIntentEngine::new());
    let orchestrator = oxios_kernel::Orchestrator::new(
        Arc::new(MockOuroborosProtocol),
        event_bus,
        state_store,
        lifecycle,
    );
    orchestrator.set_intent_engine(mock.clone() as Arc<dyn IntentEngineOps>);

    (Arc::new(orchestrator), mock)
}

/// Helper: build a Verdict that fails.
pub fn failing_verdict(gaps: Vec<String>) -> Verdict {
    Verdict {
        passed: false,
        score: 0.3,
        notes: vec!["Mock review failed".into()],
        gaps,
    }
}

/// Helper: build a Verdict that passes.
pub fn passing_verdict() -> Verdict {
    Verdict {
        passed: true,
        score: 1.0,
        notes: vec!["Mock review passed".into()],
        gaps: vec![],
    }
}
