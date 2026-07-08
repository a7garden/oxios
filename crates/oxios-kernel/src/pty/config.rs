//! Snapshot of `PtyConfig` for read-only access without holding the RwLock
//! across an await point (RFC-038 §8.1).
use crate::config::PtyConfig;

/// Cheap clone of `PtyConfig` for snapshot reads.
pub type PtyConfigSnapshot = PtyConfig;
