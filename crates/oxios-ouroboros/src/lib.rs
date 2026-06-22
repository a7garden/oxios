//! Unified intent handling for Oxios.
//!
//! Replaces the former five-phase Ouroboros protocol (interview → seed →
//! execute → evaluate → evolve) with a single-path design:
//!
//! 1. **assess** — classify the message (conversation / clarify / task + scope)
//! 2. **crystallize** — turn substantial tasks into a structured directive
//! 3. **execute** — run the agent
//! 4. **review** — check the result (substantial tasks only)
//! 5. **retry** — re-execute with feedback if review fails
//!
//! See `docs/rfc-027-unified-intent-handling.md` for the full design.
//!
//! The core idea from Ouroboros — ask when intent is unclear, verify substantial
//! results — is preserved. The ceremony (Phase enum, Seed god-object, trait
//! indirection) is removed.

#![warn(missing_docs)]

// New intent handling modules
pub mod assessment;
pub mod directive;
pub mod engine;
pub mod prompts;

// Legacy modules — kept temporarily for incremental migration.
// Will be removed in a follow-up phase.
pub mod degraded;
pub mod evaluation;
pub mod interview;
pub mod model_resolver;
pub mod ouroboros_engine;
pub mod protocol;
pub mod seed;

// Legacy re-exports (to be removed)
pub use evaluation::EvaluationResult;
pub use interview::InterviewResult;
pub use model_resolver::{ModelResolver, ResolvedModel, StaticModelResolver};
pub use ouroboros_engine::OuroborosEngine;
pub use protocol::{ExecutionResult, OuroborosProtocol, Phase, ToolCallRecord};
pub use seed::{AmbiguityScore, Entity, Seed};

// New public API (RFC-027)
pub use assessment::{Assessment, Question, QuestionKind, QuestionOption, Scope};
pub use directive::{Directive, Exchange, ExecEnv, MsgCtx, Verdict};
pub use engine::{IntentEngine, IntentEngineOps};
pub use prompts::{ASSESS_SYSTEM_PROMPT, CRYSTALLIZE_SYSTEM_PROMPT, REVIEW_SYSTEM_PROMPT};
