//! CommandHookRunner — SDK HookRunner implementation that executes
//! shell commands for matching hook events.
//!
//! Loads `[[hooks]]` from oxios config and dispatches matching specs
//! when the SDK invokes lifecycle hook events. Fail-open: a script that
//! errors, times out, or exits non-zero (other than 2) never blocks.
use oxicode_sdk::ports::hooks::{HookContext, HookEvent, HookOutcome, HookRunner, HookSpec};
use regex::Regex;
use std::pin::Pin;
use std::time::Duration;

/// Executes shell commands for matching SDK hook events.
#[derive(Debug, Clone)]
pub struct CommandHookRunner {
    specs: Vec<HookSpec>,
    #[allow(dead_code)] // reserved for per-runner default override
    default_timeout: Duration,
}

impl CommandHookRunner {
    /// Create a new runner with the given hook specifications.
    pub fn new(specs: Vec<HookSpec>) -> Self {
        Self {
            specs,
            default_timeout: Duration::from_secs(60),
        }
    }

    /// Find specs matching an event and tool name (case-insensitive regex).
    fn matching_specs(&self, event: HookEvent, tool_name: Option<&str>) -> Vec<&HookSpec> {
        self.specs
            .iter()
            .filter(|spec| {
                if spec.event != event {
                    return false;
                }
                let Some(matcher) = spec.matcher.as_deref() else {
                    return true; // no matcher = matches all
                };
                if matcher.trim().is_empty() {
                    return true;
                }
                let Some(name) = tool_name else {
                    return false; // matcher present but no tool context
                };
                Regex::new(&format!("(?i){}", matcher))
                    .map(|re| re.is_match(name))
                    .unwrap_or_else(|_| name.to_lowercase().contains(&matcher.to_lowercase()))
            })
            .collect()
    }

    /// Execute a single hook command with timeout.
    async fn execute_command(&self, spec: &HookSpec, ctx: &HookContext) -> HookOutcome {
        let timeout = Duration::from_secs(spec.timeout_secs.unwrap_or(60).min(600));

        let result = tokio::time::timeout(timeout, async {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&spec.command)
                .env("OXICODE_TOOL_NAME", ctx.tool_name.as_deref().unwrap_or(""))
                .env("OXICODE_TOOL_INPUT", ctx.tool_args.as_ref().map(|v| v.to_string()).unwrap_or_default())
                .env("OXICODE_SESSION_ID", ctx.session_id.as_deref().unwrap_or(""))
                .env("OXICODE_SESSION_CWD", ctx.session_cwd.as_deref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
        })
        .await;

        match result {
            Ok(Ok(status)) => {
                if status.code() == Some(2) {
                    HookOutcome {
                        block: true,
                        reason: Some(format!(
                            "Hook '{:?}' blocked with exit code 2",
                            spec.event
                        )),
                        override_content: None,
                    }
                } else {
                    HookOutcome::default()
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, event = ?spec.event, "Hook command failed");
                HookOutcome::default()
            }
            Err(_) => {
                tracing::warn!(event = ?spec.event, "Hook command timed out");
                HookOutcome::default()
            }
        }
    }
}

impl HookRunner for CommandHookRunner {
    fn run<'a>(
        &'a self,
        event: HookEvent,
        ctx: &'a HookContext,
    ) -> Pin<Box<dyn Future<Output = HookOutcome> + Send + 'a>> {
        Box::pin(async move {
            let specs = self.matching_specs(event, ctx.tool_name.as_deref());
            if specs.is_empty() {
                return HookOutcome::default();
            }

            let mut combined = HookOutcome::default();
            for spec in specs {
                let outcome = self.execute_command(spec, ctx).await;
                if outcome.block {
                    combined.block = true;
                    combined.reason = outcome.reason;
                }
            }
            combined
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_specs_returns_default() {
        let runner = CommandHookRunner::new(vec![]);
        let ctx = HookContext::default();
        let outcome = runner.run(HookEvent::PreToolUse, &ctx).await;
        assert!(!outcome.block);
    }

    #[tokio::test]
    async fn test_matcher_filtering() {
        let specs = vec![HookSpec {
            event: HookEvent::PreToolUse,
            matcher: Some("Bash|Write".to_string()),
            command: "true".to_string(),
            timeout_secs: Some(5),
        }];
        let runner = CommandHookRunner::new(specs);

        // Matching tool name (case-insensitive)
        let matches = runner.matching_specs(HookEvent::PreToolUse, Some("bash"));
        assert_eq!(matches.len(), 1);
        let matches = runner.matching_specs(HookEvent::PreToolUse, Some("Write"));
        assert_eq!(matches.len(), 1);

        // Non-matching tool name
        let matches = runner.matching_specs(HookEvent::PreToolUse, Some("Read"));
        assert_eq!(matches.len(), 0);

        // Different event
        let matches = runner.matching_specs(HookEvent::PostToolUse, Some("Bash"));
        assert_eq!(matches.len(), 0);

        // No matcher = matches all
        let specs = vec![HookSpec {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "true".to_string(),
            timeout_secs: Some(5),
        }];
        let runner = CommandHookRunner::new(specs);
        let matches = runner.matching_specs(HookEvent::PreToolUse, Some("Anything"));
        assert_eq!(matches.len(), 1);
    }

    #[tokio::test]
    async fn test_hook_command_success() {
        let spec = HookSpec {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "true".to_string(), // Always exits 0
            timeout_secs: Some(5),
        };
        let runner = CommandHookRunner::new(vec![spec]);
        let ctx = HookContext::default();
        let outcome = runner.run(HookEvent::PreToolUse, &ctx).await;
        assert!(!outcome.block);
    }

    #[tokio::test]
    async fn test_hook_command_exit_2_blocks() {
        let spec = HookSpec {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "exit 2".to_string(),
            timeout_secs: Some(5),
        };
        let runner = CommandHookRunner::new(vec![spec]);
        let ctx = HookContext::default();
        let outcome = runner.run(HookEvent::PreToolUse, &ctx).await;
        assert!(outcome.block);
    }

    #[tokio::test]
    async fn test_hook_command_timeout_fails_open() {
        let spec = HookSpec {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "sleep 30".to_string(),
            timeout_secs: Some(1),
        };
        let runner = CommandHookRunner::new(vec![spec]);
        let ctx = HookContext::default();
        let outcome = runner.run(HookEvent::PreToolUse, &ctx).await;
        assert!(!outcome.block); // timed out → fail-open
    }
}