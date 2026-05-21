//! Authentication token management.
//!
//! Ported from files.md (`server/sync/tokens.rs`) by Artem Zakirullin.
//! Manages one-time and permanent tokens for API authentication.

use std::collections::HashMap;
use std::time::SystemTime;

use parking_lot::Mutex;
use rand::Rng;
use sha2::{Digest, Sha256};

/// Token length in bytes.
const TOKEN_LENGTH: usize = 32;

/// One-time token expiration (10 minutes).
const ONE_TIME_EXPIRATION_SECS: i64 = 10 * 60;

/// Ban duration for invalid token attempts (10 minutes).
const BAN_DURATION_SECS: i64 = 10 * 60;

/// Internal one-time token record.
struct OneTimeToken {
    user_id: i64,
    expires_at: i64,
}

/// Token manager for authentication.
///
/// Manages one-time tokens (for initial auth) and permanent tokens
/// (for ongoing API access). Thread-safe via `parking_lot::Mutex`.
pub struct TokenManager {
    one_time_tokens: Mutex<HashMap<String, OneTimeToken>>,
    blocked_ips: Mutex<HashMap<String, i64>>,
    tokens_salt: String,
    tokens_dir: String,
}

impl TokenManager {
    /// Create a new token manager.
    pub fn new(tokens_dir: String, tokens_salt: String) -> Self {
        Self {
            one_time_tokens: Mutex::new(HashMap::new()),
            blocked_ips: Mutex::new(HashMap::new()),
            tokens_dir,
            tokens_salt,
        }
    }

    /// Generate a one-time token for a user.
    pub fn gen_one_time_token(&self, user_id: i64) -> String {
        let token = gen_token();
        let expires_at = now_timestamp() + ONE_TIME_EXPIRATION_SECS;
        self.one_time_tokens.lock().insert(
            token.clone(),
            OneTimeToken {
                user_id,
                expires_at,
            },
        );
        token
    }

    /// Issue a permanent token in exchange for a one-time token.
    pub fn issue_permanent_token(&self, one_time_token: &str) -> Option<String> {
        let user_id = {
            let tokens = self.one_time_tokens.lock();
            let data = tokens.get(one_time_token)?;
            if now_timestamp() > data.expires_at {
                return None;
            }
            data.user_id
        };
        self.one_time_tokens.lock().remove(one_time_token);

        let permanent = gen_token();
        let hashed = self.hash_token(&permanent);

        // Write to filesystem
        let path = std::path::Path::new(&self.tokens_dir).join(&hashed);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, user_id.to_string());

        Some(permanent)
    }

    /// Find a user ID by permanent token.
    pub fn find_user_id(&self, token: &str) -> Option<i64> {
        let hashed = self.hash_token(token);
        let path = std::path::Path::new(&self.tokens_dir).join(&hashed);
        let data = std::fs::read_to_string(&path).ok()?;
        data.parse().ok()
    }

    /// Check if an IP is currently blocked.
    pub fn is_ip_blocked(&self, ip: &str) -> bool {
        let blocked = self.blocked_ips.lock();
        if let Some(unblock_time) = blocked.get(ip) {
            now_timestamp() < *unblock_time
        } else {
            false
        }
    }

    /// Block an IP for invalid token attempts.
    pub fn block_ip(&self, ip: &str) {
        self.blocked_ips
            .lock()
            .insert(ip.to_string(), now_timestamp() + BAN_DURATION_SECS);
    }

    /// Extract IP from a remote address string (strip port).
    pub fn get_ip_from_remote_addr(remote_addr: &str) -> String {
        remote_addr
            .rsplit_once(':')
            .map(|(host, _)| host.to_string())
            .unwrap_or(remote_addr.to_string())
    }

    fn hash_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hasher.update(self.tokens_salt.as_bytes());
        hex::encode(hasher.finalize())
    }
}

fn gen_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; TOKEN_LENGTH] = rng.gen();
    hex::encode(bytes)
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_token_length() {
        assert_eq!(gen_token().len(), 64);
    }

    #[test]
    fn test_ip_extraction() {
        assert_eq!(
            TokenManager::get_ip_from_remote_addr("1.2.3.4:8080"),
            "1.2.3.4"
        );
        assert_eq!(TokenManager::get_ip_from_remote_addr("1.2.3.4"), "1.2.3.4");
    }

    #[test]
    fn test_block_unblock() {
        let mgr = TokenManager::new("/tmp/test_tokens".into(), "salt".into());
        assert!(!mgr.is_ip_blocked("1.2.3.4"));
        mgr.block_ip("1.2.3.4");
        assert!(mgr.is_ip_blocked("1.2.3.4"));
    }
}
