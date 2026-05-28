//! Error classification for user-facing error messages.
//!
//! Converts `anyhow::Error` into structured `UserFacingError` using a hybrid
//! of type-based classification and message-pattern heuristics.
//! When the kernel migrates to `thiserror`, `downcast` can replace heuristics.

use crate::message::{ErrorKind, UserFacingError};

/// Classifies an `anyhow::Error` into a user-friendly error.
///
/// Uses type checking, cause-chain traversal, and message-pattern matching
/// (in that priority). Falls back to `ErrorKind::Internal` when nothing matches.
pub fn classify_error(e: &anyhow::Error) -> UserFacingError {
    let kind = infer_kind(e);
    let message = user_message(&kind);
    let suggestion = suggest(&kind);

    UserFacingError {
        message,
        kind,
        suggestion,
    }
}

fn infer_kind(e: &anyhow::Error) -> ErrorKind {
    // 1. Type-based classification (exact).
    if e.is::<tokio::time::error::Elapsed>() {
        return ErrorKind::Timeout;
    }

    // 2. Cause-chain traversal.
    let mut source = e.source();
    while let Some(err) = source {
        if err.is::<tokio::time::error::Elapsed>() {
            return ErrorKind::Timeout;
        }
        source = err.source();
    }

    // 3. Message-pattern matching (heuristic).
    let msg = e.to_string().to_lowercase();
    if msg.contains("rate limit")
        || msg.contains("api key")
        || msg.contains("provider")
    {
        return ErrorKind::ProviderError;
    }
    if msg.contains("permission")
        || msg.contains("unauthorized")
        || msg.contains("access denied")
    {
        return ErrorKind::PermissionDenied;
    }
    if msg.contains("timeout") || msg.contains("deadline exceeded") {
        return ErrorKind::Timeout;
    }
    if msg.contains("validation")
        || msg.contains("invalid")
        || msg.contains("empty")
    {
        return ErrorKind::ValidationError;
    }

    ErrorKind::Internal
}

fn user_message(kind: &ErrorKind) -> String {
    match kind {
        ErrorKind::ExecutionFailed => {
            "요청을 처리하는 중 오류가 발생했습니다.".to_string()
        }
        ErrorKind::ProviderError => {
            "AI 서비스에 일시적인 문제가 있습니다. 잠시 후 다시 시도해 주세요.".to_string()
        }
        ErrorKind::Timeout => {
            "요청 처리 시간이 초과되었습니다.".to_string()
        }
        ErrorKind::PermissionDenied => {
            "이 작업을 수행할 권한이 없습니다.".to_string()
        }
        ErrorKind::ValidationError => {
            "입력이 올바르지 않습니다.".to_string()
        }
        ErrorKind::Internal => {
            "내부 오류가 발생했습니다.".to_string()
        }
    }
}

fn suggest(kind: &ErrorKind) -> Option<String> {
    match kind {
        ErrorKind::ProviderError => {
            Some("1-2분 후 다시 시도하거나 다른 모델을 선택하세요.".to_string())
        }
        ErrorKind::Timeout => {
            Some("더 간단한 요청으로 시도하거나 타임아웃을 늘리세요.".to_string())
        }
        ErrorKind::PermissionDenied => {
            Some("관리자에게 권한을 요청하세요.".to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_from_message_heuristic() {
        let e = anyhow::anyhow!("connection timeout");
        let ufe = classify_error(&e);
        assert_eq!(ufe.kind, ErrorKind::Timeout);
    }

    #[test]
    fn provider_from_message() {
        let e = anyhow::anyhow!("rate limit exceeded");
        let ufe = classify_error(&e);
        assert_eq!(ufe.kind, ErrorKind::ProviderError);
    }

    #[test]
    fn permission_from_message() {
        let e = anyhow::anyhow!("permission denied for resource");
        let ufe = classify_error(&e);
        assert_eq!(ufe.kind, ErrorKind::PermissionDenied);
    }

    #[test]
    fn validation_from_message() {
        let e = anyhow::anyhow!("invalid input provided");
        let ufe = classify_error(&e);
        assert_eq!(ufe.kind, ErrorKind::ValidationError);
    }

    #[test]
    fn internal_fallback() {
        let e = anyhow::anyhow!("something went wrong in the system");
        let ufe = classify_error(&e);
        assert_eq!(ufe.kind, ErrorKind::Internal);
    }

    #[test]
    fn suggestion_for_provider() {
        let e = anyhow::anyhow!("api key invalid");
        let ufe = classify_error(&e);
        assert!(ufe.suggestion.is_some());
        assert_eq!(ufe.kind, ErrorKind::ProviderError);
    }

    #[test]
    fn no_suggestion_for_internal() {
        let e = anyhow::anyhow!("unknown");
        let ufe = classify_error(&e);
        assert!(ufe.suggestion.is_none());
    }
}
