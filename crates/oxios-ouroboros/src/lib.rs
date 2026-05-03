//! Ouroboros spec-first protocol for Oxios.
//!
//! This crate implements the five-phase Ouroboros methodology:
//! interview → seed → execute → evaluate → evolve.
//!
//! Never execute without a spec. Clarify until ambiguity ≤ 0.2.

#![warn(missing_docs)]

pub mod evaluation;
pub mod interview;
pub mod ouroboros_engine;
pub mod protocol;
pub mod seed;

pub use evaluation::EvaluationResult;
pub use interview::InterviewResult;
pub use ouroboros_engine::OuroborosEngine;
pub use protocol::{ExecutionResult, OuroborosProtocol, Phase};
pub use seed::{AmbiguityScore, Entity, Seed};
