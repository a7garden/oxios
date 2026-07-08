//! Error type for PTY subsystem (RFC-038).
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PtyError {
    /// Master switch disabled in config.
    #[error("pty subsystem is disabled in config")]
    Disabled,

    /// Per-principal session cap reached.
    #[error("session cap reached for principal: max {max}")]
    SessionCapReached { max: u32 },

    /// Requested shell binary is not in `allowed_shells` allowlist.
    #[error("shell not allowed: {shell:?}")]
    ShellNotAllowed { shell: String },

    /// Spawning the PTY child process failed.
    #[error("failed to spawn pty: {0}")]
    Spawn(String),

    /// Resize / write / close on an unknown session.
    #[error("session not found: {0}")]
    NotFound(String),

    /// Resize / write on a closed session.
    #[error("session {0} is closed")]
    Closed(String),

    /// Resize / write on a session owned by another principal.
    #[error("session {0} not owned by caller")]
    NotOwner(String),

    /// Master/slave I/O error.
    #[error("pty io: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages() {
        assert_eq!(
            PtyError::Disabled.to_string(),
            "pty subsystem is disabled in config"
        );
        assert_eq!(
            PtyError::SessionCapReached { max: 3 }.to_string(),
            "session cap reached for principal: max 3"
        );
        assert_eq!(
            PtyError::ShellNotAllowed { shell: "fish".into() }.to_string(),
            "shell not allowed: \"fish\""
        );
    }
}