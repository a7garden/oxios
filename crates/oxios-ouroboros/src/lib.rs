//! Ouroboros spec-first protocol for Oxios.
//!
//! This crate implements the five-phase Ouroboros methodology:
//! interview → seed → execute → evaluate → evolve.
//!
//! The protocol concept and ambiguity scoring model are derived from
//! the Ouroboros project by Q00 (https://github.com/Q00/ouroboros).
//! Rust implementation is original — no source code was copied.
//! See THIRD-PARTY-NOTICES.md for full attribution.
//!
//! Never execute without a spec. Clarify until ambiguity ≤ 0.2.

#![warn(missing_docs)]

pub mod degraded;
pub mod eval_cache;
pub mod evaluation;
pub mod interview;
pub mod lateral;
pub mod ouroboros_engine;
pub mod protocol;
pub mod regression;
pub mod seed;

pub use evaluation::EvaluationResult;
pub use interview::InterviewResult;
pub use ouroboros_engine::OuroborosEngine;
pub use protocol::{ExecutionResult, OuroborosProtocol, Phase};
pub use seed::{AmbiguityScore, Entity, Seed};
