//! Authentication manager for API key validation.
//!
//! Provides a simple bearer-token authentication mechanism.
//! Tokens are loaded from a JSON file (one key per line or a JSON array).

use std::collections::HashSet;
use std::path::Path;

/// Manages API key authentication.
///
/// Keys are loaded from a JSON file at startup. If the file doesn't exist,
/// no keys are loaded and all requests will fail validation (unless auth is disabled).
pub struct AuthManager {
    /// Set of valid bearer tokens.
    valid_tokens: HashSet<String>,
}

impl AuthManager {
    /// Creates a new `AuthManager` with no valid tokens.
    pub fn new() -> Self {
        Self {
            valid_tokens: HashSet::new(),
        }
    }

    /// Loads API keys from a JSON file.
    ///
    /// The file should contain a JSON array of strings, e.g.:
    /// `["key1", "key2"]`
    ///
    /// If the file doesn't exist, logs a warning and returns with no keys.
    pub fn load_from_file(&mut self, path: &Path) -> anyhow::Result<()> {
        if !path.exists() {
            tracing::warn!(path = %path.display(), "API keys file not found; no keys loaded");
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let keys: Vec<String> = serde_json::from_str(&content).unwrap_or_else(|e| {
            // Try line-delimited format as fallback
            tracing::warn!(error = %e, "Failed to parse API keys as JSON array, trying line-delimited");
            content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        });

        let count = keys.len();
        for key in keys {
            self.valid_tokens.insert(key);
        }
        tracing::info!(count, path = %path.display(), "API keys loaded");
        Ok(())
    }

    /// Validates a bearer token against the loaded keys.
    pub fn validate(&mut self, token: &str) -> bool {
        self.valid_tokens.contains(token)
    }

    /// Returns the number of loaded keys.
    pub fn key_count(&self) -> usize {
        self.valid_tokens.len()
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty() {
        let mut mgr = AuthManager::new();
        assert!(!mgr.validate("any-key"));
    }

    #[test]
    fn test_validate_key() {
        let mut mgr = AuthManager::new();
        mgr.valid_tokens.insert("test-key".to_string());
        assert!(mgr.validate("test-key"));
        assert!(!mgr.validate("wrong-key"));
    }
}
