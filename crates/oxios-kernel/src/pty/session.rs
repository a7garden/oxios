//! Per-session PTY state (RFC-038).
//!
//! A `PtySession` holds the master PTY handle and bookkeeping. The session
//! outlives the WebSocket — when the WS closes, the session becomes
//! `Detached` and may be re-attached (until `max_lifetime_secs` elapses).
use parking_lot::Mutex;
use portable_pty::{native_pty_system, MasterPty, PtySize as PortablePtySize};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::config::PtyConfig;

use super::error::PtyError;

/// Session id (ULID, time-sortable). Type alias for readability.
pub type PtySessionId = String;

/// PTY size used at spawn / resize time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl PtySize {
    /// Convert to portable-pty's representation.
    pub fn to_portable(self) -> PortablePtySize {
        PortablePtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
        }
    }

    /// Default 80×24.
    pub fn default_80x24() -> Self {
        Self {
            cols: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// State of a session.
pub enum PtySessionState {
    /// Open and bound to a WebSocket client.
    Attached {
        /// Sender for PTY bytes flowing to the WS client.
        ws_tx: mpsc::Sender<Vec<u8>>,
    },
    /// No client attached; orphan, awaiting re-attach or GC.
    Detached {
        /// Instant when this state was entered.
        orphan_since: Instant,
    },
    /// Exit code recorded; awaiting GC.
    Closed {
        /// Exit code recorded (None if killed by signal).
        exit_code: Option<i32>,
        /// Signal that terminated the process (None if exited normally).
        signal: Option<i32>,
        /// Instant when this state was entered.
        at: Instant,
    },
}

/// Per-session info exposed to the Web UI (`GET /api/terminal/sessions`).
#[derive(Debug, Clone, Serialize)]
pub struct PtySessionInfo {
    pub id: PtySessionId,
    pub shell: String,
    pub created_at_unix_ms: u64,
    pub last_input_at_unix_ms: u64,
    pub state: String,
    pub cols: u16,
    pub rows: u16,
}

/// A live PTY session.
pub struct PtySession {
    pub id: PtySessionId,
    pub principal: String,
    pub shell: String,
    pub created_at: Instant,
    pub size: PtySize,
    /// Last input time as Unix-millis; used for idle GC.
    pub last_input_ms: parking_lot::Mutex<u64>,
    /// Live master handle. Drop = kill the child.
    pub master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    pub state: Mutex<PtySessionState>,
}

impl PtySession {
    /// Construct a new session, spawning the shell via `portable-pty`.
    pub fn spawn(
        id: PtySessionId,
        principal: String,
        shell: String,
        size: PtySize,
        config: &PtyConfig,
    ) -> Result<Arc<Self>, PtyError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PortablePtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: size.pixel_width,
                pixel_height: size.pixel_height,
            })
            .map_err(|e| PtyError::Spawn(e.to_string()))?;

        let mut cmd = portable_pty::CommandBuilder::new(&shell);
        if let Some(cwd) = &config.working_directory {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("OXIOS_PTY_SESSION", &id);
        // Strip daemon secrets, then inherit the rest.
        for (k, v) in std::env::vars() {
            if config
                .env_strip_prefixes
                .iter()
                .any(|p| k.starts_with(p.as_str()))
            {
                continue;
            }
            cmd.env(&k, &v);
        }
        for (k, v) in &config.extra_env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        drop(pair.slave);

        let now_unix_ms = unix_millis_now();
        let session = Arc::new(Self {
            id,
            principal,
            shell,
            created_at: Instant::now(),
            size,
            last_input_ms: parking_lot::Mutex::new(now_unix_ms),
            master: Mutex::new(Some(pair.master)),
            state: Mutex::new(PtySessionState::Detached {
                orphan_since: Instant::now(),
            }),
        });

        // Wait task: transitions to Closed when child exits.
        let session_for_wait = Arc::clone(&session);
        tokio::spawn(async move {
            let exit = tokio::task::spawn_blocking(move || child.wait()).await;
            let (code, signal) = match exit {
                Ok(Ok(status)) => (status.exit_code(), None),
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, session = %session_for_wait.id, "pty child wait error");
                    (None, None)
                }
                Err(e) => {
                    tracing::warn!(error = %e, session = %session_for_wait.id, "pty wait join error");
                    (None, None)
                }
            };
            let mut st = session_for_wait.state.lock();
            *st = PtySessionState::Closed {
                exit_code: code,
                signal,
                at: Instant::now(),
            };
        });

        Ok(session)
    }

    /// Touch `last_input_ms` to now.
    pub fn touch_input(&self) {
        let mut g = self.last_input_ms.lock();
        *g = unix_millis_now();
    }

    /// Compute idle duration in milliseconds.
    pub fn idle_ms(&self) -> u64 {
        unix_millis_now().saturating_sub(*self.last_input_ms.lock())
    }

    /// Lifetime elapsed in milliseconds.
    pub fn lifetime_ms(&self) -> u64 {
        self.created_at.elapsed().as_millis() as u64
    }

    /// Snapshot for the UI listing endpoint.
    pub fn info(&self) -> PtySessionInfo {
        let state_label = match &*self.state.lock() {
            PtySessionState::Attached { .. } => "attached",
            PtySessionState::Detached { .. } => "detached",
            PtySessionState::Closed { .. } => "closed",
        };
        PtySessionInfo {
            id: self.id.clone(),
            shell: self.shell.clone(),
            created_at_unix_ms: unix_millis_from(self.created_at),
            last_input_at_unix_ms: *self.last_input_ms.lock(),
            state: state_label.to_string(),
            cols: self.size.cols,
            rows: self.size.rows,
        }
    }
}

/// Drop the session → kills the child process (PTY closes).
impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.master.lock().take();
    }
}

fn unix_millis_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn unix_millis_from(t: Instant) -> u64 {
    let now = Instant::now();
    let delta = now.saturating_duration_since(t);
    unix_millis_now().saturating_sub(delta.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_default_80x24() {
        let s = PtySize::default_80x24();
        assert_eq!(s.cols, 80);
        assert_eq!(s.rows, 24);
    }

    #[test]
    fn size_to_portable() {
        let s = PtySize {
            cols: 120,
            rows: 40,
            pixel_width: 0,
            pixel_height: 0,
        };
        let p = s.to_portable();
        assert_eq!(p.cols, 120);
        assert_eq!(p.rows, 40);
    }
}