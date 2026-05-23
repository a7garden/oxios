//! Prompt assembly system.
//!
//! **Design phase** — this module defines the target architecture for prompt
//! assembly. It is not yet wired into `agent_runtime.rs` (which still uses
//! `push_str`). The modules here will replace the inline prompt construction
//! once the full integration is complete.
//!
//! This module provides a structured, cache-aware prompt builder that replaces
//! ad-hoc `push_str` assembly with a layered architecture:
//!
//! - **Layer 1** (`builder`): Pure function rendering from structured inputs.
//! - **Cache boundary** (`cache_boundary`): Stable/volatile split for provider caching.
//! - **Normalization** (`normalize`): Deterministic output for cache stability.
//! - **Types** (`types`): Shared data structures.
//!
//! # Usage
//!
//! ```ignore
//! use oxios_kernel::prompt::{build_system_prompt, PromptOptions, PromptMode};
//!
//! let opts = PromptOptions {
//!     mode: PromptMode::Full,
//!     model_id: Some("anthropic/claude-sonnet-4".into()),
//!     ..Default::default()
//! };
//! let prompt = build_system_prompt(&opts);
//! ```
//!
//! # Architecture
//!
//! Inspired by OpenClaw's three-layer prompt assembly and Pi's progressive disclosure:
//!
//! 1. **Stable prefix** (cached): Tool summaries, safety, context files, skills catalog.
//! 2. **Cache boundary**: Invisible marker for provider-specific caching.
//! 3. **Volatile suffix** (per-turn): Channel guidance, runtime info, provider contributions.

pub mod builder;
pub mod cache_boundary;
pub mod normalize;
pub mod types;

// Re-export primary API
pub use builder::build_system_prompt;
pub use cache_boundary::PromptSplit;
pub use types::{PromptMode, PromptOptions, PromptSurface};
