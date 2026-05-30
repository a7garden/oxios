//! ProcessRunner — executes `oxios run --json` as a subprocess.
//!
//! This is the primary runner for integration and e2e tier tasks.
//! It spawns the actual `oxios` binary, ensuring full end-to-end testing.

use crate::fixture::FixtureManager;
use crate::task::Task;
use crate::runner::Runner;
use crate::{RunOutput, Tier};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::Command;

/// Spawns `oxios run --json` as a subprocess for each task.
pub struct ProcessRunner {
    /// Path to the `oxios` binary.
    oxios_bin: PathBuf,
    /// Workspace root (overrides default).
    workspace_override: Option<PathBuf>,
}

impl ProcessRunner {
    pub fn new(oxios_bin: PathBuf) -> Self {
        Self {
            oxios_bin,
            workspace_override: None,
        }
    }

    /// Set a custom workspace directory.
    pub fn with_workspace(mut self, workspace: PathBuf) -> Self {
        self.workspace_override = Some(workspace);
        self
    }

    /// Execute a single `oxios run --json` command.
    async fn run_single(
        &self,
        prompt: &str,
        session_id: Option<&str>,
        context_file: Option<&Path>,
        workspace: &Path,
        timeout: Duration,
    ) -> anyhow::Result<RunOutput> {
        let mut cmd = Command::new(&self.oxios_bin);
        cmd.arg("run")
            .arg("--json")
            .arg("--exit-code")
            .env("OXIOS_WORKSPACE", workspace);

        if let Some(sid) = session_id {
            cmd.arg("--session").arg(sid);
        }

        if let Some(ctx) = context_file {
            cmd.arg("--context-file").arg(ctx);
        }

        cmd.arg(prompt);

        // Remove default stdout/stderr capture — we need stdout
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let start = Instant::now();
        let output = tokio::time::timeout(timeout, cmd.output()).await??;
        let elapsed = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let mut run_output = RunOutput::from_process_output(&stdout, exit_code, workspace.to_path_buf());
        // Use our measured duration (more accurate than internal one)
        run_output.duration_ms = elapsed;

        Ok(run_output)
    }
}

#[async_trait]
impl Runner for ProcessRunner {
    async fn run(&self, task: &Task) -> anyhow::Result<RunOutput> {
        // Create isolated workspace
        let fixture_mgr = FixtureManager::new(&task.id)?;
        let workspace = fixture_mgr.workspace().to_path_buf();

        // Setup fixtures
        if !task.fixtures.is_empty() {
            fixture_mgr.setup_fixtures(&task.fixtures)?;
        }

        let timeout = Duration::from_secs(task.timeout_secs);

        if task.is_multi_turn() {
            // Multi-turn: chain sessions
            let mut last_output: Option<RunOutput> = None;

            for (i, turn) in task.turns.iter().enumerate() {
                let session_id = last_output
                    .as_ref()
                    .and_then(|o| o.session_id.clone());

                let output = self
                    .run_single(
                        &turn.message,
                        session_id.as_deref(),
                        task.context_file.as_deref(),
                        &workspace,
                        timeout,
                    )
                    .await?;

                tracing::debug!(
                    turn = i + 1,
                    total = task.turns.len(),
                    phase = %output.phase_reached,
                    "Turn completed"
                );

                last_output = Some(output);
            }

            // Return the last turn's output as the canonical output
            let output = last_output.unwrap_or_else(|| RunOutput {
                response: String::new(),
                session_id: None,
                project_id: None,
                project_tag: None,
                seed_id: None,
                agent_id: None,
                phase_reached: "unknown".to_string(),
                evaluation_passed: false,
                exit_code: -1,
                duration_ms: 0,
                workspace: workspace.clone(),
            });

            // For evaluation: merge all turn assertions into the final output context
            // The caller (evaluate_task) handles this
            Ok(output)
        } else {
            // Single-turn
            let prompt = task.prompt.as_deref().unwrap_or("");
            let output = self
                .run_single(
                    prompt,
                    None,
                    task.context_file.as_deref(),
                    &workspace,
                    timeout,
                )
                .await?;

            // Keep workspace alive for post-run filesystem assertions
            let _ = fixture_mgr.persist(); // Don't clean up so assertions can check files
            Ok(output)
        }
    }

    fn name(&self) -> &str {
        "process"
    }

    fn tier(&self) -> Tier {
        Tier::E2e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_runner_creation() {
        let runner = ProcessRunner::new(PathBuf::from("oxios"));
        assert_eq!(runner.name(), "process");
        assert_eq!(runner.tier(), Tier::E2e);
    }
}
