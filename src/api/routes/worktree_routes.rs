//! Worktree fan-out HTTP routes (RFC-044 Phase 4).
//!
//! Exposes three endpoints that the web UI uses to spin up, diff, and merge
//! short-lived git worktrees for parallel agent exploration:
//!
//! - `POST /api/worktree/fanout` — create N worktrees and fire N parallel
//!   gateway messages, each tagged with the worktree path so the agents can
//!   commit into isolated branches.
//! - `POST /api/worktree/diff` — return per-file + aggregate stats plus the
//!   raw `git diff` text (capped at 64 KiB) for a worktree vs. the merge-base
//!   against a base branch.
//! - `POST /api/worktree/merge` — fast-forward / merge the worktree's branch
//!   into a target branch inside the main worktree, reporting conflicts when
//!   `--no-edit` fails.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::State;
use oxios_gateway::message::IncomingMessage;
use serde::{Deserialize, Serialize};

use crate::api::error::AppError;
use crate::api::server::AppState;

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/// Hard upper bound on the number of parallel worktrees a single fan-out
/// request may create. Keeps the gateway from being swamped by a runaway UI.
const MAX_FANOUT_COUNT: u32 = 8;

/// Lower bound on the number of worktrees a fan-out request must request.
/// Two is the minimum needed to have anything to compare.
const MIN_FANOUT_COUNT: u32 = 2;

/// Cap on the raw diff text returned to the web UI. Anything larger is
/// truncated to keep the JSON response bounded; the per-file stats still
/// reflect the full diff.
const MAX_DIFF_BYTES: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// POST /api/worktree/fanout
// ---------------------------------------------------------------------------

/// Request body for the fan-out endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct FanoutRequest {
    /// The prompt to send to each agent.
    pub prompt: String,
    /// Number of worktrees to spawn. Clamped to `[2, 8]`.
    pub count: u32,
    /// Absolute path to the project repository that owns the worktrees.
    pub project_path: String,
}

/// One entry in the fan-out response — describes a single spawned agent
/// alongside the worktree path it operates in.
#[derive(Debug, Serialize)]
pub(crate) struct FanoutAgentInfo {
    /// Stable identifier for this agent within the group (the worktree
    /// branch name, e.g. `oxios/fanout-2-1700000000`).
    pub agent_id: String,
    /// Human-readable label, e.g. `Agent 1`.
    pub name: String,
    /// Absolute path to the created worktree.
    pub worktree_path: String,
}

/// Response body for the fan-out endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct FanoutResponse {
    /// Group identifier shared by all spawned agents; the web UI uses this
    /// to subscribe to / correlate their responses.
    pub group_id: String,
    /// The agents created by this fan-out.
    pub agents: Vec<FanoutAgentInfo>,
}

/// `POST /api/worktree/fanout` — create `count` git worktrees and inject one
/// `IncomingMessage` per worktree into the gateway.
pub(crate) async fn handle_worktree_fanout(
    state: State<Arc<AppState>>,
    Json(body): Json<FanoutRequest>,
) -> Result<Json<FanoutResponse>, AppError> {
    // Clamp count to the supported window. Reject only if it's strictly less
    // than the floor — anything above the cap is silently truncated.
    if body.count < MIN_FANOUT_COUNT {
        return Err(AppError::BadRequest(format!(
            "count must be at least {MIN_FANOUT_COUNT}, got {}",
            body.count
        )));
    }
    let count = body.count.min(MAX_FANOUT_COUNT);

    let project_path = PathBuf::from(&body.project_path);
    if !project_path.is_dir() {
        return Err(AppError::BadRequest(format!(
            "project_path is not an existing directory: {}",
            project_path.display()
        )));
    }

    let timestamp = unix_timestamp_secs();
    let group_id = format!("fanout-{timestamp}");

    let mut agents = Vec::with_capacity(count as usize);
    for i in 0..count {
        let branch = format!("oxios/fanout-{i}-{timestamp}");
        let worktree_name = format!("fanout-{i}-{timestamp}");

        let worktree_path = create_worktree(&project_path, "HEAD", &worktree_name, &branch)
            .map_err(|e| AppError::Internal(format!("worktree create failed: {e}")))?;

        let mut msg = IncomingMessage::new("web", "fanout", body.prompt.clone());
        msg.metadata
            .insert("worktree_path".to_owned(), worktree_path.clone());
        msg.metadata
            .insert("fanout_index".to_owned(), i.to_string());
        msg.metadata
            .insert("fanout_group".to_owned(), group_id.clone());

        // Fire-and-forget: a full send-and-await would block the request
        // until every agent finishes, which defeats the parallel fan-out.
        if let Err(e) = state.bridge.incoming_tx.send(msg).await {
            tracing::warn!(
                error = %e,
                worktree = %worktree_path,
                "failed to enqueue fanout message into gateway"
            );
            return Err(AppError::Internal(format!(
                "failed to enqueue fanout message: {e}"
            )));
        }

        agents.push(FanoutAgentInfo {
            agent_id: branch,
            name: format!("Agent {}", i + 1),
            worktree_path,
        });
    }

    Ok(Json(FanoutResponse { group_id, agents }))
}

// ---------------------------------------------------------------------------
// POST /api/worktree/diff
// ---------------------------------------------------------------------------

/// Request body for the diff endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct DiffRequest {
    /// Absolute path to the worktree to inspect.
    pub worktree_path: String,
    /// Base branch to diff against. Defaults to `main`.
    #[serde(default)]
    pub base_branch: Option<String>,
}

/// One row of the `--numstat` output.
#[derive(Debug, Serialize)]
pub(crate) struct FileStat {
    /// Repository-relative path of the changed file.
    pub path: String,
    /// Lines added in this file.
    pub insertions: u64,
    /// Lines deleted in this file.
    pub deletions: u64,
}

/// Response body for the diff endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct DiffResponse {
    /// Number of files changed in the diff.
    pub files_changed: usize,
    /// Total lines inserted.
    pub insertions: u64,
    /// Total lines deleted.
    pub deletions: u64,
    /// Per-file insertion / deletion counts (same order as `--numstat`).
    pub files: Vec<FileStat>,
    /// Raw unified diff text, truncated to 64 KiB.
    pub diff_text: String,
}

/// `POST /api/worktree/diff` — compute per-file stats and the raw diff text
/// between the worktree's HEAD and its merge-base with the base branch.
pub(crate) async fn handle_worktree_diff(
    Json(body): Json<DiffRequest>,
) -> Result<Json<DiffResponse>, AppError> {
    let worktree = PathBuf::from(&body.worktree_path);
    if !worktree.is_dir() {
        return Err(AppError::BadRequest(format!(
            "worktree_path is not an existing directory: {}",
            worktree.display()
        )));
    }
    let base_branch = body.base_branch.unwrap_or_else(|| "main".to_owned());

    // Resolve the fork point with `git merge-base`. A non-zero exit means
    // there is no common ancestor (e.g. unmerged branch) — surface a 400.
    let merge_base = run_git_capture(&worktree, &["merge-base", &base_branch, "HEAD"])
        .map_err(|e| AppError::BadRequest(format!("merge-base failed: {e}")))?;
    let merge_base = merge_base.trim().to_owned();
    if merge_base.is_empty() {
        return Err(AppError::BadRequest(format!(
            "no merge-base between {base_branch} and HEAD"
        )));
    }

    // Per-file stats: `<insertions>\t<deletions>\t<path>` per line, with `-`
    // placeholders for binary files.
    let numstat_text = run_git_capture(
        &worktree,
        &["diff", "--numstat", &format!("{merge_base}..HEAD")],
    )
    .map_err(|e| AppError::Internal(format!("git diff --numstat failed: {e}")))?;

    let mut files = Vec::new();
    let mut total_ins: u64 = 0;
    let mut total_del: u64 = 0;
    for line in numstat_text.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let ins_raw = parts.next().unwrap_or("");
        let del_raw = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("").to_owned();
        // Binary files report "-" for both — keep them in the list but do
        // not add them to the line totals.
        let insertions = parse_numstat_cell(ins_raw);
        let deletions = parse_numstat_cell(del_raw);
        total_ins = total_ins.saturating_add(insertions);
        total_del = total_del.saturating_add(deletions);
        files.push(FileStat {
            path,
            insertions,
            deletions,
        });
    }

    let mut diff_text = run_git_capture(&worktree, &["diff", &format!("{merge_base}..HEAD")])
        .map_err(|e| AppError::Internal(format!("git diff failed: {e}")))?;

    if diff_text.len() > MAX_DIFF_BYTES {
        // Truncate at a char boundary so we never slice inside a UTF-8 code
        // point; the JSON layer would otherwise reject the response.
        let mut cut = MAX_DIFF_BYTES;
        while cut > 0 && !diff_text.is_char_boundary(cut) {
            cut -= 1;
        }
        diff_text.truncate(cut);
        diff_text.push_str("\n... [truncated]\n");
    }

    Ok(Json(DiffResponse {
        files_changed: files.len(),
        insertions: total_ins,
        deletions: total_del,
        files,
        diff_text,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/worktree/merge
// ---------------------------------------------------------------------------

/// Request body for the merge endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct MergeRequest {
    /// Absolute path to the worktree whose branch should be merged.
    pub worktree_path: String,
    /// Branch to merge into. Defaults to `main`.
    #[serde(default)]
    pub target_branch: Option<String>,
}

/// Response body for the merge endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct MergeResponse {
    /// `true` when the merge completed without conflicts.
    pub merged: bool,
    /// Conflicting file paths (empty when `merged` is `true`).
    pub conflicts: Vec<String>,
    /// Echo of the branch that was merged into.
    pub target_branch: String,
}

/// `POST /api/worktree/merge` — checkout `target_branch` in the main
/// worktree and merge the worktree's current branch into it with
/// `--no-edit`. Reports conflicts on failure.
pub(crate) async fn handle_worktree_merge(
    Json(body): Json<MergeRequest>,
) -> Result<Json<MergeResponse>, AppError> {
    let worktree = PathBuf::from(&body.worktree_path);
    if !worktree.is_dir() {
        return Err(AppError::BadRequest(format!(
            "worktree_path is not an existing directory: {}",
            worktree.display()
        )));
    }
    let target_branch = body.target_branch.unwrap_or_else(|| "main".to_owned());

    // The branch to merge is whatever the worktree is currently on. We need
    // its name to invoke `git merge <branch>` after checking out the target.
    let source_branch = run_git_capture(&worktree, &["branch", "--show-current"])
        .map_err(|e| AppError::Internal(format!("git branch --show-current failed: {e}")))?;
    let source_branch = source_branch.trim().to_owned();
    if source_branch.is_empty() {
        return Err(AppError::BadRequest(
            "worktree is in detached HEAD state; nothing to merge".into(),
        ));
    }

    // The merge must happen inside the main worktree (the one whose branch
    // is `target_branch`). `git rev-parse --show-toplevel` returns the
    // repository root which, in single-worktree repos, is the main
    // worktree; in multi-worktree repos it returns the path of whichever
    // worktree the command runs in, which is the right one for us.
    let repo_root = run_git_capture(&worktree, &["rev-parse", "--show-toplevel"])
        .map_err(|e| AppError::Internal(format!("git rev-parse --show-toplevel failed: {e}")))?;
    let repo_root = PathBuf::from(repo_root.trim());

    // Switch to the target branch. We do not abort the existing work if
    // this fails — that is a normal client-visible failure mode (e.g. dirty
    // working tree) and the error message is the most useful response.
    run_git(&repo_root, &["checkout", &target_branch])
        .map_err(|e| AppError::Conflict(format!("checkout {target_branch} failed: {e}")))?;

    let merge_result = run_git(&repo_root, &["merge", "--no-edit", &source_branch]);

    match merge_result {
        Ok(()) => Ok(Json(MergeResponse {
            merged: true,
            conflicts: Vec::new(),
            target_branch,
        })),
        Err(merge_err) => {
            // Collect conflicting files so the UI can highlight them. We
            // keep going past this point — listing conflicts is independent
            // of whether the merge can still succeed (e.g. the caller can
            // resolve them and retry).
            let conflicts =
                run_git_capture(&repo_root, &["diff", "--name-only", "--diff-filter=U"])
                    .map(|text| {
                        text.lines()
                            .map(str::to_owned)
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();

            tracing::warn!(
                error = %merge_err,
                source = %source_branch,
                target = %target_branch,
                "worktree merge reported conflicts"
            );

            Ok(Json(MergeResponse {
                merged: false,
                conflicts,
                target_branch,
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

/// Run `git <args>` inside `dir`, returning an error string on non-zero exit.
///
/// All commands in this module go through this helper so error formatting is
/// consistent — the caller maps the error into the appropriate `AppError`
/// variant (400 vs 500 vs 409).
fn run_git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(())
}

/// Run `git <args>` inside `dir` and return the trimmed stdout.
fn run_git_capture(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Create a fresh git worktree at `<repo>/.oxios-worktrees/<name>` on a new
/// branch `oxios/<name>` based off `base_ref` (e.g. `"HEAD"`).
fn create_worktree(
    repo_path: &Path,
    base_ref: &str,
    name: &str,
    branch: &str,
) -> Result<String, String> {
    let wt_dir = repo_path.join(".oxios-worktrees");
    std::fs::create_dir_all(&wt_dir).map_err(|e| format!("mkdir worktrees: {e}"))?;
    let wt_path = wt_dir.join(name);

    let output = Command::new("git")
        .args(["worktree", "add", "-b", branch])
        .arg(&wt_path)
        .arg(base_ref)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(wt_path.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

/// Parse a single `--numstat` cell. Returns 0 for binary files (`-`) and
/// the integer value otherwise.
fn parse_numstat_cell(cell: &str) -> u64 {
    cell.parse::<u64>().unwrap_or(0)
}

/// Wall-clock seconds since the Unix epoch. Used to make branch / group
/// identifiers unique across repeated fan-out invocations.
fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
