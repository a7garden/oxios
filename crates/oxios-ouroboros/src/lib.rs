//! Unified intent handling for Oxios.
//!
//! RFC-033 unified streaming: every message flows through the agent loop
//! directly — the former `assess`/`crystallize` external LLM gates were
//! removed. The agent's own UNDERSTAND → PLAN → EXECUTE → VERIFY → REPORT
//! protocol (in `agent_runtime.rs`) classifies and plans inline. The only
//! surviving external call is `review`, gated on a Directive that carries
//! acceptance criteria.

#![warn(missing_docs)]

pub mod resilience;

pub mod directive;
pub mod engine;
pub mod fallback;
pub mod model_resolver;
pub mod prompts;
pub mod types;

pub use directive::{Directive, Exchange, ExecEnv, MsgCtx, Verdict};
pub use engine::{IntentEngine, IntentEngineOps};
pub use model_resolver::{ModelResolver, ResolvedModel, StaticModelResolver};
pub use prompts::REVIEW_SYSTEM_PROMPT;
pub use resilience::FailureClass;
pub use types::{ExecutionResult, InterviewOptionOutput, InterviewQuestionOutput, ToolCallRecord};
