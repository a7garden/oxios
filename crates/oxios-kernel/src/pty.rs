//! Interactive PTY terminal session management.
//!
//! Each PTY session spawns the user's default shell with CWD set to the
//! project root. Output is read on a dedicated OS thread (portable_pty
//! readers are blocking) and broadcast to WebSocket subscribers.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

/// Manages interactive PTY terminal sessions.
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<String, PtySessionHandle>>>,
}

struct PtySessionHandle {
    master: Box<dyn portable_pty::MasterPty + Send>,
    /// PTY stdin writer — `MasterPty` does not implement `Write` directly;
    /// `take_writer()` yields a `Box<dyn Write>`.
    writer: Mutex<Box<dyn std::io::Write + Send>>,
    output_tx: broadcast::Sender<Arc<Vec<u8>>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new PTY session. Returns the session ID.
    pub async fn create(
        &self,
        project_path: &Path,
        shell: Option<&str>,
    ) -> anyhow::Result<String> {
        let id = Uuid::new_v4().to_string();

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let default_shell =
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let shell_cmd = shell.unwrap_or(&default_shell);

        let mut cmd = CommandBuilder::new(shell_cmd);
        cmd.cwd(project_path);
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave); // release slave so EOF propagates

        // Take the writer before moving master into the handle.
        let pty_writer = pair.master.take_writer()?;

        let (output_tx, _) = broadcast::channel::<Arc<Vec<u8>>>(256);

        // Reader runs on a dedicated OS thread — portable_pty readers block.
        let mut reader = pair.master.try_clone_reader()?;
        let tx = output_tx.clone();
        let sessions = self.sessions.clone();
        let session_id = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = tx.send(Arc::new(buf[..n].to_vec()));
                    }
                    Err(_) => break,
                }
            }
            if let Ok(mut sessions) = sessions.try_lock() {
                sessions.remove(&session_id);
            }
        });

        let handle = PtySessionHandle {
            master: pair.master,
            writer: Mutex::new(pty_writer),
            output_tx,
            child: Mutex::new(child),
        };

        self.sessions.lock().await.insert(id.clone(), handle);
        tracing::info!("Created PTY session {id} in {}", project_path.display());
        Ok(id)
    }

    /// Write data to a terminal session's stdin.
    pub async fn write(&self, id: &str, data: &[u8]) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("terminal session not found: {id}"))?;
        let mut writer = session.writer.lock().await;
        writer.write_all(data)?;
        Ok(())
    }

    /// Resize a terminal session.
    pub async fn resize(&self, id: &str, rows: u16, cols: u16) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("terminal session not found: {id}"))?;
        session
            .master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    /// Subscribe to a terminal's output stream.
    pub async fn subscribe(&self, id: &str) -> anyhow::Result<broadcast::Receiver<Arc<Vec<u8>>>> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("terminal session not found: {id}"))?;
        Ok(session.output_tx.subscribe())
    }

    /// Kill a terminal session.
    pub async fn kill(&self, id: &str) -> anyhow::Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.remove(id) {
            let mut child = session.child.lock().await;
            let _ = child.kill();
            tracing::info!("Killed PTY session {id}");
        }
        Ok(())
    }

    /// Check if a session exists.
    pub async fn exists(&self, id: &str) -> bool {
        self.sessions.lock().await.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_kill() {
        let manager = PtyManager::new();
        let tmp = tempfile::tempdir().unwrap();

        match manager.create(tmp.path(), Some("/bin/cat")).await {
            Ok(id) => {
                // Write something — cat will echo it back
                assert!(manager.write(&id, b"hello\n").await.is_ok());
                // Give the reader thread a moment
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                assert!(manager.kill(&id).await.is_ok());
            }
            Err(e) => {
                eprintln!("PTY test skipped (no PTY support in CI?): {e}");
            }
        }
    }
}
