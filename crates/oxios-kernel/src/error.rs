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

    /// Container is unavailable or not running.
    #[error("Container '{name}' is unavailable: {detail}")]
    ContainerUnavailable {
        /// Container name.
        name: String,
        /// Additional detail.
        detail: String,
    },

    /// Container backend is not available on this platform.
    #[error("Container backend not available on this platform")]
    BackendUnavailable,

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
            Self::ContainerUnavailable { .. } => HttpStatus::ServiceUnavailable,
            Self::BackendUnavailable => HttpStatus::ServiceUnavailable,
            Self::ProgramNotFound { .. } => HttpStatus::NotFound,
            Self::ProgramAlreadyExists { .. } => HttpStatus::Conflict,
            Self::InvalidConfig { .. } => HttpStatus::BadRequest,
            Self::SeedNotFound { .. } => HttpStatus::NotFound,
            Self::SessionNotFound { .. } => HttpStatus::NotFound,
            Self::StateStore(_) => HttpStatus::InternalServerError,
            Self::Internal(_) => HttpStatus::InternalServerError,
        }
    }
}

/// Convenience alias for results using [`KernelError`].
pub type KernelResult<T> = Result<T, KernelError>;
