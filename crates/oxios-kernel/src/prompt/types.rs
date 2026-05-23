//! Prompt system types: modes, surfaces, and shared data structures.
//!
//! These types define the vocabulary of the prompt assembly system.
//! The builder consumes them to produce structured system prompts.

use std::collections::HashMap;
use std::path::PathBuf;

/// Controls which sections are included in the system prompt.
///
/// - `Full`: Main agent — all sections included.
/// - `Minimal`: Sub-agents — core sections only, no channel/memory/messaging.
/// - `None`: Minimal identity line only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptMode {
    /// All sections: identity, tools, guidelines, context, skills, safety.
    Full,
    /// Reduced: identity, tools, safety, workspace. No memory, messaging, or channel guidance.
    Minimal,
    /// Identity line only.
    None,
}

impl Default for PromptMode {
    fn default() -> Self {
        Self::Full
    }
}

/// The runtime surface that will consume this prompt.
///
/// Different surfaces get different channel-specific guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptSurface {
    /// Main CLI or direct invocation.
    Cli,
    /// Web dashboard agent.
    Web,
    /// Telegram bot agent.
    Telegram,
    /// Generic / unknown surface.
    Generic,
}

impl Default for PromptSurface {
    fn default() -> Self {
        Self::Generic
    }
}

/// Metadata about an active tool, contributed by the tool itself.
///
/// Tools self-declare their prompt presence via `prompt_snippet` and
/// `prompt_guidelines`. The prompt builder aggregates these into the
/// system prompt.
#[derive(Debug, Clone)]
pub struct ToolPromptMeta {
    /// One-line description for the "Available tools" section.
    pub snippet: String,
    /// Behavioral guidelines for using this tool.
    pub guidelines: Vec<String>,
}

/// A context file discovered from the filesystem (AGENTS.md, SOUL.md, etc.).
#[derive(Debug, Clone)]
pub struct ContextFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// File content.
    pub content: String,
    /// Canonical file kind for ordering.
    pub kind: ContextFileKind,
}

/// Known context file kinds, ordered by injection priority.
///
/// Lower ordinal = injected first (stable prefix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContextFileKind {
    /// Project instructions (AGENTS.md).
    Agents = 10,
    /// Agent personality (SOUL.md).
    Soul = 20,
    /// Agent identity (IDENTITY.md).
    Identity = 30,
    /// User profile (USER.md).
    User = 40,
    /// Tool usage notes (TOOLS.md).
    Tools = 50,
    /// Durable memory (MEMORY.md).
    Memory = 70,
}

/// A skill summary for the lazy-load catalog.
///
/// Only metadata is included in the system prompt; the full SKILL.md
/// is loaded on demand via the `read` tool.
#[derive(Debug, Clone)]
pub struct SkillSummary {
    /// Unique skill name.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Path to the SKILL.md file for on-demand loading.
    pub path: PathBuf,
}

/// Provider-specific prompt contribution.
///
/// Providers (Anthropic, OpenAI, etc.) can contribute model-family-specific
/// guidance without touching the core prompt.
#[derive(Debug, Clone, Default)]
pub struct ProviderContribution {
    /// Content to include in the stable prefix (above cache boundary).
    pub stable_prefix: Option<String>,
    /// Content to include in the volatile suffix (below cache boundary).
    pub dynamic_suffix: Option<String>,
    /// Section overrides keyed by section ID.
    pub section_overrides: HashMap<String, String>,
}

/// All inputs needed to build a system prompt.
///
/// This struct is the single source of truth for prompt assembly.
/// It carries pre-resolved data — no config access or I/O needed.
#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    /// Prompt mode (full/minimal/none).
    pub mode: PromptMode,
    /// Runtime surface (CLI/Web/Telegram).
    pub surface: PromptSurface,
    /// Model ID for identity line (e.g., "anthropic/claude-sonnet-4").
    pub model_id: Option<String>,
    /// Active tool names and their prompt metadata.
    pub tools: HashMap<String, ToolPromptMeta>,
    /// Discovered context files, ordered by kind.
    pub context_files: Vec<ContextFile>,
    /// Skill summaries for the lazy-load catalog.
    pub skills: Vec<SkillSummary>,
    /// Provider-specific contributions.
    pub provider_contribution: ProviderContribution,
    /// Additional guidelines (deduplicated during assembly).
    pub extra_guidelines: Vec<String>,
    /// Workspace directory path.
    pub workspace_dir: Option<PathBuf>,
    /// Current date (injected as volatile section).
    pub current_date: Option<String>,
}
