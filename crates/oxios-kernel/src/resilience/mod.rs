//! Resilience layer (RFC-029) — failure classification and recovery
//! coordination.
//!
//! # Layers
//!
//! - [`classify`] — maps a provider error to a [`FailureClass`].
//! - [`budget`] — `AttemptBudget`, bounds total retries.
//! - [`coordinator`] — `RecoveryCoordinator`, the OTP-style recovery
//!   ladder (L1 backoff → L2 model swap → terminal).
//!
//! # Honest limitation
//!
//! The typed `oxi_ai::ProviderError` is stringified at the oxi-agent
//! boundary. Downcasting across that boundary is not possible today, so
//! `classify` uses Display-string heuristics. See [`classify`] for the
//! pattern list and its caveats.

pub mod budget;
pub mod classify;
pub mod coordinator;
pub mod error;
pub mod health;

pub use budget::AttemptBudget;
pub use classify::classify;
pub use coordinator::{RecoveryCoordinator, ResilienceConfig};
pub use error::AgentRunError;
pub use health::{BreakerConfig, ProviderHealthRegistry};
pub use oxios_ouroboros::FailureClass;
