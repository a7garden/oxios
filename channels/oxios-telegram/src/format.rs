//! Telegram response formatter.

use oxios_gateway::format::ChannelFormatter;
use oxios_gateway::message::{ErrorKind, OutgoingMessage};

/// Telegram-specific response formatter.
///
/// Formats outgoing messages for Telegram with Markdown-compatible
/// metadata footer and emoji indicators.
pub struct TelegramFormatter;

impl ChannelFormatter for TelegramFormatter {
    fn format_success(&self, msg: &OutgoingMessage) -> String {
        let mut out = msg.content.clone();

        if let Some(meta) = &msg.meta {
            let mut footer_parts = Vec::new();
            if !meta.phase.is_empty() {
                let eval = if meta.evaluation_passed.unwrap_or(false) {
                    "✅"
                } else {
                    "⚠️"
                };
                footer_parts.push(format!("{} {}", eval, meta.phase));
            }
            if let Some(tag) = &meta.project_tag {
                footer_parts.push(tag.clone());
            }
            if let Some(dur) = meta.duration_ms {
                footer_parts.push(format!("{:.1}s", dur as f64 / 1000.0));
            }
            if !footer_parts.is_empty() {
                out.push_str(&format!("\n\n_{}_", footer_parts.join(" · ")));
            }
        }

        out
    }

    fn format_error(&self, msg: &OutgoingMessage) -> String {
        let meta = msg.meta.as_ref();
        let kind = meta.and_then(|m| m.error.as_ref()).map(|e| e.kind);

        let icon = match kind {
            Some(ErrorKind::ProviderError) => "🔌",
            Some(ErrorKind::Timeout) => "⏱️",
            _ => "❌",
        };

        let mut out = format!("{} {}", icon, msg.content);

        if let Some(err) = meta.and_then(|m| m.error.as_ref()) {
            if let Some(s) = &err.suggestion {
                out.push_str(&format!("\n\n💡 _{s}_"));
            }
        }

        out
    }

    fn format_progress(&self, phase: &str) -> String {
        match phase {
            "Interview" => "🔍 분석 중...",
            "Seed" => "📋 계획 수립 중...",
            "Execute" => "⚡ 실행 중...",
            "Evaluate" => "📊 평가 중...",
            "Evolve" => "🔄 개선 중...",
            _ => "⏳ 처리 중...",
        }
        .into()
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
            channel: "telegram".to_string(),
            user_id: "123".to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            meta,
        }
    }

    #[test]
    fn format_success_no_meta() {
        let msg = make_msg("Hello", None);
        let fmt = TelegramFormatter;
        assert_eq!(fmt.format_success(&msg), "Hello");
    }

    #[test]
    fn format_success_with_phase() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: Some("[🔧 Test]".to_string()),
            seed_id: None,
            phase: "Execute".to_string(),
            evaluation_passed: Some(true),
            duration_ms: Some(3500),
            error: None,
        };
        let msg = make_msg("Done!", Some(meta));
        let fmt = TelegramFormatter;
        let result = fmt.format_success(&msg);
        assert!(result.contains("Done!"));
        assert!(result.contains("✅ Execute"));
        assert!(result.contains("[🔧 Test]"));
        assert!(result.contains("3.5s"));
    }

    #[test]
    fn format_error_internal() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: None,
            seed_id: None,
            phase: String::new(),
            evaluation_passed: None,
            duration_ms: None,
            error: Some(UserFacingError {
                message: "내부 오류".to_string(),
                kind: ErrorKind::Internal,
                suggestion: None,
            }),
        };
        let msg = make_msg("내부 오류", Some(meta));
        let fmt = TelegramFormatter;
        let result = fmt.format_error(&msg);
        assert!(result.starts_with("❌"));
    }

    #[test]
    fn format_error_provider_with_suggestion() {
        let meta = ResponseMeta {
            session_id: None,
            project_id: None,
            project_tag: None,
            seed_id: None,
            phase: String::new(),
            evaluation_passed: None,
            duration_ms: None,
            error: Some(UserFacingError {
                message: "AI 서비스 오류".to_string(),
                kind: ErrorKind::ProviderError,
                suggestion: Some("1-2분 후 다시 시도하세요.".to_string()),
            }),
        };
        let msg = make_msg("AI 서비스 오류", Some(meta));
        let fmt = TelegramFormatter;
        let result = fmt.format_error(&msg);
        assert!(result.starts_with("🔌"));
        assert!(result.contains("💡"));
    }

    #[test]
    fn format_progress_known_phases() {
        let fmt = TelegramFormatter;
        assert_eq!(fmt.format_progress("Interview"), "🔍 분석 중...");
        assert_eq!(fmt.format_progress("Execute"), "⚡ 실행 중...");
        assert_eq!(fmt.format_progress("Evolve"), "🔄 개선 중...");
    }

    #[test]
    fn format_progress_unknown() {
        let fmt = TelegramFormatter;
        assert_eq!(fmt.format_progress("Unknown"), "⏳ 처리 중...");
    }
}
