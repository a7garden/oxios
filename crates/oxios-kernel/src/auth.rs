//! API key authentication manager.
//!
//! Provides bearer token authentication for the HTTP API.
//! Keys are stored as SHA-256 hashes for security.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Prefix for all generated Oxios API keys.
const KEY_PREFIX: &str = "oxios_";

/// Metadata about an API key (stored alongside the hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMeta {
    /// Human-readable name for the key.
    pub name: String,
    /// When the key was created.
    pub created_at: String,
    /// When the key was last used (ISO 8601).
    pub last_used: Option<String>,
}

/// A stored API key entry (hash + metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyEntry {
    /// SHA-256 hash of the full API key.
    hash_hex: String,
    #[serde(flatten)]
    meta: KeyMeta,
}

/// API key file format.
#[derive(Debug, Default, Serialize, Deserialize)]
struct KeyFile {
    keys: Vec<KeyEntry>,
}

/// Manages API key authentication.
pub struct AuthManager {
    /// SHA-256 hash → KeyMeta lookup.
    entries: HashMap<String, KeyMeta>,
    /// Set of all valid hashes for O(1) lookup.
    valid_hashes: HashSet<String>,
    /// Path to persist keys (optional for in-memory-only mode).
    path: Option<std::path::PathBuf>,
}

impl AuthManager {
    /// Create a new AuthManager without persistence.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            valid_hashes: HashSet::new(),
            path: None,
        }
    }

    /// Create an AuthManager that persists keys to a file.
    pub fn with_persistence(path: impl Into<std::path::PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut mgr = Self {
            entries: HashMap::new(),
            valid_hashes: HashSet::new(),
            path: Some(path.clone()),
        };
        if path.exists() {
            mgr.load_from_file(&path)?;
        }
        Ok(mgr)
    }

    /// Load keys from a JSON file.
    pub fn load_from_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read API keys from {}", path.display()))?;
        let key_file: KeyFile =
            serde_json::from_str(&content).with_context(|| "Failed to parse API keys file")?;
        for entry in key_file.keys {
            self.valid_hashes.insert(entry.hash_hex.clone());
            self.entries.insert(entry.hash_hex, entry.meta);
        }
        tracing::info!(count = self.valid_hashes.len(), "Loaded API keys");
        Ok(())
    }

    /// Save keys to the persistence file.
    fn save_to_file(&self) -> Result<()> {
        if let Some(path) = &self.path {
            let key_file = KeyFile {
                keys: self
                    .entries
                    .iter()
                    .map(|(hash, meta)| KeyEntry {
                        hash_hex: hash.clone(),
                        meta: meta.clone(),
                    })
                    .collect(),
            };
            let content = serde_json::to_string_pretty(&key_file)?;
            // Write atomically via temp file
            let tmp_path = path.with_extension("tmp");
            std::fs::write(&tmp_path, &content)?;
            std::fs::rename(&tmp_path, path)?;
        }
        Ok(())
    }

    /// Generate a new API key.
    ///
    /// Returns the full key string (only shown once).
    pub fn generate_key(&mut self, name: &str) -> Result<String> {
        let key_bytes = Self::random_key();
        let full_key = format!("{}{}", KEY_PREFIX, hex::encode(&key_bytes));
        let hash = Self::hash_key(&full_key);
        let meta = KeyMeta {
            name: name.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_used: None,
        };
        self.valid_hashes.insert(hash.clone());
        self.entries.insert(hash, meta);
        self.save_to_file()?;
        tracing::info!(name = %name, "Generated new API key");
        Ok(full_key)
    }

    /// Validate a bearer token.
    pub fn validate(&mut self, token: &str) -> bool {
        let hash = Self::hash_key(token);
        if self.valid_hashes.contains(&hash) {
            // Update last_used
            if let Some(meta) = self.entries.get_mut(&hash) {
                meta.last_used = Some(chrono::Utc::now().to_rfc3339());
                let _ = self.save_to_file();
            }
            true
        } else {
            false
        }
    }

    /// Revoke an API key by name.
    pub fn revoke_key(&mut self, name: &str) -> Result<()> {
        let hashes_to_remove: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, meta)| meta.name == name)
            .map(|(hash, _)| hash.clone())
            .collect();
        if hashes_to_remove.is_empty() {
            anyhow::bail!("Key '{}' not found", name);
        }
        for hash in hashes_to_remove {
            self.valid_hashes.remove(&hash);
            self.entries.remove(&hash);
        }
        self.save_to_file()?;
        tracing::info!(name = %name, "Revoked API key");
        Ok(())
    }

    /// List all keys (metadata only, never expose the key itself).
    pub fn list_keys(&self) -> Vec<&KeyMeta> {
        self.entries.values().collect()
    }

    /// Check if any keys are configured.
    pub fn has_keys(&self) -> bool {
        !self.valid_hashes.is_empty()
    }

    /// Hash an API key using SHA-256.
    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Generate random bytes for a new key.
    fn random_key() -> [u8; 32] {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).expect("failed to generate random bytes");
        bytes
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
    fn generate_and_validate_key() {
        let mut mgr = AuthManager::new();
        let key = mgr.generate_key("test-key").unwrap();
        assert!(key.starts_with(KEY_PREFIX));
        assert!(mgr.validate(&key));
    }

    #[test]
    fn invalid_key_rejected() {
        let mut mgr = AuthManager::new();
        assert!(!mgr.validate("oxios_invalidkey"));
    }

    #[test]
    fn revoke_key() {
        let mut mgr = AuthManager::new();
        let key = mgr.generate_key("to-revoke").unwrap();
        assert!(mgr.validate(&key));
        mgr.revoke_key("to-revoke").unwrap();
        assert!(!mgr.validate(&key));
    }

    #[test]
    fn revoke_nonexistent_key_fails() {
        let mut mgr = AuthManager::new();
        assert!(mgr.revoke_key("no-such-key").is_err());
    }

    #[test]
    fn has_keys_reflects_state() {
        let mut mgr = AuthManager::new();
        assert!(!mgr.has_keys());
        mgr.generate_key("first").unwrap();
        assert!(mgr.has_keys());
    }

    #[test]
    fn list_keys_returns_metadata() {
        let mut mgr = AuthManager::new();
        mgr.generate_key("alpha").unwrap();
        mgr.generate_key("beta").unwrap();
        let names: Vec<&str> = mgr.list_keys().iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");

        let key = {
            let mut mgr = AuthManager::with_persistence(&path).unwrap();
            mgr.generate_key("persist-test").unwrap()
        };

        // Load from file in a fresh manager
        let mut mgr2 = AuthManager::with_persistence(&path).unwrap();
        assert!(mgr2.validate(&key));
        assert!(mgr2.has_keys());
    }

    #[test]
    fn hash_is_deterministic() {
        let h1 = AuthManager::hash_key("oxios_test123");
        let h2 = AuthManager::hash_key("oxios_test123");
        assert_eq!(h1, h2);
    }
}
