//! Auto-memory bridge: synchronization between external memory systems and Oxios.
//!
//! Bridges Oxios MemoryManager with Claude Code's MEMORY.md format and
//! similar external memory stores. Supports bidirectional sync:
//!
//! - **to-auto**: Export Oxios patterns/insights → external MEMORY.md format
//! - **from-auto**: Import external memories → Oxios MemoryStore
//! - **bidirectional**: Full two-way synchronization
//!
//! The bridge converts between Oxios's structured memory entries and
//! free-form markdown memory formats used by external tools.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::{MemoryEntry, MemoryManager, MemoryType};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Direction of memory synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    /// Export Oxios → external format.
    ToAuto,
    /// Import external → Oxios.
    FromAuto,
    /// Two-way sync.
    Bidirectional,
}

/// Category of an imported memory insight.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightCategory {
    /// Project-level patterns and conventions.
    ProjectPatterns,
    /// Debugging strategies and fixes.
    Debugging,
    /// Architecture decisions.
    Architecture,
    /// Performance observations.
    Performance,
    /// Security-related insights.
    Security,
    /// General knowledge.
    General,
}

impl InsightCategory {
    /// Parse from a string.
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "project-patterns" | "project_patterns" | "patterns" => {
                InsightCategory::ProjectPatterns
            }
            "debugging" | "debug" => InsightCategory::Debugging,
            "architecture" | "arch" => InsightCategory::Architecture,
            "performance" | "perf" => InsightCategory::Performance,
            "security" | "sec" => InsightCategory::Security,
            _ => InsightCategory::General,
        }
    }

    /// Convert to a tag string.
    pub fn to_tag(&self) -> &'static str {
        match self {
            InsightCategory::ProjectPatterns => "project-patterns",
            InsightCategory::Debugging => "debugging",
            InsightCategory::Architecture => "architecture",
            InsightCategory::Performance => "performance",
            InsightCategory::Security => "security",
            InsightCategory::General => "general",
        }
    }
}

/// A single imported memory insight from an external system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInsight {
    /// Category of the insight.
    pub category: InsightCategory,
    /// Brief summary (1 line).
    pub summary: String,
    /// Optional detailed content.
    pub detail: Option<String>,
    /// Source identifier (e.g., "claude-code", "user").
    pub source: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
}

/// Result of importing memories from an external system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportResult {
    /// Number of insights successfully imported.
    pub imported: usize,
    /// Number of duplicates skipped.
    pub skipped_duplicates: usize,
    /// Number of failed imports.
    pub failed: usize,
    /// Error messages for failed imports.
    pub errors: Vec<String>,
}

/// Result of exporting memories to an external system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportResult {
    /// Number of patterns exported.
    pub exported: usize,
    /// Number of categories created/updated.
    pub categories_updated: usize,
}

/// Result of a bidirectional sync operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    /// Import results.
    pub import: ImportResult,
    /// Export results.
    pub export: ExportResult,
}

/// A guidance pattern for export to external format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidancePattern {
    /// Pattern identifier.
    pub id: String,
    /// Category.
    pub category: InsightCategory,
    /// Brief description.
    pub description: String,
    /// Confidence/importance.
    pub confidence: f32,
    /// Usage count.
    pub usage_count: u32,
}

// ---------------------------------------------------------------------------
// AutoMemoryBridge
// ---------------------------------------------------------------------------

/// Bridge between Oxios memory and external memory systems.
///
/// Bridges Oxios MemoryManager (agent session memory) and optional
/// KnowledgeBase (markdown knowledge base) with external MEMORY.md
/// files. Supports bidirectional sync with Claude Code and similar
/// external tools.
///
/// **RFC-003: KnowledgeBase is the single source of truth for markdown.
/// MemoryManager stores agent session memory. The bridge can sync from
/// either source or both.**
pub struct AutoMemoryBridge {
    /// Directory containing external memory files.
    auto_memory_dir: PathBuf,
    /// Oxios memory manager (agent session memory).
    oxios_memory: std::sync::Arc<MemoryManager>,
    /// Optional markdown knowledge base (global, not per-Space).
    knowledge_base: Option<std::sync::Arc<oxios_markdown::KnowledgeBase>>,
}

impl AutoMemoryBridge {
    /// Create a new bridge.
    ///
    /// # Arguments
    /// * `auto_memory_dir` - Path to directory containing MEMORY.md files
    /// * `oxios_memory` - Oxios MemoryManager instance
    pub fn new(auto_memory_dir: PathBuf, oxios_memory: std::sync::Arc<MemoryManager>) -> Self {
        Self {
            auto_memory_dir,
            oxios_memory,
            knowledge_base: None,
        }
    }

    /// Set the optional markdown knowledge base.
    ///
    /// When set, `export_knowledge_to_auto()` can read directly from
    /// `.md` files instead of relying on MemoryManager entries.
    pub fn with_knowledge_base(mut self, kb: std::sync::Arc<oxios_markdown::KnowledgeBase>) -> Self {
        self.knowledge_base = Some(kb);
        self
    }

    /// Import memories from external format into Oxios.
    ///
    /// Reads MEMORY.md files from the auto_memory_dir, parses them into
    /// structured insights, and stores them via MemoryManager.
    pub async fn import_from_auto(&self) -> Result<ImportResult> {
        let mut result = ImportResult::default();

        // Find all MEMORY.md files
        let memory_files = self.find_memory_files()?;

        for file_path in &memory_files {
            match self.import_file(file_path).await {
                Ok(file_result) => {
                    result.imported += file_result.imported;
                    result.skipped_duplicates += file_result.skipped_duplicates;
                    result.failed += file_result.failed;
                    result.errors.extend(file_result.errors);
                }
                Err(e) => {
                    result.failed += 1;
                    result
                        .errors
                        .push(format!("{}: {}", file_path.display(), e));
                }
            }
        }

        Ok(result)
    }

    /// Export Oxios patterns to external MEMORY.md format.
    ///
    /// Reads patterns from Oxios memory and writes them as structured
    /// markdown files in the auto_memory_dir.
    pub async fn export_to_auto(&self, patterns: &[GuidancePattern]) -> Result<ExportResult> {
        let mut result = ExportResult::default();

        // Ensure output directory exists
        tokio::fs::create_dir_all(&self.auto_memory_dir).await?;

        // Group patterns by category
        let mut by_category: HashMap<InsightCategory, Vec<&GuidancePattern>> = HashMap::new();
        for pattern in patterns {
            by_category
                .entry(pattern.category.clone())
                .or_default()
                .push(pattern);
        }

        // Write each category to its own file
        for (category, cat_patterns) in &by_category {
            let filename = match category {
                InsightCategory::ProjectPatterns => "patterns.md",
                InsightCategory::Debugging => "debugging.md",
                InsightCategory::Architecture => "architecture.md",
                InsightCategory::Performance => "performance.md",
                InsightCategory::Security => "security.md",
                InsightCategory::General => "general.md",
            };

            let content = self.format_patterns_md(cat_patterns);
            let path = self.auto_memory_dir.join(filename);

            tokio::fs::write(&path, &content).await?;
            result.categories_updated += 1;
        }

        // Write main MEMORY.md with all patterns sorted by confidence
        let mut all_patterns: Vec<&GuidancePattern> = patterns.iter().collect();
        all_patterns.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let main_content = self.format_main_md(&all_patterns);
        let main_path = self.auto_memory_dir.join("MEMORY.md");
        tokio::fs::write(&main_path, &main_content).await?;

        result.exported = patterns.len();
        result.categories_updated += 1; // MEMORY.md itself
        Ok(result)
    }

    /// Perform a bidirectional sync.
    pub async fn sync_session(&self, direction: SyncDirection) -> Result<SyncResult> {
        let mut sync_result = SyncResult::default();

        match direction {
            SyncDirection::FromAuto => {
                sync_result.import = self.import_from_auto().await?;
            }
            SyncDirection::ToAuto => {
                sync_result.export = self.export_knowledge_to_auto().await?;
            }
            SyncDirection::Bidirectional => {
                sync_result.import = self.import_from_auto().await?;
                sync_result.export = self.export_knowledge_to_auto().await?;
            }
        }

        Ok(sync_result)
    }

    /// Returns the auto-memory directory path.
    pub fn auto_memory_dir(&self) -> &Path {
        &self.auto_memory_dir
    }

    /// Export all knowledge memories from Oxios to external format.
    ///
    /// Reads from MemoryManager (primary) or falls back to KnowledgeBase
    /// `.md` files when `knowledge_base` is set and MemoryManager has no
    /// `MemoryType::Knowledge` entries.
    async fn export_knowledge_to_auto(&self) -> Result<ExportResult> {
        // Try MemoryManager first (primary source)
        let entries = self
            .oxios_memory
            .list(MemoryType::Knowledge, 1000)
            .await
            .unwrap_or_default();

        if !entries.is_empty() {
            let patterns: Vec<GuidancePattern> = entries
                .iter()
                .map(|e| GuidancePattern {
                    id: e.id.clone(),
                    category: e
                        .tags
                        .first()
                        .map(|t| InsightCategory::from_str_loose(t))
                        .unwrap_or(InsightCategory::General),
                    description: e.content.clone(),
                    confidence: e.importance,
                    usage_count: e.access_count,
                })
                .collect();
            return self.export_to_auto(&patterns).await;
        }

        // Fall back to KnowledgeBase .md files (RFC-003)
        if let Some(kb) = &self.knowledge_base {
            let entries = kb.index_all()?;
            if entries > 0 {
                let kb_root = kb.root();
                let patterns = kb
                    .note_tree("/")
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|e| !e.is_dir && e.name.ends_with(".md"))
                    .map(|e| {
                        let path = if e.parent_dir == "/" || e.parent_dir.is_empty() {
                            e.name.clone()
                        } else {
                            format!("{}/{}", e.parent_dir, e.name)
                        };
                        let content = kb.note_read(&path).ok().flatten().unwrap_or_default();
                        let headings = kb.extract_headings(&content);
                        let tag = headings.first().map(|s| s.as_str()).unwrap_or("general");
                        GuidancePattern {
                            id: format!("note-{}", path.replace('/', "-").trim_end_matches(".md")),
                            category: InsightCategory::from_str_loose(tag),
                            description: content.chars().take(300).collect(),
                            confidence: 0.6,
                            usage_count: 0,
                        }
                    })
                    .collect::<Vec<_>>();
                if !patterns.is_empty() {
                    return self.export_to_auto(&patterns).await;
                }
            }
        }

        // No entries from either source — export empty result
        Ok(ExportResult::default())
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl AutoMemoryBridge {
    /// Find all MEMORY.md files in the auto_memory_dir.
    fn find_memory_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if self.auto_memory_dir.exists() {
            // Look for MEMORY.md directly
            let main = self.auto_memory_dir.join("MEMORY.md");
            if main.exists() {
                files.push(main);
            }

            // Look for topic files
            for topic in &[
                "patterns.md",
                "debugging.md",
                "architecture.md",
                "performance.md",
                "security.md",
                "general.md",
            ] {
                let path = self.auto_memory_dir.join(topic);
                if path.exists() {
                    files.push(path);
                }
            }

            // Look for *.md files in subdirectories
            if let Ok(entries) = std::fs::read_dir(&self.auto_memory_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "md") {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        // Skip already-added files
                        if ![
                            "MEMORY.md",
                            "patterns.md",
                            "debugging.md",
                            "architecture.md",
                            "performance.md",
                            "security.md",
                            "general.md",
                        ]
                        .contains(&name.as_ref())
                        {
                            files.push(path);
                        }
                    }
                }
            }
        }

        Ok(files)
    }

    /// Import a single markdown file into Oxios memory.
    async fn import_file(&self, path: &Path) -> Result<ImportResult> {
        let content = tokio::fs::read_to_string(path).await?;
        let insights = self.parse_markdown_insights(&content);
        let mut result = ImportResult::default();

        for insight in &insights {
            // Check for duplicates
            if self.oxios_memory.is_duplicate(&insight.summary).await {
                result.skipped_duplicates += 1;
                continue;
            }

            let entry = MemoryEntry {
                id: format!(
                    "auto-{}-{}",
                    insight.source,
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                ),
                memory_type: MemoryType::Knowledge,
                content: match &insight.detail {
                    Some(d) => format!("{}\n\n{}", insight.summary, d),
                    None => insight.summary.clone(),
                },
                source: insight.source.clone(),
                session_id: None,
                tags: vec![insight.category.to_tag().to_string()],
                importance: insight.confidence,
                created_at: Utc::now(),
                accessed_at: Utc::now(),
                access_count: 0,
            };

            match self.oxios_memory.remember(entry).await {
                Ok(_) => result.imported += 1,
                Err(e) => {
                    result.failed += 1;
                    result.errors.push(e.to_string());
                    tracing::warn!(error = %e, "Failed to import memory insight");
                }
            }
        }

        Ok(result)
    }

    /// Parse insights from markdown content.
    ///
    /// Supports multiple formats:
    /// - List items: `- **Category**: Description`
    /// - Headers: `## Category` followed by bullet points
    /// - Free-form text split into chunks
    fn parse_markdown_insights(&self, content: &str) -> Vec<MemoryInsight> {
        let mut insights = Vec::new();
        let mut current_category = InsightCategory::General;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Detect category headers: ## Category or # Category
            if trimmed.starts_with('#') {
                let header = trimmed.trim_start_matches('#').trim().to_lowercase();
                current_category = InsightCategory::from_str_loose(&header);
                continue;
            }

            // Parse bullet list items: - **Category**: Description
            if trimmed.starts_with('-') || trimmed.starts_with('*') {
                // Strip the leading list marker and whitespace, preserving ** bold markers
                let item = if trimmed.starts_with('-') {
                    trimmed.trim_start_matches('-').trim_start()
                } else {
                    trimmed.trim_start_matches('*').trim_start()
                };

                // Check for bold category prefix: **Category**:
                if let Some(rest) = Self::extract_bold_category(item) {
                    let (cat_name, description) = rest;
                    let category = InsightCategory::from_str_loose(cat_name);

                    // Split on colon for detail
                    let (summary, detail) = if let Some(pos) = description.find(':') {
                        let s = description[..pos].trim();
                        let d = description[pos + 1..].trim();
                        (
                            s.to_string(),
                            if d.is_empty() {
                                None
                            } else {
                                Some(d.to_string())
                            },
                        )
                    } else {
                        (description.to_string(), None)
                    };

                    if !summary.is_empty() {
                        insights.push(MemoryInsight {
                            category,
                            summary,
                            detail,
                            source: "auto-import".to_string(),
                            confidence: 0.7,
                        });
                    }
                } else if !item.is_empty() {
                    // Plain bullet point
                    insights.push(MemoryInsight {
                        category: current_category.clone(),
                        summary: item.to_string(),
                        detail: None,
                        source: "auto-import".to_string(),
                        confidence: 0.6,
                    });
                }
            }
        }

        // If no structured content found, treat the whole content as a single insight
        if insights.is_empty() && !content.trim().is_empty() {
            let summary: String = content
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(200)
                .collect();

            if !summary.trim().is_empty() {
                insights.push(MemoryInsight {
                    category: InsightCategory::General,
                    summary,
                    detail: Some(content.to_string()),
                    source: "auto-import".to_string(),
                    confidence: 0.5,
                });
            }
        }

        insights
    }

    /// Extract a bold category prefix from a markdown item.
    ///
    /// Returns Some((category, rest)) if the item starts with **Category**:
    fn extract_bold_category(item: &str) -> Option<(&str, &str)> {
        if !item.starts_with("**") {
            return None;
        }

        let end = item[2..].find("**")?;
        let category = &item[2..2 + end];
        let rest = item[2 + end + 2..].trim_start_matches([' ', ':']);

        Some((category, rest))
    }

    /// Format patterns as a category-specific markdown file.
    fn format_patterns_md(&self, patterns: &[&GuidancePattern]) -> String {
        let mut md = String::new();
        md.push_str("# Memory Insights\n\n");

        for pattern in patterns {
            let confidence_bar = format_confidence_bar(pattern.confidence);
            md.push_str(&format!(
                "- **{}**: {} [{}]\n",
                pattern.category.to_tag(),
                pattern.description,
                confidence_bar,
            ));
        }

        md.push('\n');
        md
    }

    /// Format all patterns as the main MEMORY.md file.
    fn format_main_md(&self, patterns: &[&GuidancePattern]) -> String {
        let mut md = String::new();
        md.push_str("# Oxios Memory\n\n");
        md.push_str(&format!(
            "Auto-generated at {}\n\n",
            Utc::now().to_rfc3339()
        ));
        md.push_str("## Insights\n\n");

        for pattern in patterns {
            let confidence_pct = (pattern.confidence * 100.0) as u8;
            md.push_str(&format!(
                "- **{}** [{}%]: {}\n",
                pattern.category.to_tag(),
                confidence_pct,
                pattern.description,
            ));
        }

        md.push('\n');
        md
    }
}

/// Format a confidence value as a visual bar.
fn format_confidence_bar(confidence: f32) -> String {
    let bars = (confidence * 5.0).round() as usize;
    let bars = bars.min(5);
    let filled: String = "█".repeat(bars);
    let empty: String = "░".repeat(5 - bars);
    format!("{}{} {:.0}%", filled, empty, confidence * 100.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_bridge(dir: &Path) -> AutoMemoryBridge {
        let store = Arc::new(crate::state_store::StateStore::new(dir.join("state")).unwrap());
        let memory = Arc::new(MemoryManager::new(store));
        AutoMemoryBridge::new(dir.join("auto"), memory)
            .with_knowledge_base(Arc::new(
                oxios_markdown::KnowledgeBase::new(dir.join("kb")).unwrap(),
            ))
    }

    #[test]
    fn test_parse_bold_category() {
        let result =
            AutoMemoryBridge::extract_bold_category("**Debugging**: Use trace-level logging");
        assert!(result.is_some());
        let (cat, rest) = result.unwrap();
        assert_eq!(cat, "Debugging");
        assert_eq!(rest, "Use trace-level logging");
    }

    #[test]
    fn test_parse_bold_category_no_colon() {
        let result = AutoMemoryBridge::extract_bold_category("**Security** important rule");
        assert!(result.is_some());
        let (cat, rest) = result.unwrap();
        assert_eq!(cat, "Security");
        assert_eq!(rest, "important rule");
    }

    #[test]
    fn test_parse_no_bold() {
        let result = AutoMemoryBridge::extract_bold_category("Just a plain item");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_markdown_insights() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bridge = make_bridge(temp_dir.path());

        let md = r#"# Project Patterns

- **Debugging**: Use trace-level logging for async tasks
- **Architecture**: Follow the kernel module pattern
- **Performance**: Batch embeddings when possible

## Security

- Always validate input at the boundary
- Use RBAC for multi-agent access control
"#;

        let insights = bridge.parse_markdown_insights(md);
        assert!(!insights.is_empty());

        // Should have parsed the bullet points
        let debugging = insights
            .iter()
            .find(|i| i.category == InsightCategory::Debugging);
        assert!(debugging.is_some());

        let security = insights
            .iter()
            .find(|i| i.category == InsightCategory::Security);
        assert!(security.is_some());
    }

    #[test]
    fn test_parse_empty_markdown() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bridge = make_bridge(temp_dir.path());

        let insights = bridge.parse_markdown_insights("");
        assert!(insights.is_empty());
    }

    #[test]
    fn test_parse_plain_text_as_single_insight() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bridge = make_bridge(temp_dir.path());

        let text = "This is a plain text memory entry without markdown formatting.";
        let insights = bridge.parse_markdown_insights(text);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].category, InsightCategory::General);
    }

    #[test]
    fn test_insight_category_parsing() {
        assert_eq!(
            InsightCategory::from_str_loose("patterns"),
            InsightCategory::ProjectPatterns
        );
        assert_eq!(
            InsightCategory::from_str_loose("debug"),
            InsightCategory::Debugging
        );
        assert_eq!(
            InsightCategory::from_str_loose("arch"),
            InsightCategory::Architecture
        );
        assert_eq!(
            InsightCategory::from_str_loose("perf"),
            InsightCategory::Performance
        );
        assert_eq!(
            InsightCategory::from_str_loose("sec"),
            InsightCategory::Security
        );
        assert_eq!(
            InsightCategory::from_str_loose("unknown"),
            InsightCategory::General
        );
    }

    #[test]
    fn test_confidence_bar() {
        let bar = format_confidence_bar(0.8);
        assert!(bar.contains("80%"));

        let bar_low = format_confidence_bar(0.2);
        assert!(bar_low.contains("20%"));
    }

    #[tokio::test]
    async fn test_import_from_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bridge = make_bridge(temp_dir.path());

        // No files to import
        let result = bridge.import_from_auto().await.unwrap();
        assert_eq!(result.imported, 0);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn test_import_from_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let auto_dir = temp_dir.path().join("auto");
        tokio::fs::create_dir_all(&auto_dir).await.unwrap();

        // Create a MEMORY.md file
        let md = r#"- **Debugging**: Use println! for quick debugging
- **Architecture**: Keep modules small and focused
"#;
        tokio::fs::write(auto_dir.join("MEMORY.md"), md)
            .await
            .unwrap();

        let bridge = make_bridge(temp_dir.path());
        let result = bridge.import_from_auto().await.unwrap();
        assert!(result.imported >= 1, "Should import at least 1 insight");
    }

    #[tokio::test]
    async fn test_export_to_auto() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bridge = make_bridge(temp_dir.path());

        let patterns = vec![
            GuidancePattern {
                id: "p1".to_string(),
                category: InsightCategory::Debugging,
                description: "Always check error chains".to_string(),
                confidence: 0.9,
                usage_count: 5,
            },
            GuidancePattern {
                id: "p2".to_string(),
                category: InsightCategory::Architecture,
                description: "Use actor model for concurrency".to_string(),
                confidence: 0.7,
                usage_count: 3,
            },
        ];

        let result = bridge.export_to_auto(&patterns).await.unwrap();
        assert_eq!(result.exported, 2);
        assert!(result.categories_updated >= 2);

        // Verify files were created
        assert!(bridge.auto_memory_dir.join("MEMORY.md").exists());
        assert!(bridge.auto_memory_dir.join("debugging.md").exists());
        assert!(bridge.auto_memory_dir.join("architecture.md").exists());

        // Verify content
        let main = tokio::fs::read_to_string(bridge.auto_memory_dir.join("MEMORY.md"))
            .await
            .unwrap();
        assert!(main.contains("Oxios Memory"));
        assert!(main.contains("Always check error chains"));
        assert!(main.contains("Use actor model for concurrency"));
    }

    #[tokio::test]
    async fn test_bidirectional_sync() {
        let temp_dir = tempfile::tempdir().unwrap();
        let auto_dir = temp_dir.path().join("auto");
        tokio::fs::create_dir_all(&auto_dir).await.unwrap();

        // Create an external memory file
        tokio::fs::write(
            auto_dir.join("MEMORY.md"),
            "- **Debugging**: Test insight for sync",
        )
        .await
        .unwrap();

        let bridge = make_bridge(temp_dir.path());
        let result = bridge
            .sync_session(SyncDirection::Bidirectional)
            .await
            .unwrap();

        // Should have imported the insight
        assert!(result.import.imported >= 1 || result.import.skipped_duplicates > 0);

        // Should have exported current state (usize, always >= 0)
    }

    #[test]
    fn test_sync_direction_serialization() {
        let dir = SyncDirection::ToAuto;
        let json = serde_json::to_string(&dir).unwrap();
        assert_eq!(json, "\"to_auto\"");

        let parsed: SyncDirection = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SyncDirection::ToAuto);
    }
}
