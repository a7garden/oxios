#![allow(dead_code)]
//! Prompt builder: pure function that assembles system prompts from structured inputs.
//!
//! This is Layer 1 of the three-layer prompt architecture:
//! - Layer 1 (this module): Pure rendering, no config access, no I/O.
//! - Layer 2: Config resolution (`resolve_system_prompt_config`).
//! - Layer 3: Runtime adapters gather live facts and call the combined facade.
//!
//! # Cache-Aware Structure
//!
//! The builder produces prompts with an explicit cache boundary:
//! - **Stable prefix**: Tool summaries, safety, context files, skills catalog, workspace.
//! - **Volatile suffix**: Channel guidance, provider contributions, runtime info, model identity.

use crate::prompt::cache_boundary::{normalize_section, CACHE_BOUNDARY};
use crate::prompt::normalize::{deduplicate_guidelines, sort_context_files, sort_tool_names};
use crate::prompt::types::{PromptMode, PromptOptions, PromptSurface};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a complete system prompt from structured options.
///
/// This is a pure function — no side effects, no I/O, no config access.
/// The caller is responsible for resolving all inputs before calling.
pub fn build_system_prompt(opts: &PromptOptions) -> String {
    if opts.mode == PromptMode::None {
        return build_identity_only(opts);
    }

    let mut stable = Vec::new();
    let mut volatile = Vec::new();

    // ── Stable prefix (cached across turns) ──

    stable.push(build_identity_section(opts));
    stable.push(build_tooling_section(opts));
    stable.push(build_safety_section());

    if opts.mode == PromptMode::Full {
        stable.push(build_guidelines_section(opts));
        stable.push(build_context_section(opts));
        stable.push(build_skills_catalog(opts));
    }

    stable.push(build_workspace_section(opts));

    // Provider stable contribution
    if let Some(ref prefix) = opts.provider_contribution.stable_prefix {
        stable.push(prefix.clone());
    }

    // ── Cache boundary ──

    // ── Volatile suffix (changes per turn) ──

    if opts.mode == PromptMode::Full {
        volatile.push(build_channel_section(opts));
    }

    volatile.push(build_runtime_section(opts));

    // Provider dynamic contribution
    if let Some(ref suffix) = opts.provider_contribution.dynamic_suffix {
        volatile.push(suffix.clone());
    }

    // Assemble
    let stable_text: String = stable
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| normalize_section(&s))
        .collect::<Vec<_>>()
        .join("\n\n");

    let volatile_text: String = volatile
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| normalize_section(&s))
        .collect::<Vec<_>>()
        .join("\n\n");

    if volatile_text.is_empty() {
        stable_text
    } else {
        format!("{}{}{}", stable_text, CACHE_BOUNDARY, volatile_text)
    }
}

// ---------------------------------------------------------------------------
// Section builders
// ---------------------------------------------------------------------------

/// Identity-only mode (PromptMode::None).
fn build_identity_only(opts: &PromptOptions) -> String {
    let model_line = opts
        .model_id
        .as_ref()
        .map(|m| format!(" You are running as {}.", m))
        .unwrap_or_default();
    format!(
        "You are an autonomous agent in the Oxios operating system.{}",
        model_line
    )
}

/// Agent identity and role.
fn build_identity_section(opts: &PromptOptions) -> String {
    let mut section = String::from(
        "You are an autonomous agent in the Oxios operating system.\n\
         You execute Seeds — immutable specifications with goals, constraints, and\n\
         acceptance criteria. You have tools for reading, writing, editing files,\n\
         running commands, and accessing kernel services.",
    );

    if let Some(ref model) = opts.model_id {
        section.push_str(&format!(
            "\n\nModel identity: {}. If asked what model you are, answer with this value.",
            model
        ));
    }

    section
}

/// Tool summary list (one line per tool).
fn build_tooling_section(opts: &PromptOptions) -> String {
    if opts.tools.is_empty() {
        return String::new();
    }

    let mut lines = vec!["## Available Tools".to_string()];

    let tool_names: Vec<String> = opts.tools.keys().cloned().collect();
    let sorted = sort_tool_names(&tool_names);

    for name in &sorted {
        if let Some(meta) = opts.tools.get(*name) {
            lines.push(format!("- {}: {}", name, meta.snippet));
        } else {
            lines.push(format!("- {}", name));
        }
    }

    lines.join("\n")
}

/// Safety advisory (always included, even in minimal mode).
fn build_safety_section() -> String {
    String::from(
        "## Safety\n\
         No independent goals: no self-preservation, replication, resource acquisition, or\n\
         long-term plans beyond the user's request. Safety over completion. Conflicts:\n\
         pause and ask. Obey stop/pause/audit directives. Never bypass safeguards.",
    )
}

/// Deduplicated, tool-aware guidelines.
fn build_guidelines_section(opts: &PromptOptions) -> String {
    let mut all_guidelines: Vec<String> = Vec::new();

    // Tool-contributed guidelines
    for meta in opts.tools.values() {
        all_guidelines.extend(meta.guidelines.iter().cloned());
    }

    // Extra guidelines
    all_guidelines.extend(opts.extra_guidelines.iter().cloned());

    let deduped = deduplicate_guidelines(&all_guidelines);
    if deduped.is_empty() {
        return String::new();
    }

    let mut lines = vec!["## Guidelines".to_string()];
    for g in &deduped {
        lines.push(format!("- {}", g));
    }
    lines.join("\n")
}

/// Context files (AGENTS.md, SOUL.md, etc.) wrapped in XML.
fn build_context_section(opts: &PromptOptions) -> String {
    if opts.context_files.is_empty() {
        return String::new();
    }

    let mut files = opts.context_files.clone();
    sort_context_files(&mut files);

    let mut lines = vec!["<project_context>".to_string()];

    for file in &files {
        let name = file.kind.as_ref();
        lines.push(format!(
            "<project_instructions path=\"{}\" kind=\"{}\">\n{}\n</project_instructions>",
            file.path.display(),
            name,
            file.content
        ));
    }

    lines.push("</project_context>".to_string());
    lines.join("\n\n")
}

/// Skills catalog (XML, lazy-load).
fn build_skills_catalog(opts: &PromptOptions) -> String {
    if opts.skills.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "The following skills provide specialized instructions.".to_string(),
        "Use the `read` tool to load a skill's SKILL.md when the task matches.".to_string(),
        "<available_skills>".to_string(),
    ];

    // Sort skills alphabetically for determinism
    let mut skills = opts.skills.clone();
    skills.sort_by(|a, b| a.name.cmp(&b.name));

    for skill in &skills {
        lines.push("  <skill>".to_string());
        lines.push(format!("    <name>{}</name>", xml_escape(&skill.name)));
        lines.push(format!(
            "    <description>{}</description>",
            xml_escape(&skill.description)
        ));
        lines.push(format!(
            "    <location>{}</location>",
            skill.path.display()
        ));
        lines.push("  </skill>".to_string());
    }

    lines.push("</available_skills>".to_string());
    lines.join("\n")
}

/// Workspace information.
fn build_workspace_section(opts: &PromptOptions) -> String {
    match &opts.workspace_dir {
        Some(dir) => format!("## Workspace\n{}", dir.display()),
        None => String::new(),
    }
}

/// Channel-specific guidance (volatile).
fn build_channel_section(opts: &PromptOptions) -> String {
    match opts.surface {
        PromptSurface::Telegram => String::from(
            "## Channel: Telegram\n\
             Be a good Telegram participant. Reply concisely. Use markdown formatting.\n\
             For long output, prefer writing to a file and sharing the path.",
        ),
        PromptSurface::Web => String::from(
            "## Channel: Web Dashboard\n\
             Output will be rendered in the web UI. Use markdown formatting.\n\
             File paths will be clickable links.",
        ),
        PromptSurface::Cli => String::from(
            "## Channel: CLI\n\
             Output goes to stdout. Be direct and technical. Use ANSI-safe formatting.",
        ),
        PromptSurface::Generic => String::new(),
    }
}

/// Runtime info (volatile — changes every turn).
fn build_runtime_section(opts: &PromptOptions) -> String {
    let mut lines = vec!["## Runtime".to_string()];

    if let Some(ref date) = opts.current_date {
        lines.push(format!("Current date: {}", date));
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape special XML characters.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

impl std::fmt::Display for super::types::ContextFileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            super::types::ContextFileKind::Agents => write!(f, "agents"),
            super::types::ContextFileKind::Soul => write!(f, "soul"),
            super::types::ContextFileKind::Identity => write!(f, "identity"),
            super::types::ContextFileKind::User => write!(f, "user"),
            super::types::ContextFileKind::Tools => write!(f, "tools"),
            super::types::ContextFileKind::Memory => write!(f, "memory"),
        }
    }
}

impl AsRef<str> for super::types::ContextFileKind {
    fn as_ref(&self) -> &str {
        match self {
            super::types::ContextFileKind::Agents => "agents",
            super::types::ContextFileKind::Soul => "soul",
            super::types::ContextFileKind::Identity => "identity",
            super::types::ContextFileKind::User => "user",
            super::types::ContextFileKind::Tools => "tools",
            super::types::ContextFileKind::Memory => "memory",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::types::{ContextFile, ContextFileKind, SkillSummary, ToolPromptMeta};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn default_opts() -> PromptOptions {
        PromptOptions {
            mode: PromptMode::Full,
            surface: PromptSurface::Generic,
            model_id: Some("anthropic/claude-sonnet-4".to_string()),
            tools: HashMap::new(),
            context_files: vec![],
            skills: vec![],
            provider_contribution: Default::default(),
            extra_guidelines: vec![],
            workspace_dir: Some(PathBuf::from("/tmp/workspace")),
            current_date: Some("2025-01-01".to_string()),
        }
    }

    #[test]
    fn test_full_mode_includes_all_sections() {
        let prompt = build_system_prompt(&default_opts());
        assert!(prompt.contains("Oxios operating system"));
        assert!(prompt.contains("## Safety"));
        assert!(prompt.contains("anthropic/claude-sonnet-4"));
        assert!(prompt.contains("/tmp/workspace"));
    }

    #[test]
    fn test_minimal_mode_omits_guidelines_context_skills() {
        let opts = PromptOptions {
            mode: PromptMode::Minimal,
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("## Safety"));
        assert!(!prompt.contains("<project_context>"));
        assert!(!prompt.contains("<available_skills>"));
    }

    #[test]
    fn test_none_mode_is_identity_only() {
        let opts = PromptOptions {
            mode: PromptMode::None,
            model_id: Some("test/model".to_string()),
            ..Default::default()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("autonomous agent"));
        assert!(prompt.contains("test/model"));
        assert!(!prompt.contains("## Safety"));
    }

    #[test]
    fn test_tool_summaries_appear() {
        let mut tools = HashMap::new();
        tools.insert(
            "read".to_string(),
            ToolPromptMeta {
                snippet: "Read file contents".to_string(),
                guidelines: vec!["Use read instead of cat".to_string()],
            },
        );
        let opts = PromptOptions {
            tools,
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("- read: Read file contents"));
        assert!(prompt.contains("Use read instead of cat"));
    }

    #[test]
    fn test_context_files_wrapped_in_xml() {
        let opts = PromptOptions {
            context_files: vec![ContextFile {
                path: PathBuf::from("/project/AGENTS.md"),
                content: "Use Rust 2021".to_string(),
                kind: ContextFileKind::Agents,
            }],
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("<project_context>"));
        assert!(prompt.contains("Use Rust 2021"));
        assert!(prompt.contains("</project_context>"));
    }

    #[test]
    fn test_skills_catalog_xml() {
        let opts = PromptOptions {
            skills: vec![SkillSummary {
                name: "code-review".to_string(),
                description: "Deep code review".to_string(),
                path: PathBuf::from("/skills/code-review/SKILL.md"),
            }],
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("<name>code-review</name>"));
        assert!(prompt.contains("<description>Deep code review</description>"));
    }

    #[test]
    fn test_channel_telegram_guidance() {
        let opts = PromptOptions {
            surface: PromptSurface::Telegram,
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("Channel: Telegram"));
    }

    #[test]
    fn test_cache_boundary_present_in_full_mode() {
        // Full mode with volatile content should have boundary
        let opts = PromptOptions {
            surface: PromptSurface::Cli,
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        assert!(prompt.contains("OXIOS_CACHE_BOUNDARY"));
    }

    #[test]
    fn test_guideline_deduplication() {
        let mut tools = HashMap::new();
        tools.insert(
            "read".to_string(),
            ToolPromptMeta {
                snippet: "Read".to_string(),
                guidelines: vec!["Be concise".to_string()],
            },
        );
        let opts = PromptOptions {
            tools,
            extra_guidelines: vec!["Be concise".to_string()],
            ..default_opts()
        };
        let prompt = build_system_prompt(&opts);
        // "Be concise" should appear only once
        assert_eq!(prompt.matches("Be concise").count(), 1);
    }
}
