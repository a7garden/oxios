//! `oxios run` subcommand — execute a single prompt through the Ouroboros flow.
//!
//! Supports structured output for programmatic consumption:
//! - `--json` — machine-readable JSON output
//! - `--session <ID>` — continue a multi-turn conversation
//! - `--context-file <PATH>` — prepend file contents to the prompt (`-` for stdin)
//! - `--exit-code` — set process exit code based on evaluation result

use anyhow::Result;
use serde_json::json;
use std::io::Read;

use crate::kernel::Kernel;

/// Options for `oxios run`.
pub struct RunOptions {
    /// Output as JSON instead of human-readable text.
    pub json: bool,
    /// Session ID for multi-turn conversations.
    pub session_id: Option<String>,
    /// File whose contents are prepended to the prompt as context.
    /// Use `-` to read from stdin.
    pub context_file: Option<String>,
    /// Set process exit code based on evaluation result.
    pub exit_code: bool,
}

/// Execute the `oxios run` subcommand.
///
/// Returns the process exit code (0 = success, 1 = evaluation failed).
pub async fn run(kernel: &Kernel, prompt: &str, opts: &RunOptions) -> Result<i32> {
    let start = std::time::Instant::now();

    // ── Build effective prompt ──
    let effective_prompt = build_effective_prompt(prompt, &opts.context_file)?;

    tracing::info!(
        prompt_len = effective_prompt.len(),
        session_id = ?opts.session_id,
        "Processing run command"
    );

    // ── Audit ──
    kernel.handle().security.audit(
        "cli",
        oxi_sdk::AuditAction::Other {
            detail: format!(
                "run: {}",
                effective_prompt.chars().take(100).collect::<String>()
            ),
        },
        "cli-user",
    );

    // ── Execute ──
    let result = kernel
        .execute_prompt_with_session(&effective_prompt, opts.session_id.as_deref())
        .await?;

    let duration_ms = start.elapsed().as_millis() as u64;

    // ── Determine exit code ──
    let exit_code = if opts.exit_code && !result.evaluation_passed {
        1
    } else {
        0
    };

    // ── Output ──
    if opts.json {
        let json_output = json!({
            "response": result.response,
            "session_id": result.session_id,
            "primary_project_id": result.primary_project_id.map(|id| id.to_string()),
            "project_tag": result.project_tag,
            "seed_id": result.seed_id.map(|id| id.to_string()),
            "agent_id": result.agent_id.map(|id| id.to_string()),
            "phase_reached": result.phase_reached.to_string(),
            "evaluation_passed": result.evaluation_passed,
            "exit_code": exit_code,
            "duration_ms": duration_ms,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json_output).unwrap_or_default()
        );
    } else {
        // Human-readable output
        println!("{}", result.response);
        if let Some(ref seed_id) = result.seed_id {
            println!("\nSeed: {seed_id}");
        }
        if let Some(ref session_id) = result.session_id {
            println!("Session: {session_id}");
        }
        if !result.evaluation_passed {
            eprintln!("\n⚠️  Evaluation did not fully pass.");
            if let Some(ref output) = result.output {
                eprintln!("Notes: {output}");
            }
        }
    }

    Ok(exit_code)
}

/// Build the effective prompt by optionally prepending file context.
fn build_effective_prompt(prompt: &str, context_file: &Option<String>) -> Result<String> {
    let Some(ref path) = context_file else {
        return Ok(prompt.to_string());
    };

    let (label, content) = if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        if buf.is_empty() {
            return Err(anyhow::anyhow!("stdin is empty, no context to read"));
        }
        ("stdin".to_string(), buf)
    } else {
        let expanded = oxios_kernel::config::expand_home(path);
        let content = std::fs::read_to_string(&expanded)
            .map_err(|e| anyhow::anyhow!("failed to read context file '{path}': {e}"))?;
        (path.clone(), content)
    };

    Ok(format!(
        "--- Context ({label}) ---\n{content}\n--- End Context ---\n\n{prompt}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_effective_prompt_no_context() {
        let result = build_effective_prompt("hello", &None).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_build_effective_prompt_with_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let result = build_effective_prompt(
            "review this",
            &Some(file_path.to_str().unwrap().to_string()),
        )
        .unwrap();

        assert!(result.contains("--- Context"));
        assert!(result.contains("fn main() {}"));
        assert!(result.contains("--- End Context ---"));
        assert!(result.contains("review this"));
    }
}
