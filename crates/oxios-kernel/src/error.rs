//! Typed error types for the Oxios kernel public API.
//!
//! Library consumers should match on these variants for structured error handling.
//! Internal implementation uses `anyhow` and wraps into [`KernelError::Internal`].

use thiserror::Error;

/// Oxios kernel error type.
#[derive(Debug, Error)]
pub enum KernelError {
    /// Requested agent was not found.
    #[error("Agent {id} not found")]
    AgentNotFound {
        /// The agent identifier.
        id: crate::types::AgentId,
    },

    /// Permission denied for the requested operation.
    #[error("Permission denied: {reason}")]
    PermissionDenied {
        /// Why permission was denied.
        reason: String,
    },

    /// Requested program was not found.
    #[error("Program '{name}' not found")]
    ProgramNotFound {
        /// Program name.
        name: String,
    },

    /// A program with this name is already installed.
    #[error("Program '{name}' already installed")]
    ProgramAlreadyExists {
        /// Program name.
        name: String,
    },

    /// Invalid configuration value.
    #[error("Invalid configuration: {detail}")]
    InvalidConfig {
        /// What's invalid.
        detail: String,
    },

    /// Requested seed was not found.
    #[error("Seed '{id}' not found")]
    SeedNotFound {
        /// Seed identifier.
        id: String,
    },

    /// Requested session was not found.
    #[error("Session '{id}' not found")]
    SessionNotFound {
        /// Session identifier.
        id: String,
    },

    /// I/O error from the state store.
    #[error("State store error: {0}")]
    StateStore(#[from] std::io::Error),

    /// An internal error wrapped from anyhow.
    #[error("{0}")]
    Internal(#[from] anyhow::Error),

    /// Memory subsystem error (HNSW index, embedding, etc.).
    #[error("Memory error: {reason}")]
    Memory {
        /// Detailed error reason.
        reason: String,
    },

    /// Operation timed out.
    #[error("Operation timed out: {context}")]
    Timeout {
        /// Context describing what timed out.
        context: String,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {context}")]
    RateLimited {
        /// Context describing what was rate limited.
        context: String,
    },
}

/// HTTP status code mapping (independent of any web framework).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatus {
    /// 200 OK
    Ok = 200,
    /// 400 Bad Request
    BadRequest = 400,
    /// 403 Forbidden
    Forbidden = 403,
    /// 404 Not Found
    NotFound = 404,
    /// 409 Conflict
    Conflict = 409,
    /// 429 Too Many Requests
    TooManyRequests = 429,
    /// 500 Internal Server Error
    InternalServerError = 500,
    /// 503 Service Unavailable
    ServiceUnavailable = 503,
}

impl From<HttpStatus> for u16 {
    fn from(status: HttpStatus) -> u16 {
        status as u16
    }
}

impl KernelError {
    /// Map this error to an HTTP-compatible status code.
    ///
    /// Returns a framework-agnostic [`HttpStatus`] that consumers can convert
    /// to their web framework's status type.
    pub fn http_status(&self) -> HttpStatus {
        match self {
            Self::AgentNotFound { .. } => HttpStatus::NotFound,
            Self::PermissionDenied { .. } => HttpStatus::Forbidden,

            Self::ProgramNotFound { .. } => HttpStatus::NotFound,
            Self::ProgramAlreadyExists { .. } => HttpStatus::Conflict,
            Self::InvalidConfig { .. } => HttpStatus::BadRequest,
            Self::SeedNotFound { .. } => HttpStatus::NotFound,
            Self::SessionNotFound { .. } => HttpStatus::NotFound,
            Self::StateStore(_) => HttpStatus::InternalServerError,
            Self::Memory { .. } => HttpStatus::InternalServerError,
            Self::Timeout { .. } => HttpStatus::ServiceUnavailable,
            Self::RateLimited { .. } => HttpStatus::TooManyRequests,
            Self::Internal(_) => HttpStatus::InternalServerError,
        }
    }
}

/// Convenience alias for results using [`KernelError`].
pub type KernelResult<T> = Result<T, KernelError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let id = crate::types::AgentId::new_v4();
        let err = KernelError::AgentNotFound { id };
        let msg = err.to_string();
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_all_http_status_mappings() {
        let id = crate::types::AgentId::new_v4();
        assert_eq!(
            u16::from(KernelError::AgentNotFound { id }.http_status()),
            404
        );
        assert_eq!(
            u16::from(
                KernelError::PermissionDenied {
                    reason: "test".into()
                }
                .http_status()
            ),
            403
        );
        assert_eq!(
            u16::from(KernelError::ProgramNotFound { name: "p".into() }.http_status()),
            404
        );
        assert_eq!(
            u16::from(KernelError::ProgramAlreadyExists { name: "p".into() }.http_status()),
            409
        );
        assert_eq!(
            u16::from(
                KernelError::InvalidConfig {
                    detail: "bad".into()
                }
                .http_status()
            ),
            400
        );
        assert_eq!(
            u16::from(KernelError::SeedNotFound { id: "s".into() }.http_status()),
            404
        );
        assert_eq!(
            u16::from(KernelError::SessionNotFound { id: "s".into() }.http_status()),
            404
        );
    }

    #[test]
    fn test_internal_error_wrapping() {
        let err = KernelError::Internal(anyhow::anyhow!("something broke"));
        assert!(err.to_string().contains("something broke"));
        assert_eq!(u16::from(err.http_status()), 500);
    }

    #[test]
    fn test_io_error_conversion() {
        let err =
            KernelError::StateStore(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(err.to_string().contains("gone"));
        assert_eq!(u16::from(err.http_status()), 500);
    }

    #[test]
    fn test_timeout_error_status() {
        let err = KernelError::Timeout {
            context: "agent execution exceeded 300s".into(),
        };
        assert!(err.to_string().contains("timed out"));
        assert!(err.to_string().contains("300s"));
        assert_eq!(u16::from(err.http_status()), 503);
    }

    #[test]
    fn test_rate_limited_error_status() {
        let err = KernelError::RateLimited {
            context: "API calls exceeded 60/min".into(),
        };
        assert!(err.to_string().contains("Rate limit exceeded"));
        assert!(err.to_string().contains("60/min"));
        assert_eq!(u16::from(err.http_status()), 429);
    }
}
