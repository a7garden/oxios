//! Agent tool for authoring, validating, and packaging skills.
//!
//! Wraps the [`ExtensionApi`] / [`SkillManager`] domain of the [`KernelHandle`].
//! Mirrors Anthropic's `skill-creator` model split along the knowledge/capability
//! seam: this tool is the *capability* (deterministic create/validate/package),
//! the bundled `skill-creator` skill is the *methodology* (how to write a good
//! skill). The tool is self-discoverable — its description carries enough
//! authoring guidance that an agent can produce well-formed skills even when the
//! methodology skill is not loaded (e.g. in an installed binary before default
//! skills are embedded).
//!
//! ## Actions
//!
//! | Action    | Description                                  | Required params       | Optional params |
//! |-----------|----------------------------------------------|-----------------------|-----------------|
//! | `list`    | List all installed skills                    | —                     | —               |
//! | `get`     | Get a skill's full content + metadata        | `name`                | —               |
//! | `create`  | Create a skill (synthesized frontmatter)     | `name`, `description` | `content`       |
//! | `write`   | Write raw `SKILL.md` (rich frontmatter kept) | `name`, `content`     | —               |
//! | `validate`| Validate a skill's structure                 | —                     | `name`, `content` |
//! | `package` | Package a skill into a `.skill` zip          | `name`                | —               |
//! | `import`  | Import a skill from raw `SKILL.md` text      | `content`             | `name`          |
//! | `delete`  | Delete a skill                               | `name`                | —               |
//! | `enable`  | Enable a skill                               | `name`                | —               |
//! | `disable` | Disable a skill                              | `name`                | —               |

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::kernel_handle::KernelHandle;
use crate::skill::SkillManager;
use crate::skill::frontmatter::parse_skill;

/// Agent tool for skill authoring, validation, and packaging.
///
/// Holds an [`Arc<SkillManager>`] cloned from the [`KernelHandle`] extensions
/// facade. All mutations go through the manager so the in-memory index and the
/// on-disk tree stay consistent.
pub struct SkillForgeTool {
    skill_manager: std::sync::Arc<SkillManager>,
}

impl SkillForgeTool {
    /// Build a [`SkillForgeTool`] from a [`KernelHandle`].
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            skill_manager: kernel.extensions.skill_manager().clone(),
        }
    }
}

impl std::fmt::Debug for SkillForgeTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillForgeTool").finish()
    }
}

#[async_trait]
impl AgentTool for SkillForgeTool {
    fn name(&self) -> &str {
        "skill_forge"
    }

    fn label(&self) -> &str {
        "Skill Forge"
    }

    fn description(&self) -> &'static str {
        "Create, validate, package, import, and manage skills. \
         A skill is a folder with a SKILL.md file: YAML frontmatter with `name` and `description` \
         (the description is the PRIMARY trigger — write it to fire when the user wants this \
         capability, and make it a little pushy), followed by markdown instructions. \
         Skills use progressive disclosure: (1) name+description always in context, (2) the SKILL.md \
         body loaded when the skill triggers (keep under ~500 lines), (3) optional bundled \
         resources — `scripts/`, `references/`, `assets/` — read on demand via their absolute path. \
         Use `create` to scaffold a new skill, `write` to author rich SKILL.md content, `validate` \
         to check structure before shipping, and `package` to export a distributable `.skill` zip. \
         Use this tool whenever the user wants to build, edit, test, package, or ship a skill."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "create", "write", "validate", "package",
                             "import", "delete", "enable", "disable", "benchmark", "view"],
                    "description": "Skill operation. Authoring: create/write/validate/package/import/list/get/delete/enable/disable. Eval (deterministic post-processing): `benchmark` aggregates grading.json files in an eval workspace iteration into benchmark.json+benchmark.md (mean±stddev, delta); `view` generates a static self-contained HTML viewer of an eval iteration."
                },
                "name": {
                    "type": "string",
                    "description": "Skill name (lowercase, hyphens). Required for get/create/write/validate/package/delete/enable/disable. For `import`, an optional hint when frontmatter has no name."
                },
                "description": {
                    "type": "string",
                    "description": "One-line description of when the skill should trigger (create action)"
                },
                "content": {
                    "type": "string",
                    "description": "create: the markdown body (frontmatter synthesized for you). write/import: the FULL raw SKILL.md including frontmatter. validate: full raw SKILL.md when `name` is omitted."
                },
                "workspace": {
                    "type": "string",
                    "description": "benchmark/view: absolute path to an eval workspace ITERATION directory (contains per-eval subdirs each with with_skill/ and a baseline run holding grading.json + timing.json)."
                },
                "skill_name": {
                    "type": "string",
                    "description": "benchmark/view: skill name for labeling the report (optional)."
                },
                "output": {
                    "type": "string",
                    "description": "view: where to write the static HTML file (optional; defaults to <workspace>/review.html)."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;
        let name = params.get("name").and_then(|v| v.as_str());
        let description = params.get("description").and_then(|v| v.as_str());
        let content = params.get("content").and_then(|v| v.as_str());
        let workspace = params.get("workspace").and_then(|v| v.as_str());
        let skill_name = params.get("skill_name").and_then(|v| v.as_str());
        let output = params.get("output").and_then(|v| v.as_str());

        match action {
            "list" => self.act_list().await,
            "get" => {
                let name = name.ok_or("'get' requires 'name'")?;
                self.act_get(name).await
            }
            "create" => {
                let name = name.ok_or("'create' requires 'name'")?;
                let description = description.unwrap_or("");
                let content = content.unwrap_or("");
                self.act_create(name, description, content).await
            }
            "write" => {
                let name = name.ok_or("'write' requires 'name'")?;
                let content = content.ok_or("'write' requires 'content' (full raw SKILL.md)")?;
                self.act_write(name, content).await
            }
            "validate" => self.act_validate(name, content).await,
            "package" => {
                let name = name.ok_or("'package' requires 'name'")?;
                self.act_package(name).await
            }
            "import" => {
                let content = content.ok_or("'import' requires 'content' (raw SKILL.md text)")?;
                self.act_import(content, name).await
            }
            "delete" => {
                let name = name.ok_or("'delete' requires 'name'")?;
                self.act_delete(name).await
            }
            "enable" => {
                let name = name.ok_or("'enable' requires 'name'")?;
                self.act_set_enabled(name, true).await
            }
            "disable" => {
                let name = name.ok_or("'disable' requires 'name'")?;
                self.act_set_enabled(name, false).await
            }
            "benchmark" => {
                let workspace =
                    workspace.ok_or("'benchmark' requires 'workspace' (iteration dir)")?;
                self.act_benchmark(workspace, skill_name).await
            }
            "view" => {
                let workspace = workspace.ok_or("'view' requires 'workspace' (iteration dir)")?;
                self.act_view(workspace, skill_name, output).await
            }
            other => Ok(AgentToolResult::error(format!("Unknown action: {other}"))),
        }
    }
}

impl SkillForgeTool {
    async fn act_list(&self) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let entries = self.skill_manager.list_skills().await;
        let rows: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "name": e.skill.name,
                    "description": e.skill.description,
                    "status": e.status.to_string(),
                    "format": e.format.to_string(),
                    "bundled": e.bundled,
                })
            })
            .collect();
        Ok(AgentToolResult::success(
            serde_json::to_string_pretty(&json!({ "skills": rows, "count": rows.len() }))
                .unwrap_or_default(),
        ))
    }

    async fn act_get(&self, name: &str) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        match self.skill_manager.get_skill(name).await {
            Some(e) => {
                let meta = e.metadata.as_ref();
                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&json!({
                        "name": e.skill.name,
                        "description": e.skill.description,
                        "status": e.status.to_string(),
                        "format": e.format.to_string(),
                        "bundled": e.bundled,
                        "skill_dir": e.skill.path.parent().map(|p| p.display().to_string()),
                        "file_path": e.skill.file_path.display().to_string(),
                        "author": meta.and_then(|m| m.author.clone()),
                        "version": meta.and_then(|m| m.version.clone()),
                        "content": e.skill.content,
                    }))
                    .unwrap_or_default(),
                ))
            }
            None => Ok(AgentToolResult::error(format!("Skill not found: {name}"))),
        }
    }

    async fn act_create(
        &self,
        name: &str,
        description: &str,
        content: &str,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        // Pre-validate the proposed shape so agents get fast feedback.
        let synthetic = format!("---\nname: {name}\ndescription: {description}\n---\n\n{content}");
        let report = validate_skill_content(&synthetic, Some(name));
        if report.has_errors() {
            return Ok(AgentToolResult::error(format!(
                "Refusing to create skill with structural errors:\n{}",
                report.render()
            )));
        }
        match self
            .skill_manager
            .create_skill(name, description, content)
            .await
        {
            Ok(()) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "name": name,
                    "path": self.skill_dir_of(name).await,
                    "warnings": report.warnings(),
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Create failed: {e}"))),
        }
    }

    async fn act_write(
        &self,
        name: &str,
        content: &str,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let report = validate_skill_content(content, Some(name));
        if report.has_errors() {
            return Ok(AgentToolResult::error(format!(
                "Refusing to write skill with structural errors:\n{}",
                report.render()
            )));
        }
        match self.skill_manager.write_skill_raw(name, content).await {
            Ok(entry) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "name": entry.skill.name,
                    "description": entry.skill.description,
                    "path": entry.skill.file_path.display().to_string(),
                    "warnings": report.warnings(),
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Write failed: {e}"))),
        }
    }

    async fn act_validate(
        &self,
        name: Option<&str>,
        content: Option<&str>,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let report = match (name, content) {
            (Some(n), Some(c)) => {
                // Prefer the on-disk skill if it exists; fall back to supplied content.
                match self.skill_manager.get_skill(n).await {
                    Some(e) => {
                        let raw = read_skill_md(&e.skill.file_path);
                        validate_skill_content(&raw, Some(n))
                    }
                    None => validate_skill_content(c, Some(n)),
                }
            }
            (Some(n), None) => {
                let e = self.skill_manager.get_skill(n).await.ok_or_else(|| {
                    format!("Skill not found: {n} (pass `content` to validate raw text)")
                })?;
                let raw = read_skill_md(&e.skill.file_path);
                validate_skill_content(&raw, Some(n))
            }
            (None, Some(c)) => validate_skill_content(c, None),
            (None, None) => {
                return Ok(AgentToolResult::error(
                    "validate requires `name` (of an installed skill) or `content` (raw SKILL.md)",
                ));
            }
        };
        Ok(AgentToolResult::success(
            serde_json::to_string_pretty(&report.to_json()).unwrap_or_default(),
        ))
    }

    async fn act_package(&self, name: &str) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let entry = self
            .skill_manager
            .get_skill(name)
            .await
            .ok_or_else(|| format!("Skill not found: {name}"))?;
        let skill_dir = entry
            .skill
            .path
            .parent()
            .ok_or("skill has no directory")?
            .to_path_buf();
        let out = self.skill_manager.path().join(format!("{name}.skill"));
        match package_skill_dir(&skill_dir, name, &out) {
            Ok(path) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "name": name,
                    "archive": path.display().to_string(),
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Package failed: {e}"))),
        }
    }

    async fn act_import(
        &self,
        content: &str,
        name_hint: Option<&str>,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        match self
            .skill_manager
            .import_skill_text(content, name_hint)
            .await
        {
            Ok(entry) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "name": entry.skill.name,
                    "description": entry.skill.description,
                    "path": entry.skill.file_path.display().to_string(),
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Import failed: {e}"))),
        }
    }

    async fn act_delete(&self, name: &str) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        match self.skill_manager.delete_skill(name).await {
            Ok(()) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({ "ok": true, "deleted": name }))
                    .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Delete failed: {e}"))),
        }
    }

    async fn act_set_enabled(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let op = if enabled { "enable" } else { "disable" };
        match self.skill_manager.set_enabled(name, enabled).await {
            Ok(()) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "ok": true, "name": name, "enabled": enabled
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("{op} failed: {e}"))),
        }
    }

    async fn skill_dir_of(&self, name: &str) -> String {
        self.skill_manager
            .get_skill(name)
            .await
            .and_then(|e| e.skill.path.parent().map(|p| p.display().to_string()))
            .unwrap_or_default()
    }
    async fn act_benchmark(
        &self,
        workspace: &str,
        skill_name: Option<&str>,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let name = skill_name.unwrap_or("skill");
        match aggregate_benchmark(Path::new(workspace), name) {
            Ok(bench) => {
                let json_path = Path::new(workspace).join("benchmark.json");
                let md_path = Path::new(workspace).join("benchmark.md");
                let json_err = std::fs::write(&json_path, bench.to_json_string()).err();
                let md_err = std::fs::write(&md_path, bench.to_markdown()).err();
                if let Some(e) = json_err.or(md_err) {
                    return Ok(AgentToolResult::error(format!(
                        "Wrote partial benchmark but failed to persist: {e}"
                    )));
                }
                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "benchmark_json": json_path.display().to_string(),
                        "benchmark_md": md_path.display().to_string(),
                        "configs": bench.config_names(),
                        "evals": bench.metadata.eval_count,
                        "delta_pass_rate": bench.delta.pass_rate,
                    }))
                    .unwrap_or_default(),
                ))
            }
            Err(e) => Ok(AgentToolResult::error(format!("Benchmark failed: {e}"))),
        }
    }

    async fn act_view(
        &self,
        workspace: &str,
        skill_name: Option<&str>,
        output: Option<&str>,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let name = skill_name.unwrap_or("skill");
        let out = output
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(workspace).join("review.html"));
        match generate_review_html(Path::new(workspace), name, &out) {
            Ok(path) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "html": path.display().to_string(),
                    "note": "Open this file in a browser. Use the Outputs tab to review each eval and leave feedback; the Benchmark tab shows the quantitative comparison."
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("View failed: {e}"))),
        }
    }
}

fn read_skill_md(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

// ─── Validation (port of Anthropic quick_validate) ──────────────────────────

/// One structural finding from validation.
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Blocks create/write — the skill is malformed.
    Error,
    /// Worth fixing before shipping — progressive-disclosure guidance, etc.
    Warning,
}

impl Finding {
    fn warn(msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: msg.into(),
        }
    }
    fn error(msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: msg.into(),
        }
    }
}

/// Result of validating a skill's structure.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
    pub name: String,
    pub description: String,
    pub body_lines: usize,
    pub format: String,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    pub fn warnings(&self) -> Vec<String> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .map(|f| f.message.clone())
            .collect()
    }

    pub fn render(&self) -> String {
        self.findings
            .iter()
            .map(|f| {
                let lvl = match f.severity {
                    Severity::Error => "ERROR",
                    Severity::Warning => "WARN",
                };
                format!("  [{lvl}] {}", f.message)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_json(&self) -> Value {
        json!({
            "valid": !self.has_errors(),
            "name": self.name,
            "description_chars": self.description.chars().count(),
            "body_lines": self.body_lines,
            "format": self.format,
            "findings": self.findings.iter().map(|f| {
                let lvl = match f.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                };
                json!({ "severity": lvl, "message": f.message })
            }).collect::<Vec<_>>(),
        })
    }
}

/// Validate a raw `SKILL.md` (frontmatter + body).
///
/// `dir_name` is the expected skill directory name; when supplied, a mismatch
/// with the frontmatter `name` is flagged (matters for packaged `.skill`
/// archives where the top-level folder must equal the skill name).
pub fn validate_skill_content(content: &str, dir_name: Option<&str>) -> ValidationReport {
    let mut findings = Vec::new();

    // Frontmatter presence.
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        findings.push(Finding::error(
            "Missing YAML frontmatter (must start with `---`).",
        ));
        return ValidationReport {
            findings,
            ..Default::default()
        };
    }

    // Parse via the shared pipeline — reuses format detection + body sanitization.
    let (parsed, body) = match parse_skill(content, &PathBuf::new()) {
        Ok(v) => v,
        Err(e) => {
            findings.push(Finding::error(format!("Frontmatter does not parse: {e}")));
            return ValidationReport {
                findings,
                ..Default::default()
            };
        }
    };

    let name = parsed.name.trim().to_string();
    let description = parsed.description.trim().to_string();
    let body_lines = body.lines().count();

    // name checks.
    if name.is_empty() {
        findings.push(Finding::error("Frontmatter is missing `name`."));
    } else {
        if !is_valid_skill_name(&name) {
            findings.push(Finding::error(format!(
                "Skill name `{name}` is invalid — use lowercase ascii letters, digits, and hyphens only."
            )));
        }
        if let Some(dir) = dir_name
            && dir != name
        {
            findings.push(Finding::error(format!(
                "Frontmatter name `{name}` does not match the skill directory `{dir}`. They must be identical for a packaged skill."
            )));
        }
    }

    // description checks (the primary trigger).
    if description.is_empty() {
        findings.push(Finding::error(
            "Frontmatter is missing `description` — this is the PRIMARY trigger and must state when to use the skill.",
        ));
    } else if description.chars().count() < 15 {
        findings.push(Finding::warn(
            "Description is very short. It is the primary trigger — describe both what the skill does AND when to use it. A little pushy is good.",
        ));
    }

    // body checks (progressive disclosure level 2).
    if body.trim().is_empty() {
        findings.push(Finding::error(
            "SKILL.md body is empty — the skill has no instructions.",
        ));
    } else if body_lines > 500 {
        findings.push(Finding::warn(format!(
            "Body is {body_lines} lines. Progressive disclosure aims for <500; consider moving detail into `references/` and pointing to it."
        )));
    }

    ValidationReport {
        findings,
        name,
        description,
        body_lines,
        format: parsed.format.to_string(),
    }
}

/// A skill name is lowercase ascii letters, digits, and hyphens; non-empty;
/// no leading/trailing hyphen.
pub fn is_valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// ─── Packaging (port of Anthropic package_skill) ────────────────────────────

/// Patterns excluded everywhere in a packaged skill tree.
const EXCLUDE_DIRS: &[&str] = &["__pycache__", "node_modules", ".git"];
const EXCLUDE_FILES: &[&str] = &[".DS_Store"];
/// Directories excluded only at the skill root.
const ROOT_EXCLUDE_DIRS: &[&str] = &["evals"];

/// Package `skill_dir` into a `.skill` zip at `out`, with the skill folder as
/// the top-level entry (so extraction lands at `<name>/...`).
///
/// Excludes build artifacts (`__pycache__`, `node_modules`, `.git`, `.pyc`,
/// `.DS_Store`) and the root-level `evals/` directory (test cases are not
/// shipped in the distributable archive).
pub fn package_skill_dir(
    skill_dir: &Path,
    skill_name: &str,
    out: &Path,
) -> anyhow::Result<PathBuf> {
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    anyhow::ensure!(
        skill_dir.is_dir(),
        "skill directory not found: {}",
        skill_dir.display()
    );
    anyhow::ensure!(
        skill_dir.join("SKILL.md").exists(),
        "SKILL.md not found in {}",
        skill_dir.display()
    );

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(out)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut stack: Vec<(PathBuf, String)> = vec![(skill_dir.to_path_buf(), skill_name.to_string())];
    while let Some((dir, prefix)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "skipping unreadable dir");
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // Use symlink_metadata so we DON'T follow symlinks — a malicious
            // skill could otherwise symlink to ~/.ssh/id_rsa etc. and have its
            // contents embedded in the .skill archive (information disclosure).
            let file_type = match fs::symlink_metadata(&path) {
                Ok(m) => m.file_type(),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping unreadable entry");
                    continue;
                }
            };
            if file_type.is_symlink() {
                tracing::warn!(path = %path.display(), "skipping symlink in skill tree");
                continue;
            }

            // Root-only excludes: compare against the top-level prefix depth.
            let is_root = prefix == skill_name;
            if file_type.is_dir() {
                if EXCLUDE_DIRS.contains(&file_name.as_str()) {
                    continue;
                }
                if is_root && ROOT_EXCLUDE_DIRS.contains(&file_name.as_str()) {
                    continue;
                }
                stack.push((path, format!("{prefix}/{file_name}")));
            } else if file_type.is_file() {
                if EXCLUDE_FILES.contains(&file_name.as_str()) {
                    continue;
                }
                if file_name.ends_with(".pyc") {
                    continue;
                }
                let arcname = format!("{prefix}/{file_name}");
                if let Err(e) = add_file_to_zip(&mut zip, &path, &arcname, options) {
                    tracing::warn!(file = %arcname, error = %e, "skipping file");
                }
            }
        }
    }

    zip.finish()?;
    Ok(out.to_path_buf())
}

fn add_file_to_zip(
    zip: &mut zip::ZipWriter<fs::File>,
    path: &Path,
    arcname: &str,
    options: zip::write::SimpleFileOptions,
) -> io::Result<()> {
    let mut f = fs::File::open(path)?;
    zip.start_file(arcname, options)?;
    io::copy(&mut f, zip)?;
    Ok(())
}

// silence unused import when `Write`/`Read` paths are pruned by the compiler
#[allow(dead_code)]
fn _ensure_write_in_scope() -> Option<Box<dyn Write>> {
    None
}
// ─── Eval harness (port of Anthropic aggregate_benchmark + generate_review) ──

/// Aggregate pass-rate / time / token stats for the configurations found in an
/// eval workspace iteration directory.
///
/// Layout expected (per `skill-creator` SKILL.md):
/// ```text
/// <iteration-dir>/
///   <eval-name>/
///     with_skill/{grading.json, timing.json, outputs/...}
///     without_skill/{grading.json, timing.json, outputs/...}   # or old_skill/
///     eval_metadata.json
/// ```
/// `with_skill` is the treatment; the first of `without_skill` / `old_skill`
/// that exists is the baseline. Missing files are skipped (the eval is still
/// counted, but contributes no data to that configuration).
pub fn aggregate_benchmark(
    iteration_dir: &Path,
    skill_name: &str,
) -> anyhow::Result<BenchmarkData> {
    use std::collections::HashMap;

    let mut runs: Vec<BenchmarkRun> = Vec::new();
    // per-config metric vectors
    let mut buckets: HashMap<String, MetricVec> = HashMap::new();

    let eval_dirs = list_eval_dirs(iteration_dir)?;
    for (eval_name, eval_dir) in &eval_dirs {
        for cfg in ["with_skill", "without_skill", "old_skill"] {
            let run_dir = eval_dir.join(cfg);
            if !run_dir.join("grading.json").exists() {
                continue;
            }
            let grading = read_grading(&run_dir.join("grading.json"));
            let timing = read_timing(&run_dir.join("timing.json"));
            let pass_rate = grading
                .as_ref()
                .and_then(|g| g.summary.as_ref())
                .map(|s| s.pass_rate)
                .unwrap_or(0.0);
            let time_seconds = timing.duration_seconds();
            let tokens = timing.total_tokens.unwrap_or(0.0);

            // Normalize the baseline label: old_skill → without_skill for the summary.
            let label = if cfg == "old_skill" {
                "without_skill"
            } else {
                cfg
            };
            let mv = buckets.entry(label.to_string()).or_default();
            mv.pass_rate.push(pass_rate);
            mv.time_seconds.push(time_seconds);
            mv.tokens.push(tokens);

            let expectations = grading
                .as_ref()
                .map(|g| g.expectations.clone())
                .unwrap_or_default();
            runs.push(BenchmarkRun {
                eval_name: eval_name.clone(),
                configuration: label.to_string(),
                pass_rate,
                time_seconds,
                tokens,
                expectations,
            });
        }
    }

    if runs.is_empty() {
        anyhow::bail!(
            "no grading.json files found under {}. Run the eval cases (with_skill + baseline) first; \
             each run dir must contain a grading.json (see references/schemas.md).",
            iteration_dir.display()
        );
    }

    let mut run_summary: HashMap<String, ConfigSummary> = HashMap::new();
    for (label, mv) in &buckets {
        run_summary.insert(
            label.clone(),
            ConfigSummary {
                pass_rate: stats(&mv.pass_rate),
                time_seconds: stats(&mv.time_seconds),
                tokens: stats(&mv.tokens),
            },
        );
    }

    let delta = {
        let with = run_summary.get("with_skill");
        let without = run_summary.get("without_skill");
        DeltaSummary {
            pass_rate: diff(
                with.map(|c| c.pass_rate.mean),
                without.map(|c| c.pass_rate.mean),
            ),
            time_seconds: diff(
                with.map(|c| c.time_seconds.mean),
                without.map(|c| c.time_seconds.mean),
            ),
            tokens: diff(with.map(|c| c.tokens.mean), without.map(|c| c.tokens.mean)),
        }
    };

    let mut notes = Vec::new();
    // Non-discriminating assertion heuristic: passes 100% in BOTH configs.
    analyze_results(&runs, &mut notes);

    Ok(BenchmarkData {
        metadata: BenchmarkMeta {
            skill_name: skill_name.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            eval_count: eval_dirs.len(),
            configs: buckets.keys().cloned().collect(),
        },
        runs,
        run_summary,
        delta,
        notes,
    })
}

#[derive(Default)]
struct MetricVec {
    pass_rate: Vec<f64>,
    time_seconds: Vec<f64>,
    tokens: Vec<f64>,
}

/// Mean / sample-stddev / min / max over a slice of f64.
fn stats(xs: &[f64]) -> ConfigStats {
    if xs.is_empty() {
        return ConfigStats {
            mean: 0.0,
            stddev: 0.0,
            min: 0.0,
            max: 0.0,
        };
    }
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    let variance = if xs.len() > 1 {
        xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64
    } else {
        0.0
    };
    ConfigStats {
        mean,
        stddev: variance.sqrt(),
        min: xs.iter().cloned().fold(f64::INFINITY, f64::min),
        max: xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Signed difference `a - b` formatted to a string, or "n/a" if either side is missing.
fn diff(a: Option<f64>, b: Option<f64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) => format!("{:+.2}", a - b),
        _ => "n/a".to_string(),
    }
}

/// Surface a few analyst observations hidden by the aggregate stats.
fn analyze_results(runs: &[BenchmarkRun], notes: &mut Vec<String>) {
    use std::collections::HashMap;
    // Per-eval pass_rate grouped by eval → detect 100%-in-both (non-discriminating).
    let mut by_eval: HashMap<String, Vec<(&str, f64)>> = HashMap::new();
    for r in runs {
        by_eval
            .entry(r.eval_name.clone())
            .or_default()
            .push((r.configuration.as_str(), r.pass_rate));
    }
    for (eval, cfgs) in &by_eval {
        let all_full = cfgs.iter().all(|(_, p)| (*p - 1.0).abs() < 1e-9);
        let has_multiple = cfgs.len() > 1;
        if all_full && has_multiple {
            notes.push(format!(
                "Eval '{eval}' passes 100% in every configuration — the assertions may not discriminate skill value."
            ));
        }
        if has_multiple {
            let vals: Vec<f64> = cfgs.iter().map(|(_, p)| *p).collect();
            let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let spread = hi - lo;
            if spread > 0.4 {
                notes.push(format!(
                    "Eval '{eval}' shows large pass-rate spread ({spread:.2}) across configurations — inspect for flakiness."
                ));
            }
        }
    }
}

fn list_eval_dirs(iteration_dir: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    let entries = fs::read_dir(iteration_dir)
        .with_context(|| format!("reading iteration dir {}", iteration_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // An eval dir contains at least one run subdir with grading.json.
        let is_eval = ["with_skill", "without_skill", "old_skill"]
            .iter()
            .any(|c| path.join(c).join("grading.json").exists());
        if is_eval {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("eval")
                .to_string();
            out.push((name, path));
        }
    }
    // Stable order by name.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn read_grading(path: &Path) -> Option<GradingFile> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn read_timing(path: &Path) -> TimingFile {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

// ── eval data types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct GradingSummary {
    #[serde(default)]
    pass_rate: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct GradingFile {
    #[serde(default)]
    summary: Option<GradingSummary>,
    #[serde(default)]
    expectations: Vec<ExpectationRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectationRow {
    #[serde(default)]
    text: String,
    #[serde(default)]
    passed: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TimingFile {
    #[serde(default)]
    total_tokens: Option<f64>,
    #[serde(default)]
    total_duration_seconds: Option<f64>,
    #[serde(default)]
    duration_ms: Option<f64>,
}

impl TimingFile {
    fn duration_seconds(&self) -> f64 {
        self.total_duration_seconds
            .or_else(|| self.duration_ms.map(|ms| ms / 1000.0))
            .unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkRun {
    pub eval_name: String,
    pub configuration: String,
    pub pass_rate: f64,
    pub time_seconds: f64,
    pub tokens: f64,
    pub expectations: Vec<ExpectationRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigStats {
    pub mean: f64,
    pub stddev: f64,
    pub min: f64,
    pub max: f64,
}

/// Serialized as a nested object: `{ mean, stddev, min, max }`.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSummary {
    pub pass_rate: ConfigStats,
    pub time_seconds: ConfigStats,
    pub tokens: ConfigStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeltaSummary {
    pub pass_rate: String,
    pub time_seconds: String,
    pub tokens: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkMeta {
    pub skill_name: String,
    pub timestamp: String,
    pub eval_count: usize,
    pub configs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkData {
    pub metadata: BenchmarkMeta,
    pub runs: Vec<BenchmarkRun>,
    pub run_summary: std::collections::HashMap<String, ConfigSummary>,
    pub delta: DeltaSummary,
    pub notes: Vec<String>,
}

impl BenchmarkData {
    pub fn config_names(&self) -> Vec<String> {
        self.metadata.configs.clone()
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Benchmark — {}\n\n", self.metadata.skill_name));
        md.push_str(&format!(
            "_{} evals · {} · configs: {}_\n\n",
            self.metadata.eval_count,
            self.metadata.timestamp,
            self.metadata.configs.join(", ")
        ));
        md.push_str("| config | pass_rate (mean ± σ) | time_s (mean ± σ) | tokens (mean ± σ) |\n");
        md.push_str("|---|---|---|---|\n");
        for cfg in &self.metadata.configs {
            if let Some(c) = self.run_summary.get(cfg) {
                md.push_str(&format!(
                    "| {} | {:.2} ± {:.2} | {:.1} ± {:.1} | {:.0} ± {:.0} |\n",
                    cfg,
                    c.pass_rate.mean,
                    c.pass_rate.stddev,
                    c.time_seconds.mean,
                    c.time_seconds.stddev,
                    c.tokens.mean,
                    c.tokens.stddev
                ));
            }
        }
        md.push_str(&format!(
            "\n**Δ (with_skill − baseline):** pass_rate {}, time {}s, tokens {}\n",
            self.delta.pass_rate, self.delta.time_seconds, self.delta.tokens
        ));
        if !self.notes.is_empty() {
            md.push_str("\n## Notes\n\n");
            for n in &self.notes {
                md.push_str(&format!("- {n}\n"));
            }
        }
        md
    }
}

// ── static HTML viewer (port of generate_review --static) ──────────────────

/// Generate a self-contained static HTML review of an eval iteration.
///
/// Two sections: per-eval outputs (prompt + produced files + formal grades +
/// a feedback textbox) and the benchmark summary (pass-rate/time/tokens per
/// config, with the delta and analyst notes). A "Download feedback" button
/// serializes all textareas to `feedback.json` client-side — no server needed.
pub fn generate_review_html(
    iteration_dir: &Path,
    skill_name: &str,
    out: &Path,
) -> anyhow::Result<PathBuf> {
    let eval_dirs = list_eval_dirs(iteration_dir)?;
    let bench = aggregate_benchmark(iteration_dir, skill_name).ok();

    let mut eval_cards = String::new();
    for (eval_name, eval_dir) in &eval_dirs {
        let prompt = fs::read_to_string(eval_dir.join("eval_metadata.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|v| v.get("prompt").and_then(|p| p.as_str()).map(str::to_string))
            .unwrap_or_default();
        eval_cards.push_str(&render_eval_card(eval_name, &prompt, eval_dir));
    }

    let benchmark_html = match &bench {
        Some(b) => render_benchmark_section(b),
        None => "<p><em>No benchmark data yet. Run <code>skill_forge</code> action <code>benchmark</code> after grading.</em></p>".to_string(),
    };
    let html = HTML_TEMPLATE
        .replace("__SKILL_NAME__", &html_escape(skill_name))
        .replace("__EVAL_CARDS__", &eval_cards)
        .replace("__BENCHMARK__", &benchmark_html);

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(out, html)?;
    Ok(out.to_path_buf())
}

fn render_eval_card(eval_name: &str, prompt: &str, eval_dir: &Path) -> String {
    let mut card = String::new();
    card.push_str(&format!(
        "<section class=\"eval\" data-eval=\"{name}\">\n",
        name = html_escape(eval_name)
    ));
    card.push_str(&format!("<h2>{}</h2>\n", html_escape(eval_name)));
    if !prompt.is_empty() {
        card.push_str(&format!(
            "<details open><summary>Prompt</summary><pre>{}</pre></details>\n",
            html_escape(prompt)
        ));
    }
    for cfg in ["with_skill", "without_skill", "old_skill"] {
        let run_dir = eval_dir.join(cfg);
        if !run_dir.exists() {
            continue;
        }
        let label = if cfg == "old_skill" {
            "baseline (old_skill)"
        } else {
            cfg
        };
        card.push_str(&format!(
            "<details><summary><b>{}</b></summary>\n",
            html_escape(label)
        ));
        // outputs
        let outputs_dir = run_dir.join("outputs");
        if outputs_dir.is_dir() {
            card.push_str("<div class=\"outputs\">");
            if let Ok(entries) = fs::read_dir(&outputs_dir) {
                for e in entries.flatten() {
                    if let Some(text) = read_output_text(&e.path()) {
                        card.push_str(&format!(
                            "<h4>{}</h4><pre>{}</pre>\n",
                            html_escape(&e.file_name().to_string_lossy()),
                            html_escape(&text)
                        ));
                    }
                }
            }
            card.push_str("</div>");
        }
        // formal grades
        if let Some(g) = read_grading(&run_dir.join("grading.json")) {
            card.push_str("<details><summary>Formal grades</summary><table><tr><th>expectation</th><th>verdict</th></tr>");
            for row in &g.expectations {
                let verdict = if row.passed { "✅ pass" } else { "❌ fail" };
                card.push_str(&format!(
                    "<tr><td>{}</td><td>{verdict}</td></tr>",
                    html_escape(&row.text)
                ));
            }
            card.push_str("</table></details>");
        }
        card.push_str(&format!(
            "<textarea data-run=\"{cfg}\" placeholder=\"Leave feedback for {label}…\"></textarea>\n"
        ));
        card.push_str("</details>\n");
    }
    card.push_str("</section>\n");
    card
}

fn render_benchmark_section(b: &BenchmarkData) -> String {
    let mut s = String::new();
    s.push_str("<table><tr><th>config</th><th>pass_rate</th><th>time (s)</th><th>tokens</th></tr>");
    for cfg in &b.metadata.configs {
        if let Some(c) = b.run_summary.get(cfg) {
            s.push_str(&format!(
                "<tr><td>{}</td><td>{:.2} ± {:.2}</td><td>{:.1} ± {:.1}</td><td>{:.0} ± {:.0}</td></tr>",
                html_escape(cfg), c.pass_rate.mean, c.pass_rate.stddev,
                c.time_seconds.mean, c.time_seconds.stddev, c.tokens.mean, c.tokens.stddev
            ));
        }
    }
    s.push_str("</table>");
    s.push_str(&format!(
        "<p><b>Δ (with_skill − baseline):</b> pass_rate {}, time {}s, tokens {}</p>",
        html_escape(&b.delta.pass_rate),
        html_escape(&b.delta.time_seconds),
        html_escape(&b.delta.tokens)
    ));
    if !b.notes.is_empty() {
        s.push_str("<ul>");
        for n in &b.notes {
            s.push_str(&format!("<li>{}</li>", html_escape(n)));
        }
        s.push_str("</ul>");
    }
    s
}

fn read_output_text(path: &Path) -> Option<String> {
    // Only inline text-ish files; skip binaries/large.
    if path.metadata().ok()?.len() > 200_000 {
        return Some(format!("[binary or large file: {}]", path.display()));
    }
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    if text.chars().any(|c| c == '\u{0}') {
        return Some(format!("[binary file: {}]", path.display()));
    }
    Some(text.into_owned())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Skill review — __SKILL_NAME__</title>
<style>
  :root { color-scheme: light dark; --bg:#fff; --fg:#111; --muted:#666; --line:#ddd; }
  @media (prefers-color-scheme: dark){ :root{ --bg:#0d1117; --fg:#e6edf3; --muted:#8b949e; --line:#30363d; } }
  body{ font:14px/1.5 -apple-system,Segoe UI,Roboto,sans-serif; background:var(--bg); color:var(--fg); margin:0 auto; max-width:960px; padding:24px; }
  h1,h2{ line-height:1.25; } details{ margin:.5em 0; padding:.5em .75em; border:1px solid var(--line); border-radius:6px; }
  summary{ cursor:pointer; font-weight:600; } pre{ background:rgba(127,127,127,.1); padding:.75em; border-radius:6px; overflow:auto; white-space:pre-wrap; word-break:break-word; }
  table{ border-collapse:collapse; width:100%; margin:.5em 0; } th,td{ border:1px solid var(--line); padding:.35em .5em; text-align:left; }
  textarea{ width:100%; min-height:60px; margin:.5em 0; font:inherit; box-sizing:border-box; background:transparent; color:var(--fg); border:1px solid var(--line); border-radius:6px; padding:.5em; }
  nav button{ font:inherit; padding:.4em .9em; margin-right:.5em; border:1px solid var(--line); border-radius:6px; background:transparent; color:var(--fg); cursor:pointer; }
  nav button.active{ background:var(--fg); color:var(--bg); } .hidden{ display:none; }
</style>
</head>
<body>
<h1>Skill review — __SKILL_NAME__</h1>
<nav><button id="tab-outputs" class="active" onclick="show('outputs')">Outputs</button><button id="tab-bench" onclick="show('benchmark')">Benchmark</button><button onclick="download()">Download feedback</button></nav>
<div id="outputs">__EVAL_CARDS__</div>
<div id="benchmark" class="hidden">__BENCHMARK__</div>
<script>
function show(id){ document.getElementById('outputs').classList.toggle('hidden',id!=='outputs'); document.getElementById('benchmark').classList.toggle('hidden',id!=='benchmark'); document.getElementById('tab-outputs').classList.toggle('active',id==='outputs'); document.getElementById('tab-bench').classList.toggle('active',id==='benchmark'); }
function download(){ const reviews=[]; document.querySelectorAll('section.eval').forEach(s=>{ s.querySelectorAll('textarea').forEach(t=>{ if(t.value && t.value.trim()){ reviews.push({run_id:s.getAttribute('data-eval')+'-'+t.getAttribute('data-run'), feedback:t.value, timestamp:new Date().toISOString()}); } }); }); const blob=new Blob([JSON.stringify({reviews,status:'complete'},null,2)],{type:'application/json'}); const a=document.createElement('a'); a.href=URL.createObjectURL(blob); a.download='feedback.json'; a.click(); }
</script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\nname: my-skill\ndescription: Does X when the user wants Y. Fires on mentions of Y or Z.\n---\n\n# My Skill\n\nInstructions here.\n";

    #[test]
    fn validate_accepts_well_formed_skill() {
        let report = validate_skill_content(GOOD, Some("my-skill"));
        assert!(!report.has_errors(), "{}", report.render());
        assert_eq!(report.name, "my-skill");
    }

    #[test]
    fn validate_flags_missing_frontmatter() {
        let report = validate_skill_content("# just a body\n", None);
        assert!(report.has_errors());
        assert!(report.render().contains("frontmatter"));
    }

    #[test]
    fn validate_flags_missing_name() {
        let bad = "---\ndescription: A skill.\n---\n\nbody\n";
        let report = validate_skill_content(bad, None);
        assert!(report.has_errors());
        assert!(report.render().contains("name"));
    }

    #[test]
    fn validate_flags_name_dir_mismatch() {
        let report = validate_skill_content(GOOD, Some("other-name"));
        assert!(report.has_errors());
        assert!(report.render().contains("does not match"));
    }

    #[test]
    fn validate_flags_invalid_name() {
        let bad = "---\nname: My Skill!\ndescription: A skill that does the thing when needed.\n---\n\nbody\n";
        let report = validate_skill_content(bad, None);
        assert!(report.has_errors());
    }

    #[test]
    fn validate_warns_on_short_description() {
        let bad = "---\nname: x\ndescription: short\n---\n\nbody\n";
        let report = validate_skill_content(bad, None);
        assert!(!report.has_errors());
        assert!(!report.warnings().is_empty());
    }

    #[test]
    fn validate_warns_on_long_body() {
        let long_body: String = "line\n".repeat(600);
        let bad = format!(
            "---\nname: big\ndescription: A skill that does the thing when the user asks.\n---\n\n{long_body}"
        );
        let report = validate_skill_content(&bad, None);
        assert!(!report.has_errors());
        assert!(report.warnings().iter().any(|w| w.contains("500")));
    }

    #[test]
    fn validate_flags_empty_body() {
        let bad = "---\nname: empty\ndescription: A skill that does the thing when the user asks.\n---\n\n";
        let report = validate_skill_content(bad, None);
        assert!(report.has_errors());
    }

    #[test]
    fn is_valid_skill_name_rules() {
        assert!(is_valid_skill_name("my-skill"));
        assert!(is_valid_skill_name("a"));
        assert!(is_valid_skill_name("skill-2"));
        assert!(!is_valid_skill_name(""));
        assert!(!is_valid_skill_name("-leading"));
        assert!(!is_valid_skill_name("trailing-"));
        assert!(!is_valid_skill_name("UPPER"));
        assert!(!is_valid_skill_name("with space"));
        assert!(!is_valid_skill_name("under_score"));
    }

    #[test]
    fn package_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("demo-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo-skill\ndescription: A demo skill.\n---\n\n# Demo\n",
        )
        .unwrap();
        fs::create_dir_all(skill_dir.join("references")).unwrap();
        fs::write(skill_dir.join("references/api.md"), "# API\n").unwrap();
        // excluded at root
        fs::create_dir_all(skill_dir.join("evals")).unwrap();
        fs::write(skill_dir.join("evals/evals.json"), "{}").unwrap();
        // excluded everywhere
        fs::create_dir_all(skill_dir.join("__pycache__")).unwrap();
        fs::write(skill_dir.join("__pycache__/x.pyc"), "x").unwrap();
        fs::write(skill_dir.join(".DS_Store"), "x").unwrap();

        let out = tmp.path().join("demo-skill.skill");
        let path = package_skill_dir(&skill_dir, "demo-skill", &out).unwrap();
        assert!(path.exists());

        // Re-open and verify contents.
        let f = fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "demo-skill/SKILL.md"));
        assert!(names.iter().any(|n| n == "demo-skill/references/api.md"));
        // evals/ excluded at root
        assert!(names.iter().all(|n| !n.contains("evals/")));
        // build artifacts excluded
        assert!(names.iter().all(|n| !n.contains("__pycache__")));
        assert!(names.iter().all(|n| !n.contains(".DS_Store")));
        assert!(names.iter().all(|n| !n.contains(".pyc")));
    }
    #[cfg(unix)]
    #[test]
    fn package_skill_skips_symlinks() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        // A secret outside the skill tree that a symlink must NOT pull in.
        let secret = tmp.path().join("secret.txt");
        fs::write(&secret, "PRIVATE_KEY_CONTENTS").unwrap();

        let skill_dir = tmp.path().join("leak-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: leak-skill\ndescription: A skill that tries to smuggle a symlink.\n---\n\n# x\n",
        )
        .unwrap();
        // Malicious symlink inside the skill tree → must be skipped, not followed.
        symlink(&secret, skill_dir.join("id_rsa")).unwrap();

        let out = tmp.path().join("leak-skill.skill");
        package_skill_dir(&skill_dir, "leak-skill", &out).unwrap();

        let f = fs::File::open(&out).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        for i in 0..archive.len() {
            let name = archive.by_index(i).unwrap().name().to_string();
            assert!(
                !name.contains("id_rsa"),
                "symlink leaked into archive: {name}"
            );
        }
        // And the secret contents must not appear anywhere in the archive bytes.
        let bytes = fs::read(&out).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("PRIVATE_KEY_CONTENTS"),
            "symlink target contents leaked into .skill archive"
        );
    }
    #[test]
    fn aggregate_benchmark_computes_stats_and_delta() {
        let tmp = tempfile::tempdir().unwrap();
        let iter = tmp.path().join("iteration-1");
        let mk_run = |eval: &str, cfg: &str, pass_rate: f64, tokens: f64| {
            let dir = iter.join(eval).join(cfg);
            fs::create_dir_all(dir.join("outputs")).unwrap();
            fs::write(
                dir.join("grading.json"),
                serde_json::json!({
                    "summary": {"passed": 2, "failed": 0, "total": 2, "pass_rate": pass_rate},
                    "expectations": [
                        {"text": "has X", "passed": true},
                        {"text": "has Y", "passed": pass_rate > 0.5}
                    ]
                })
                .to_string(),
            )
            .unwrap();
            fs::write(
                dir.join("timing.json"),
                serde_json::json!({"total_tokens": tokens, "total_duration_seconds": 10.0})
                    .to_string(),
            )
            .unwrap();
        };
        // Two evals × two configs.
        mk_run("eval-0", "with_skill", 1.0, 3800.0);
        mk_run("eval-0", "without_skill", 0.5, 2100.0);
        mk_run("eval-1", "with_skill", 0.9, 4000.0);
        mk_run("eval-1", "without_skill", 0.3, 1900.0);

        let bench = aggregate_benchmark(&iter, "demo").unwrap();
        assert_eq!(bench.metadata.eval_count, 2);
        assert!(bench.metadata.configs.contains(&"with_skill".to_string()));
        assert!(
            bench
                .metadata
                .configs
                .contains(&"without_skill".to_string())
        );
        // with_skill mean pass_rate = (1.0 + 0.9)/2 = 0.95
        let with = bench.run_summary.get("with_skill").unwrap();
        assert!((with.pass_rate.mean - 0.95).abs() < 1e-9);
        // delta is a formatted signed string: 0.95 - 0.4 = +0.55
        assert!(
            bench.delta.pass_rate.starts_with('+'),
            "delta={}",
            bench.delta.pass_rate
        );

        // benchmark.md is non-empty and mentions both configs.
        let md = bench.to_markdown();
        assert!(md.contains("with_skill"));
        assert!(md.contains("without_skill"));
    }

    #[test]
    fn aggregate_benchmark_errors_on_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let iter = tmp.path().join("iteration-1");
        fs::create_dir_all(&iter).unwrap();
        assert!(aggregate_benchmark(&iter, "demo").is_err());
    }

    #[test]
    fn generate_review_html_writes_self_contained_file() {
        let tmp = tempfile::tempdir().unwrap();
        let iter = tmp.path().join("iteration-1");
        let dir = iter.join("eval-0").join("with_skill");
        fs::create_dir_all(dir.join("outputs")).unwrap();
        fs::write(dir.join("outputs/result.txt"), "hello world").unwrap();
        fs::write(
            dir.join("grading.json"),
            serde_json::json!({
                "summary": {"pass_rate": 1.0},
                "expectations": [{"text": "says hello", "passed": true}]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            dir.join("timing.json"),
            serde_json::json!({"total_tokens": 100.0, "total_duration_seconds": 5.0}).to_string(),
        )
        .unwrap();
        // baseline so benchmark has two configs
        let bdir = iter.join("eval-0").join("without_skill");
        fs::create_dir_all(bdir.join("outputs")).unwrap();
        fs::write(
            bdir.join("grading.json"),
            serde_json::json!({"summary": {"pass_rate": 0.0}, "expectations": []}).to_string(),
        )
        .unwrap();
        fs::write(
            bdir.join("timing.json"),
            serde_json::json!({"total_tokens": 80.0, "total_duration_seconds": 4.0}).to_string(),
        )
        .unwrap();

        let out = tmp.path().join("review.html");
        let path = generate_review_html(&iter, "demo", &out).unwrap();
        assert!(path.exists());
        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("hello world"));
        assert!(html.contains("says hello"));
        assert!(html.contains("with_skill"));
        // feedback download mechanism present
        assert!(html.contains("download()"));
    }
}
