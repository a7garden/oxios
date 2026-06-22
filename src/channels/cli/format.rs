//! CLI response formatter.
//!
//! Formats outgoing messages for terminal output with ANSI-compatible
//! indicators for phase, evaluation, duration, and errors.

use oxios_gateway::format::ChannelFormatter;
use oxios_gateway::message::{ErrorKind, OutgoingMessage};

/// CLI-specific response formatter.
///
/// Formats outgoing messages for terminal output with emoji indicators
/// for phase, evaluation result, duration, and error classification.
pub struct CliFormatter;

impl ChannelFormatter for CliFormatter {
    fn format_success(&self, msg: &OutgoingMessage) -> String {
        let mut out = msg.content.clone();

        if let Some(meta) = &msg.meta {
            let eval_icon = if meta.evaluation_passed.unwrap_or(false) {
                "✅"
            } else {
                "⚠️"
            };
            if !meta.phase.is_empty() {
                out.push_str(&format!(
                    "\n{} {} | {}",
                    eval_icon,
                    meta.phase,
                    if meta.evaluation_passed.unwrap_or(false) {
                        "통과"
                    } else {
                        "미통과"
                    }
                ));
            }

            if let Some(tag) = &meta.project_tag {
                out.push_str(&format!(" | {tag}"));
            }

            if let Some(dur) = meta.duration_ms {
                if dur >= 1000 {
                    out.push_str(&format!(" | {:.1}s", dur as f64 / 1000.0));
                } else {
                    out.push_str(&format!(" | {dur}ms"));
                }
            }
        }

        out
    }

    fn format_error(&self, msg: &OutgoingMessage) -> String {
        let meta = msg.meta.as_ref();
        let kind = meta.and_then(|m| m.error.as_ref()).map(|e| e.kind);

        let icon = match kind {
            Some(ErrorKind::ExecutionFailed) => "❌",
            Some(ErrorKind::ProviderError) => "🔌",
            Some(ErrorKind::Timeout) => "⏱️",
            Some(ErrorKind::PermissionDenied) => "🔒",
            Some(ErrorKind::ValidationError) => "⚠️",
            _ => "💥",
        };

        let mut out = format!("{} {}", icon, msg.content);

        if let Some(err) = meta.and_then(|m| m.error.as_ref())
            && let Some(s) = &err.suggestion
        {
            out.push_str(&format!("\n💡 {s}"));
        }

        out
    }

    fn format_progress(&self, phase: &str) -> String {
        match phase {
            "Interview" => "🔍 분석 중...".into(),
            "Seed" => "📋 계획 수립 중...".into(),
            "Execute" => "⚡ 실행 중...".into(),
            "Evaluate" => "📊 평가 중...".into(),
            "Evolve" => "🔄 개선 중...".into(),
            _ => "⏳ 처리 중...".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxios_gateway::message::{ResponseMeta, UserFacingError};
    use std::collections::HashMap;

    fn make_msg(content: &str, meta: Option<ResponseMeta>) -> OutgoingMessage {
        OutgoingMessage {
            id: uuid::Uuid::new_v4(),
            channel: "cli".into(),
            user_id: "test-user".into(),
            content: content.into(),
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            meta,
            target_conn_id: None,
            seq: None,
        }
    }

    #[test]
    fn format_success_no_meta() {
        let msg = make_msg("Hello", None);
        let formatter = CliFormatter;
        assert_eq!(formatter.format_success(&msg), "Hello");
    }

    #[test]
    fn format_success_with_phase_and_eval() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: Some("[🔧 oxios]".into()),
            phase: "Execute".into(),
            evaluation_passed: Some(true),
            duration_ms: Some(1500),
            error: None,
            interview_questions: None,
            interview_round: None,
        };
        let msg = make_msg("Done!", Some(meta));
        let formatter = CliFormatter;
        let output = formatter.format_success(&msg);
        assert!(output.contains("✅ Execute | 통과"));
        assert!(output.contains("[🔧 oxios]"));
        assert!(output.contains("1.5s"));
    }

    #[test]
    fn format_success_failed_eval() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: None,
            phase: "Evaluate".into(),
            evaluation_passed: Some(false),
            duration_ms: Some(500),
            error: None,
            interview_questions: None,
            interview_round: None,
        };
        let msg = make_msg("Partial", Some(meta));
        let formatter = CliFormatter;
        let output = formatter.format_success(&msg);
        assert!(output.contains("⚠️ Evaluate | 미통과"));
        assert!(output.contains("500ms"));
    }

    #[test]
    fn format_error_timeout() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: None,
            phase: String::new(),
            evaluation_passed: None,
            duration_ms: None,
            error: Some(UserFacingError {
                message: "시간이 초과되었습니다.".into(),
                kind: ErrorKind::Timeout,
                suggestion: Some("더 간단한 요청으로 시도하세요.".into()),
            }),
            interview_questions: None,
            interview_round: None,
        };
        let msg = make_msg("시간이 초과되었습니다.", Some(meta));
        let formatter = CliFormatter;
        let output = formatter.format_error(&msg);
        assert!(output.starts_with("⏱️"));
        assert!(output.contains("💡 더 간단한 요청으로 시도하세요."));
    }

    #[test]
    fn format_error_provider() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: None,
            phase: String::new(),
            evaluation_passed: None,
            duration_ms: None,
            error: Some(UserFacingError {
                message: "AI 서비스 오류.".into(),
                kind: ErrorKind::ProviderError,
                suggestion: None,
            }),
            interview_questions: None,
            interview_round: None,
        };
        let msg = make_msg("AI 서비스 오류.", Some(meta));
        let formatter = CliFormatter;
        let output = formatter.format_error(&msg);
        assert!(output.starts_with("🔌"));
        assert!(!output.contains("💡")); // no suggestion
    }

    #[test]
    fn format_progress_phases() {
        let formatter = CliFormatter;
        assert_eq!(formatter.format_progress("Interview"), "🔍 분석 중...");
        assert_eq!(formatter.format_progress("Seed"), "📋 계획 수립 중...");
        assert_eq!(formatter.format_progress("Execute"), "⚡ 실행 중...");
        assert_eq!(formatter.format_progress("Evaluate"), "📊 평가 중...");
        assert_eq!(formatter.format_progress("Evolve"), "🔄 개선 중...");
        assert_eq!(formatter.format_progress("Unknown"), "⏳ 처리 중...");
    }
}
