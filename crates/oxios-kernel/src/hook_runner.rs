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
    /// Fallback per-hook timeout when the spec omits `timeout_secs`.
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

    /// Override the fallback per-hook timeout used when a spec omits
    /// `timeout_secs`. Defaults to 60 seconds.
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
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
                Regex::new(&format!("(?i){matcher}"))
                    .map(|re| re.is_match(name))
                    .unwrap_or_else(|_| name.to_lowercase().contains(&matcher.to_lowercase()))
            })
            .collect()
    }

    /// Execute a single hook command with timeout.
    ///
    /// The child process is spawned with `kill_on_drop(true)` and, on unix,
    /// placed in its own process group via `process_group(0)`. On timeout we
    /// explicitly `kill().await` then `wait().await` to reap the zombie — the
    /// `timeout` future dropping alone would leak the descriptor. `sh -c`
    /// descendants are killed together because the whole process group is
    /// signaled.
    async fn execute_command(&self, spec: &HookSpec, ctx: &HookContext) -> HookOutcome {
        let timeout = spec
            .timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(self.default_timeout)
            // Hard cap to avoid pathological configs pinning the runner forever.
            .min(Duration::from_secs(600));

        let mut command = tokio::process::Command::new("sh");
        command
            .arg("-c")
            .arg(&spec.command)
            .env("OXICODE_TOOL_NAME", ctx.tool_name.as_deref().unwrap_or(""))
            .env(
                "OXICODE_TOOL_INPUT",
                ctx.tool_args
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            )
            .env(
                "OXICODE_SESSION_ID",
                ctx.session_id.as_deref().unwrap_or(""),
            )
            .env(
                "OXICODE_SESSION_CWD",
                ctx.session_cwd
                    .as_deref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        // Place the `sh -c` process (and its descendants) in a new process
        // group so we can signal them all together on timeout. macOS and
        // Linux are both unix here; the cfg gates the call for safety on
        // any future non-unix target.
        #[cfg(unix)]
        {
            command.process_group(0);
        }

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, event = ?spec.event, "Hook command failed to spawn");
                return HookOutcome::default();
            }
        };

        let timed_out = match tokio::time::timeout(timeout, child.wait()).await {
            Ok(_status) => false,
            Err(_) => {
                // Timeout: explicitly kill the entire process group and reap
                // the zombie. Falling out of scope alone (kill_on_drop) is
                // not enough — `wait()` must complete to release the pid.
                let _ = child.kill().await;
                let _ = child.wait().await;
                true
            }
        };

        if timed_out {
            tracing::warn!(event = ?spec.event, "Hook command timed out");
            return HookOutcome::default();
        }

        match child.wait().await {
            Ok(status) => {
                if status.code() == Some(2) {
                    HookOutcome {
                        block: true,
                        reason: Some(format!("Hook '{:?}' blocked with exit code 2", spec.event)),
                        override_content: None,
                    }
                } else {
                    HookOutcome::default()
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, event = ?spec.event, "Hook command failed");
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

    #[tokio::test]
    async fn test_hook_command_timeout_kills_child() {
        // Spawn a hook that creates a marker file via `sh -c`, with a tight
        // timeout. After the runner returns, the child process group must be
        // gone — i.e. the `touch` inside the `sleep 30` chain never lands.
        // Regression guard for the "process tree survives the timeout" bug:
        // we need `kill().await` + `wait().await` to actually clean up.
        let marker = std::env::temp_dir().join(format!(
            "oxios-hook-timeout-{}-{}.marker",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        let _ = std::fs::remove_file(&marker);

        // Command: ensure the marker is touched ONLY if the sleep completes
        // (i.e. NOT if we kill it). `&&` makes the touch conditional on sleep.
        let cmd = format!(
            "sleep 30 && touch {}",
            marker.to_string_lossy()
        );
        let spec = HookSpec {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: cmd,
            timeout_secs: Some(1),
        };
        let runner = CommandHookRunner::new(vec![spec]);
        let ctx = HookContext::default();
        let outcome = runner.run(HookEvent::PreToolUse, &ctx).await;
        assert!(!outcome.block, "timeout must fail-open (block=false)");

        // Give the OS a moment to deliver SIGKILL to the process group.
        // 500ms is generous; the runner's own wait().await blocks until
        // the child is reaped.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        assert!(
            !marker.exists(),
            "timed-out hook must not leave its `sleep && touch` chain alive — marker exists: {}",
            marker.display(),
        );
        let _ = std::fs::remove_file(&marker);
    }

    #[tokio::test]
    async fn test_hook_uses_default_timeout_when_spec_omits() {
        // Finding 6 wiring: when `spec.timeout_secs` is None, the runner's
        // `default_timeout` is used. Verify a 0.5s default kicks in.
        let spec = HookSpec {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "sleep 5".to_string(),
            timeout_secs: None, // <-- omit; default_timeout applies
        };
        let runner = CommandHookRunner::new(vec![spec]).with_default_timeout(Duration::from_millis(500));
        let ctx = HookContext::default();
        let start = std::time::Instant::now();
        let outcome = runner.run(HookEvent::PreToolUse, &ctx).await;
        let elapsed = start.elapsed();
        assert!(!outcome.block);
        assert!(
            elapsed < Duration::from_secs(2),
            "default_timeout (500ms) must trigger; got {elapsed:?}",
        );
    }
}
