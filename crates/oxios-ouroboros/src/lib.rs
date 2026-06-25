//! Unified intent handling for Oxios.
//!
//! Every message flows through a single path:
//! 1. **assess** — classify the message (conversation / clarify / task + scope)
//! 2. **crystallize** — turn substantial tasks into a structured Directive
//! 3. **execute** — run the agent
//! 4. **review** — check the result (substantial tasks only)
//! 5. **retry** — re-execute with feedback if review fails

#![warn(missing_docs)]

pub mod resilience;

pub mod assessment;
pub mod directive;
pub mod engine;
pub mod fallback;
pub mod model_resolver;
pub mod prompts;
pub mod types;

pub use assessment::{Assessment, Question, QuestionKind, QuestionOption, Scope};
pub use directive::{Directive, Exchange, ExecEnv, MsgCtx, Verdict};
pub use engine::{IntentEngine, IntentEngineOps};
pub use model_resolver::{ModelResolver, ResolvedModel, StaticModelResolver};
pub use prompts::{ASSESS_SYSTEM_PROMPT, CRYSTALLIZE_SYSTEM_PROMPT, REVIEW_SYSTEM_PROMPT};
pub use resilience::FailureClass;
pub use types::{ExecutionResult, InterviewOptionOutput, InterviewQuestionOutput, ToolCallRecord};
