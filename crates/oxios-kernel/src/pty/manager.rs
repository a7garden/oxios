//! PtyManager — session registry, GC tick, attach/detach (RFC-038 §5.3, §6.3, §8.3).
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::config::PtyConfig;

use super::error::PtyError;
use super::session::{PtySession, PtySessionId, PtySessionInfo, PtySessionState, PtySize};

/// The shared PTY session registry + lifecycle manager.
pub struct PtyManager {
    sessions: RwLock<HashMap<PtySessionId, Arc<PtySession>>>,
    by_principal: parking_lot::Mutex<HashMap<String, HashSet<PtySessionId>>>,
    config: Arc<RwLock<PtyConfig>>,
}

impl PtyManager {
    pub fn new(config: Arc<RwLock<PtyConfig>>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            by_principal: parking_lot::Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Read-only snapshot of the current config.
    pub fn config_snapshot(&self) -> PtyConfig {
        self.config.read().clone()
    }

    /// Check if PTY subsystem is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.read().enabled
    }

    /// Generate a fresh session id (ULID-style: timestamp + random hex).
    pub fn new_id() -> PtySessionId {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut bytes = [0u8; 26];
        bytes[0..8].copy_from_slice(&now.to_be_bytes()[..8]);
        bytes[8] = (now >> 40) as u8;
        let rand_bytes: [u8; 16] = rand_bytes();
        bytes[10..26].copy_from_slice(&rand_bytes);
        hex_encode(&bytes)
    }

    /// Open a new PTY session. Performs shell allowlist check and per-principal cap.
    pub fn open(
        self: &Arc<Self>,
        principal: &str,
        shell: Option<String>,
        size: PtySize,
    ) -> Result<Arc<PtySession>, PtyError> {
        let cfg = self.config_snapshot();
        if !cfg.enabled {
            return Err(PtyError::Disabled);
        }
        let resolved = match shell {
            Some(s) if !s.is_empty() => s,
            _ => cfg.default_shell.clone(),
        };
        if !cfg.is_shell_allowed(&resolved) {
            return Err(PtyError::ShellNotAllowed { shell: resolved });
        }
        {
            let map = self.by_principal.lock();
            let count = map.get(principal).map(|s| s.len()).unwrap_or(0);
            if count >= cfg.max_sessions as usize {
                return Err(PtyError::SessionCapReached {
                    max: cfg.max_sessions,
                });
            }
        }
        let id = Self::new_id();
        let session = PtySession::spawn(id.clone(), principal.to_string(), resolved, size, &cfg)?;
        self.sessions
            .write()
            .insert(id.clone(), Arc::clone(&session));
        self.by_principal
            .lock()
            .entry(principal.to_string())
            .or_default()
            .insert(id.clone());
        tracing::info!(
            session = %id,
            shell = %session.shell,
            principal = %principal,
            "pty.open"
        );
        Ok(session)
    }

    /// Re-attach an existing session by id. Validates principal match.
    pub fn attach(
        self: &Arc<Self>,
        principal: &str,
        session_id: &str,
    ) -> Result<Arc<PtySession>, PtyError> {
        let session = {
            let map = self.sessions.read();
            map.get(session_id).cloned()
        }
        .ok_or_else(|| PtyError::NotFound(session_id.to_string()))?;
        if session.principal != principal {
            return Err(PtyError::NotOwner(session_id.to_string()));
        }
        if let PtySessionState::Closed { .. } = &*session.state.lock() {
            return Err(PtyError::Closed(session_id.to_string()));
        }
        Ok(session)
    }

    /// Write bytes to PTY stdin.
    pub fn write(&self, session_id: &str, bytes: &[u8]) -> Result<(), PtyError> {
        let session = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| PtyError::NotFound(session_id.to_string()))?;
        let mut guard = session.master.lock();
        let master = guard
            .as_mut()
            .ok_or_else(|| PtyError::Closed(session_id.to_string()))?;
        master
            .take_writer()
            .map_err(|e| PtyError::Io(e.to_string()))?
            .write_all(bytes)
            .map_err(|e| PtyError::Io(e.to_string()))?;
        session.touch_input();
        Ok(())
    }

    /// Resize the PTY.
    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), PtyError> {
        let session = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| PtyError::NotFound(session_id.to_string()))?;
        let guard = session.master.lock();
        let master = guard
            .as_ref()
            .ok_or_else(|| PtyError::Closed(session_id.to_string()))?;
        let _ = master.resize(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        Ok(())
    }

    /// Take the master reader for streaming to a WS client.
    pub fn try_clone_reader(
        &self,
        session_id: &str,
    ) -> Result<Box<dyn std::io::Read + Send>, PtyError> {
        let session = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| PtyError::NotFound(session_id.to_string()))?;
        let guard = session.master.lock();
        let master = guard
            .as_ref()
            .ok_or_else(|| PtyError::Closed(session_id.to_string()))?;
        master
            .try_clone_reader()
            .map_err(|e| PtyError::Io(e.to_string()))
    }

    /// List sessions for a principal.
    pub fn list_sessions(&self, principal: &str) -> Vec<PtySessionInfo> {
        let ids: Vec<PtySessionId> = self
            .by_principal
            .lock()
            .get(principal)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let sessions = self.sessions.read();
        ids.into_iter()
            .filter_map(|id| sessions.get(&id).map(|s| s.info()))
            .collect()
    }

    /// Transition a session to Attached (called by WS handler on Open).
    pub fn mark_attached(&self, session_id: &str) -> bool {
        let session = match self.sessions.read().get(session_id).cloned() {
            Some(s) => s,
            None => return false,
        };
        let mut st = session.state.lock();
        match &*st {
            PtySessionState::Closed { .. } => false,
            _ => {
                let (tx, _rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
                *st = PtySessionState::Attached { ws_tx: tx };
                true
            }
        }
    }

    /// Mark a session as Detached (called by WS handler on Close).
    pub fn mark_detached(&self, session_id: &str) -> bool {
        let session = match self.sessions.read().get(session_id).cloned() {
            Some(s) => s,
            None => return false,
        };
        let mut st = session.state.lock();
        match &*st {
            PtySessionState::Closed { .. } => false,
            _ => {
                *st = PtySessionState::Detached {
                    orphan_since: Instant::now(),
                };
                true
            }
        }
    }

    /// Close + remove a session.
    pub fn close(&self, session_id: &str) -> Result<(), PtyError> {
        let session = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| PtyError::NotFound(session_id.to_string()))?;
        let _ = session.master.lock().take();
        let mut st = session.state.lock();
        *st = PtySessionState::Closed {
            exit_code: None,
            signal: None,
            at: Instant::now(),
        };
        self.sessions.write().remove(session_id);
        if let Some(set) = self.by_principal.lock().get_mut(&session.principal) {
            set.remove(session_id);
        }
        tracing::info!(
            session = %session_id,
            principal = %session.principal,
            "pty.close"
        );
        Ok(())
    }

    /// GC tick — close idle / past-max-lifetime sessions.
    pub fn gc_tick(&self) {
        let cfg = self.config_snapshot();
        let idle_ms_threshold = cfg.idle_timeout_secs.saturating_mul(1000);
        let max_life_ms = cfg.max_lifetime_secs.saturating_mul(1000);
        let mut to_close: Vec<PtySessionId> = Vec::new();
        {
            let map = self.sessions.read();
            for (id, s) in map.iter() {
                if s.lifetime_ms() > max_life_ms {
                    to_close.push(id.clone());
                    continue;
                }
                if s.idle_ms() > idle_ms_threshold {
                    let st = s.state.lock();
                    if !matches!(*st, PtySessionState::Closed { .. }) {
                        to_close.push(id.clone());
                    }
                }
            }
        }
        for id in to_close {
            let _ = self.close(&id);
        }
    }

    /// Spawn the GC tick task.
    pub fn start_gc(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let interval = {
                    let cfg = self.config_snapshot();
                    std::cmp::min(cfg.idle_timeout_secs / 4, 60).max(5)
                };
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                self.gc_tick();
            }
        })
    }
}

fn rand_bytes() -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut state = RandomState::new().build_hasher();
    state.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    let a = state.finish();
    state.write_u64(std::process::id() as u64);
    let b = state.finish();
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&a.to_be_bytes());
    out[8..].copy_from_slice(&b.to_be_bytes());
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_unique() {
        let a = PtyManager::new_id();
        let b = PtyManager::new_id();
        assert_ne!(a, b);
        assert!(a.len() >= 32);
    }

    #[test]
    fn disabled_blocks_open() {
        let cfg = Arc::new(RwLock::new(PtyConfig::default()));
        let m = Arc::new(PtyManager::new(cfg));
        let res = m.open("user", None, PtySize::default_80x24());
        assert!(matches!(res, Err(PtyError::Disabled)));
    }
}
