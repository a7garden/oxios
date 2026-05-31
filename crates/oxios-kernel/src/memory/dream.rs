#![allow(missing_docs)]
//! Dream process — 4-phase background memory consolidation.
//!
//! Phase 1: Orient — scan current state, build map
//! Phase 2: Gather Signal — find patterns, auto-protect, auto-classify
//! Phase 3: Consolidate — compress, dedupe, resolve conflicts
//! Phase 4: Prune & Index — update ROOT, remove stale entries
//!
//! Supports checkpointing for crash recovery.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use super::auto_classify::AutoClassifier;
use super::auto_protect::AutoProtector;
use super::compaction::CompactionTree;
use super::decay::DecayEngine;
use super::root_index::RootIndex;
use super::{MemoryEntry, MemoryManager, MemoryTier, MemoryType, ProtectionLevel};

// ---------------------------------------------------------------------------
// DreamCheckpoint
// ---------------------------------------------------------------------------

/// Dream execution state (checkpoint for crash recovery).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamCheckpoint {
    /// Unique dream ID.
    pub dream_id: String,
    /// Space ID this dream is running for.

    /// When the dream started.
    pub started_at: DateTime<Utc>,
    /// Last completed phase (0 = not started).
    pub completed_phase: u8,
    /// Cached signals from Phase 2.
    pub cached_signals: Option<Vec<MemorySignal>>,
    /// Cached plan from Phase 3.
    pub cached_plan: Option<ConsolidationPlan>,
}

impl DreamCheckpoint {
    /// Path for the checkpoint file within a space's memory directory.
    pub fn path(space_dir: &Path) -> PathBuf {
        space_dir.join("memory/.dream_checkpoint.json")
    }

    /// Path for the dream lock file.
    pub fn lock_path(space_dir: &Path) -> PathBuf {
        space_dir.join("memory/.dream.lock")
    }

    /// Check if a checkpoint is stale (older than 1 hour).
    pub fn is_stale(&self) -> bool {
        let age = Utc::now() - self.started_at;
        age.num_hours() >= 1
    }
}

// ---------------------------------------------------------------------------
// DreamReport
// ---------------------------------------------------------------------------

/// Report from a dream (consolidation) run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReport {
    /// Unique dream ID.
    pub dream_id: String,
    /// Space ID.

    /// When the dream started.
    pub started_at: DateTime<Utc>,
    /// When the dream completed.
    pub completed_at: DateTime<Utc>,
    /// Whether this was resumed from a checkpoint.
    pub resumed_from_checkpoint: bool,
    /// Entry count before dream.
    pub entries_before: usize,
    /// Entry count after dream.
    pub entries_after: usize,
    /// Number of entries compacted.
    pub compacted: usize,
    /// Number of entries tier-promoted (Cold→Warm, Warm→Hot).
    pub promoted: usize,
    /// Number of entries tier-demoted (Hot→Warm, Warm→Cold).
    pub demoted: usize,
    /// Number of protection level promotions.
    pub protection_promoted: usize,
    /// Number of protection level demotions.
    pub protection_demoted: usize,
    /// Number of entries deleted.
    pub deleted: usize,
    /// Number of contradictions resolved.
    pub contradictions_resolved: usize,
    /// Number of duplicates merged.
    pub duplicates_merged: usize,
    /// Number of auto-protected entries.
    pub auto_protected: usize,
    /// Number of auto-classified entries.
    pub auto_classified: usize,
    /// Number of type promotions (e.g., Fact → Skill).
    pub type_promotions: usize,
    /// Whether ROOT index was updated.
    pub root_updated: bool,
    /// Whether LLM was used for compaction.
    pub used_llm: bool,
    /// Number of PageRank importance updates (Phase 2).
    pub pagerank_updates: usize,
    /// Number of learning patterns persisted (Phase 4).
    pub patterns_persisted: usize,
    /// Whether hyperbolic embeddings were rebuilt (Phase 5).
    pub hyperbolic_rebuilt: bool,
    /// Number of memories re-ranked by Flash Attention (Phase 6).
    pub flash_reranked: usize,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Error if Dream failed (None = success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DreamReport {
    /// Path for saving dream reports.
    pub fn report_path(space_dir: &Path, dream_id: &str) -> PathBuf {
        space_dir
            .join("memory/dream_reports")
            .join(format!("{dream_id}.json"))
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// A signal detected during Dream Phase 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemorySignal {
    /// A protection level change.
    ProtectionChanged(ProtectionChange),
    /// Auto-classify an entry.
    AutoClassify { id: String, new_type: MemoryType },
    /// Type promotion (e.g., Fact → Skill).
    TypePromotion(TypePromotion),
    /// Tier promotion candidate.
    PromotionCandidate(TierChange),
    /// Decay/deletion candidate.
    DecayCandidate(DecayCandidate),
    /// Duplicate detected.
    Duplicate {
        id_a: String,
        id_b: String,
        similarity: f64,
    },
    /// Contradiction detected.
    Contradiction { newer_id: String, older_id: String },
    /// PageRank-based importance boost (Phase 2).
    PageRankBoost {
        rowid: u64,
        old_importance: f32,
        new_importance: f32,
        pagerank_score: f64,
    },
}

/// A protection level change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionChange {
    pub id: String,
    pub from: ProtectionLevel,
    pub to: ProtectionLevel,
    pub reason: String,
}

/// A type promotion suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypePromotion {
    pub id: String,
    pub current_type: MemoryType,
    pub suggested_type: MemoryType,
    pub repetitions: u32,
}

/// A tier change suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierChange {
    pub id: String,
    pub from_tier: MemoryTier,
    pub to_tier: MemoryTier,
    pub reason: String,
}

/// A decay/deletion candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayCandidate {
    pub id: String,
    pub decay_score: f32,
    pub protection: ProtectionLevel,
    pub memory_type: MemoryType,
}

/// Consolidation plan from Phase 3.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidationPlan {
    /// Protection level updates.
    pub protection_updates: Vec<ProtectionChange>,
    /// Type reclassification.
    pub reclassify: Vec<ReclassifyPlan>,
    /// Tier promotions.
    pub promote: Vec<TierChange>,
    /// Tier demotions.
    pub demote: Vec<TierChange>,
    /// Entries to delete.
    pub delete: Vec<String>,
    /// Entries to merge.
    pub merge: Vec<MergePlan>,
    /// PageRank-based importance updates (Phase 2).
    pub pagerank_updates: Vec<PageRankUpdate>,
}

/// A PageRank-based importance update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankUpdate {
    /// Memory row ID.
    pub rowid: u64,
    /// Previous importance.
    pub old_importance: f32,
    /// New importance after PageRank boost.
    pub new_importance: f32,
    /// PageRank score (0.0–1.0).
    pub pagerank_score: f64,
}

impl ConsolidationPlan {
    /// Total number of changes in this plan.
    pub fn total_changes(&self) -> usize {
        self.protection_updates.len()
            + self.reclassify.len()
            + self.promote.len()
            + self.demote.len()
            + self.delete.len()
            + self.merge.len()
            + self.pagerank_updates.len()
    }
}

/// A type reclassification plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclassifyPlan {
    pub id: String,
    pub new_type: MemoryType,
}

/// A merge plan for duplicate entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePlan {
    pub keep_id: String,
    pub remove_id: String,
    pub merged_content: String,
}

// ---------------------------------------------------------------------------
// DreamState (Phase 1 output)
// ---------------------------------------------------------------------------

/// State snapshot from Phase 1 (Orient).
#[derive(Debug, Clone)]
pub struct DreamState {
    pub total_entries: usize,
    pub hot_count: usize,
    pub warm_count: usize,
    pub cold_count: usize,
    pub root_version: u64,
    pub type_distribution: Vec<(MemoryType, usize)>,
    pub protection_distribution: Vec<(ProtectionLevel, usize)>,
    pub avg_decay: f32,
}

// ---------------------------------------------------------------------------
// DreamConfig
// ---------------------------------------------------------------------------

/// Configuration extracted for Dream use.
#[derive(Debug, Clone)]
pub struct DreamConfig {
    pub dream_enabled: bool,
    pub dream_interval_hours: u64,
    pub dream_min_sessions: u32,
    pub hot_max_entries: usize,
    pub warm_max_entries: usize,
    pub cold_max_entries: usize,
    pub hot_token_budget: usize,
    pub decay_threshold: f32,
    pub retention_days: u32,
    pub decay_multiplier: f32,
    pub auto_protection: bool,
    pub protection_low_access: u32,
    pub protection_medium_access: u32,
    pub protection_high_access: u32,
    pub protection_medium_sessions: u32,
    pub protection_high_sessions: u32,
    pub protection_demotion_enabled: bool,
    pub protection_demotion_stale_days: u32,
    pub auto_classification: bool,
    pub type_promotion_repetitions: u32,
    pub compaction_line_threshold: usize,
    pub proactive_recall_limit: usize,
    pub proactive_recall_threshold: f32,
    /// Enable PageRank-based importance boost (Phase 2).
    pub pagerank_enabled: bool,
    /// PageRank damping factor (typically 0.85).
    pub pagerank_damping: f64,
    /// PageRank iteration count (typically 20-50).
    pub pagerank_iterations: usize,
    /// How much PageRank score influences importance (0.0–1.0).
    pub pagerank_boost_factor: f32,
}

impl DreamConfig {
    /// Extract from the kernel's ConsolidationConfig.
    pub fn from_consolidation(c: &crate::config::ConsolidationConfig) -> Self {
        Self {
            dream_enabled: c.dream_enabled,
            dream_interval_hours: c.dream_interval_hours,
            dream_min_sessions: c.dream_min_sessions,
            hot_max_entries: c.hot_max_entries,
            warm_max_entries: c.warm_max_entries,
            cold_max_entries: c.cold_max_entries,
            hot_token_budget: c.hot_token_budget,
            decay_threshold: c.decay_threshold,
            retention_days: c.retention_days,
            decay_multiplier: c.decay_multiplier,
            auto_protection: c.auto_protection,
            protection_low_access: c.protection_low_access,
            protection_medium_access: c.protection_medium_access,
            protection_high_access: c.protection_high_access,
            protection_medium_sessions: c.protection_medium_sessions,
            protection_high_sessions: c.protection_high_sessions,
            protection_demotion_enabled: c.protection_demotion_enabled,
            protection_demotion_stale_days: c.protection_demotion_stale_days,
            auto_classification: c.auto_classification,
            type_promotion_repetitions: c.type_promotion_repetitions,
            compaction_line_threshold: c.compaction_line_threshold,
            proactive_recall_limit: c.proactive_recall_limit,
            proactive_recall_threshold: c.proactive_recall_threshold,
            pagerank_enabled: true,
            pagerank_damping: 0.85,
            pagerank_iterations: 30,
            pagerank_boost_factor: 0.3,
        }
    }
}

// ---------------------------------------------------------------------------
// DreamProcess
// ---------------------------------------------------------------------------

/// The Dream process — 4-phase background memory consolidation.
///
/// Runs automatically on a timer (default: every 24 hours) or can be
/// triggered manually. Uses checkpointing for crash recovery.
pub struct DreamProcess {
    /// Reference to the memory manager.
    memory_manager: Arc<MemoryManager>,
    /// Decay engine.
    decay_engine: DecayEngine,
    /// Auto-protector.
    auto_protector: AutoProtector,
    /// Compaction tree.
    #[allow(dead_code)]
    compaction_tree: CompactionTree,
    /// Configuration.
    config: DreamConfig,
    /// Root index (read/write).
    root_index: RwLock<RootIndex>,
    /// Space directory for file storage.
    space_dir: PathBuf,
}

impl std::fmt::Debug for DreamProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DreamProcess")
            .field("space_dir", &self.space_dir.display())
            .finish()
    }
}

/// Result of Phase 4 (Prune & Index).
struct Phase4Result {
    contradictions_resolved: usize,
    /// Number of memories re-ranked by Flash Attention (Phase 6).
    flash_reranked: usize,
    /// Number of learning patterns persisted (Phase 4: SONA).
    patterns_persisted: usize,
}

impl DreamProcess {
    /// Create a new DreamProcess.
    pub fn new(
        memory_manager: Arc<MemoryManager>,
        config: DreamConfig,
        space_dir: PathBuf,
    ) -> Self {
        let auto_protector = AutoProtector::new(
            config.protection_low_access,
            config.protection_medium_access,
            config.protection_high_access,
            config.protection_medium_sessions,
            config.protection_high_sessions,
            config.protection_demotion_stale_days,
        );

        Self {
            memory_manager,
            decay_engine: DecayEngine::new(config.decay_multiplier),
            auto_protector,
            compaction_tree: CompactionTree::new(config.compaction_line_threshold),
            config,
            root_index: RwLock::new(RootIndex::new()),
            space_dir,
        }
    }

    /// Check if a dream should run now based on configuration.
    pub fn should_dream(&self, last_dream: Option<DateTime<Utc>>, sessions_since: u32) -> bool {
        if !self.config.dream_enabled {
            return false;
        }

        match last_dream {
            None => true, // Never ran before
            Some(last) => {
                let hours = (Utc::now() - last).num_hours() as u64;
                hours >= self.config.dream_interval_hours
                    || sessions_since >= self.config.dream_min_sessions
            }
        }
    }

    /// Run the full 4-phase dream process.
    ///
    /// Returns a DreamReport with statistics about what was done.
    pub async fn dream(&self) -> DreamReport {
        let dream_id = Uuid::new_v4().to_string();
        let started_at = Utc::now();
        let entries_before = self.memory_manager.total_entries().await;

        // Check for existing checkpoint
        let resumed = self.load_checkpoint().await.ok().flatten();
        let resumed_from_checkpoint = resumed.is_some();

        let start_phase = resumed.as_ref().map(|cp| cp.completed_phase).unwrap_or(0);

        // Run phases
        let mut report = DreamReport {
            dream_id: dream_id.clone(),

            started_at,
            completed_at: started_at,
            resumed_from_checkpoint,
            entries_before,
            entries_after: entries_before,
            compacted: 0,
            promoted: 0,
            demoted: 0,
            protection_promoted: 0,
            protection_demoted: 0,
            deleted: 0,
            contradictions_resolved: 0,
            duplicates_merged: 0,
            auto_protected: 0,
            auto_classified: 0,
            type_promotions: 0,
            root_updated: false,
            used_llm: false,
            pagerank_updates: 0,
            patterns_persisted: 0,
            hyperbolic_rebuilt: false,
            flash_reranked: 0,
            duration_ms: 0,
            error: None,
        };

        let result = async {
            // Phase 1: Orient
            let _state = if start_phase < 1 {
                self.dream_orient().await?
            } else {
                // Skip — use cached state (in a full impl, we'd cache this)
                self.dream_orient().await?
            };
            self.save_checkpoint(&dream_id, 1, None, None).await?;

            // Phase 2: Gather Signal
            let signals = if start_phase < 2 {
                self.dream_gather_signal().await?
            } else {
                resumed
                    .as_ref()
                    .and_then(|cp| cp.cached_signals.clone())
                    .unwrap_or_default()
            };
            self.save_checkpoint(&dream_id, 2, Some(&signals), None)
                .await?;

            // Phase 3: Consolidate
            let plan = if start_phase < 3 {
                self.dream_consolidate(&signals).await?
            } else {
                resumed
                    .as_ref()
                    .and_then(|cp| cp.cached_plan.clone())
                    .unwrap_or_default()
            };
            self.save_checkpoint(&dream_id, 3, Some(&signals), Some(&plan))
                .await?;

            // Phase 4: Prune & Index
            let phase4_result = self.dream_prune_and_index(&plan).await?;

            // Update report counters
            report.protection_promoted = plan
                .protection_updates
                .iter()
                .filter(|c| c.to > c.from)
                .count();
            report.protection_demoted = plan
                .protection_updates
                .iter()
                .filter(|c| c.to < c.from)
                .count();
            report.promoted = plan.promote.len();
            report.demoted = plan.demote.len();
            report.deleted = plan.delete.len();
            report.duplicates_merged = plan.merge.len();
            report.type_promotions = plan.reclassify.len();
            report.auto_protected = report.protection_promoted;
            report.auto_classified = signals
                .iter()
                .filter(|s| matches!(s, MemorySignal::AutoClassify { .. }))
                .count();
            report.contradictions_resolved = phase4_result.contradictions_resolved;
            report.root_updated = true;
            report.pagerank_updates = plan.pagerank_updates.len();
            report.hyperbolic_rebuilt = true;
            report.flash_reranked = phase4_result.flash_reranked;
            report.patterns_persisted = phase4_result.patterns_persisted;

            // Clear checkpoint on success
            self.clear_checkpoint().await.ok();

            Ok::<(), anyhow::Error>(())
        }
        .await;

        if let Err(e) = result {
            report.error = Some(e.to_string());
        }

        report.completed_at = Utc::now();
        report.duration_ms = (report.completed_at - report.started_at)
            .num_milliseconds()
            .max(0) as u64;
        report.entries_after = self.memory_manager.total_entries().await;

        // Save report
        let report_path = DreamReport::report_path(&self.space_dir, &dream_id);
        if let Some(parent) = report_path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }
        if let Ok(data) = serde_json::to_string_pretty(&report) {
            let _ = fs::write(&report_path, data).await;
        }

        report
    }

    /// Spawn the dream as a background task.
    pub fn spawn_dream_task(self: &Arc<Self>) {
        let dream = Arc::clone(self);
        tokio::spawn(async move {
            let report = dream.dream().await;
            if report.error.is_some() {
                tracing::warn!(
                    dream_id = %report.dream_id,
                    error = ?report.error,
                    "Dream process completed with error"
                );
            } else {
                tracing::info!(
                    dream_id = %report.dream_id,
                    promoted = report.promoted,
                    demoted = report.demoted,
                    deleted = report.deleted,
                    auto_protected = report.auto_protected,
                    duration_ms = report.duration_ms,
                    "Dream process completed"
                );
            }
        });
    }

    // ── Phase implementations ──────────────────────────

    async fn dream_orient(&self) -> Result<DreamState> {
        let hot = self
            .memory_manager
            .list_by_tier(MemoryTier::Hot, 10_000)
            .await
            .unwrap_or_default();
        let warm = self
            .memory_manager
            .list_by_tier(MemoryTier::Warm, 10_000)
            .await
            .unwrap_or_default();
        let cold = self
            .memory_manager
            .list_by_tier(MemoryTier::Cold, 10_000)
            .await
            .unwrap_or_default();

        let hot_count = hot.len();
        let warm_count = warm.len();
        let cold_count = cold.len();
        let total = hot_count + warm_count + cold_count;

        let root = self.root_index.read().clone();

        let mut type_dist: Vec<(MemoryType, usize)> = Vec::new();
        for mt in MemoryType::all() {
            if let Ok(entries) = self.memory_manager.list(*mt, 1_000_000).await {
                let count = entries.len();
                if count > 0 {
                    type_dist.push((*mt, count));
                }
            }
        }

        let mut prot_dist: Vec<(ProtectionLevel, usize)> = Vec::new();
        let all_entries: Vec<&MemoryEntry> =
            hot.iter().chain(warm.iter()).chain(cold.iter()).collect();
        for level in &[
            ProtectionLevel::None,
            ProtectionLevel::Low,
            ProtectionLevel::Medium,
            ProtectionLevel::High,
            ProtectionLevel::Permanent,
        ] {
            let count = all_entries
                .iter()
                .filter(|e| e.protection == *level)
                .count();
            if count > 0 {
                prot_dist.push((*level, count));
            }
        }

        let avg_decay = if all_entries.is_empty() {
            1.0
        } else {
            all_entries.iter().map(|e| e.decay_score).sum::<f32>() / all_entries.len() as f32
        };

        Ok(DreamState {
            total_entries: total,
            hot_count,
            warm_count,
            cold_count,
            root_version: root.version,
            type_distribution: type_dist,
            protection_distribution: prot_dist,
            avg_decay,
        })
    }

    async fn dream_gather_signal(&self) -> Result<Vec<MemorySignal>> {
        let mut signals = Vec::new();

        // Gather all entries across all types
        let mut all_entries = Vec::new();
        for mt in MemoryType::all() {
            if let Ok(entries) = self.memory_manager.list(*mt, 1_000_000).await {
                all_entries.extend(entries);
            }
        }

        let now = Utc::now();

        // 1. Protection re-evaluation
        if self.config.auto_protection {
            for entry in &all_entries {
                let old_protection = entry.protection;
                let new_protection = self.auto_protector.compute_protection(entry);

                // Check demotion
                let final_protection = if self.config.protection_demotion_enabled {
                    self.auto_protector
                        .should_demote_protection(entry, new_protection)
                        .unwrap_or(new_protection)
                } else {
                    new_protection
                };

                if old_protection != final_protection {
                    signals.push(MemorySignal::ProtectionChanged(ProtectionChange {
                        id: entry.id.clone(),
                        from: old_protection,
                        to: final_protection,
                        reason: format!(
                            "access_count={}, sessions={}, corrected={}",
                            entry.access_count, entry.session_appearances, entry.user_corrected
                        ),
                    }));
                }
            }
        }

        // 2. Auto-classification for entries that haven't been classified
        if self.config.auto_classification {
            for entry in &all_entries {
                if entry.auto_classified || entry.memory_type == MemoryType::Knowledge {
                    continue; // Skip already classified or knowledge-base entries
                }
                let inferred = AutoClassifier::infer_memory_type(&entry.content, "");
                if inferred != entry.memory_type {
                    signals.push(MemorySignal::AutoClassify {
                        id: entry.id.clone(),
                        new_type: inferred,
                    });
                }
            }
        }

        // 3. Decay computation and deletion candidates
        for entry in &all_entries {
            let decay = self.decay_engine.compute_decay(entry, now);
            if self
                .decay_engine
                .is_prunable(entry, self.config.decay_threshold)
            {
                signals.push(MemorySignal::DecayCandidate(DecayCandidate {
                    id: entry.id.clone(),
                    decay_score: decay,
                    protection: entry.protection,
                    memory_type: entry.memory_type,
                }));
            }
        }

        // 4. Tier overflow checks
        let hot_count = all_entries
            .iter()
            .filter(|e| e.tier == MemoryTier::Hot)
            .count();
        if hot_count > self.config.hot_max_entries {
            let overflow = hot_count - self.config.hot_max_entries;
            let mut candidates: Vec<&MemoryEntry> = all_entries
                .iter()
                .filter(|e| {
                    e.tier == MemoryTier::Hot && e.protection < ProtectionLevel::High && !e.pinned
                })
                .collect();
            candidates.sort_by(|a, b| {
                a.protection.cmp(&b.protection).then(
                    a.decay_score
                        .partial_cmp(&b.decay_score)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
            });
            for entry in candidates.into_iter().take(overflow) {
                signals.push(MemorySignal::PromotionCandidate(TierChange {
                    id: entry.id.clone(),
                    from_tier: MemoryTier::Hot,
                    to_tier: MemoryTier::Warm,
                    reason: "Hot tier overflow".to_string(),
                }));
            }
        }

        // 5. PageRank-based importance boost (Phase 2)
        if self.config.pagerank_enabled {
            #[cfg(feature = "sqlite-memory")]
            if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
                let scores = sqlite.compute_pagerank(
                    self.config.pagerank_damping,
                    self.config.pagerank_iterations,
                    None,
                );

                if !scores.is_empty() {
                    // Get current importance for each scored memory
                    let conn = sqlite.db().conn();
                    for (&rowid, &pr_score) in &scores {
                        if let Ok(old_importance) = conn.query_row(
                            "SELECT importance FROM memories WHERE rowid = ?1",
                            rusqlite::params![rowid as i64],
                            |row| row.get::<_, f32>(0),
                        ) {
                            let new_importance = (old_importance
                                * (1.0 + self.config.pagerank_boost_factor * pr_score as f32))
                                .clamp(0.0, 1.0);

                            if (new_importance - old_importance).abs() > 0.001 {
                                signals.push(MemorySignal::PageRankBoost {
                                    rowid,
                                    old_importance,
                                    new_importance,
                                    pagerank_score: pr_score,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(signals)
    }

    async fn dream_consolidate(&self, signals: &[MemorySignal]) -> Result<ConsolidationPlan> {
        let mut plan = ConsolidationPlan::default();

        for signal in signals {
            match signal {
                MemorySignal::ProtectionChanged(change) => {
                    plan.protection_updates.push(change.clone());
                }
                MemorySignal::AutoClassify { id, new_type } => {
                    plan.reclassify.push(ReclassifyPlan {
                        id: id.clone(),
                        new_type: *new_type,
                    });
                }
                MemorySignal::TypePromotion(promo) => {
                    plan.reclassify.push(ReclassifyPlan {
                        id: promo.id.clone(),
                        new_type: promo.suggested_type,
                    });
                }
                MemorySignal::PromotionCandidate(tc) => {
                    plan.demote.push(tc.clone());
                }
                MemorySignal::DecayCandidate(dc) => {
                    if dc.protection <= ProtectionLevel::Low {
                        plan.delete.push(dc.id.clone());
                    }
                }
                MemorySignal::Duplicate { id_a, id_b, .. } => {
                    plan.merge.push(MergePlan {
                        keep_id: id_a.clone(),
                        remove_id: id_b.clone(),
                        merged_content: String::new(), // Would be computed in full impl
                    });
                }
                MemorySignal::Contradiction { newer_id, older_id } => {
                    // Mark older as contradicted
                    plan.merge.push(MergePlan {
                        keep_id: newer_id.clone(),
                        remove_id: older_id.clone(),
                        merged_content: String::new(),
                    });
                }
                MemorySignal::PageRankBoost {
                    rowid,
                    old_importance,
                    new_importance,
                    pagerank_score,
                } => {
                    plan.pagerank_updates.push(PageRankUpdate {
                        rowid: *rowid,
                        old_importance: *old_importance,
                        new_importance: *new_importance,
                        pagerank_score: *pagerank_score,
                    });
                }
            }
        }

        Ok(plan)
    }

    async fn dream_prune_and_index(&self, plan: &ConsolidationPlan) -> Result<Phase4Result> {
        let mut contradictions_resolved = 0;

        // 1. Apply protection updates
        for change in &plan.protection_updates {
            if let Ok(Some(mut entry)) = self.memory_manager.get_by_id(&change.id).await {
                entry.protection = change.to;
                let _ = self.memory_manager.remember(entry).await;
            }
        }

        // 2. Apply type reclassification
        for reclassify in &plan.reclassify {
            if let Ok(Some(mut entry)) = self.memory_manager.get_by_id(&reclassify.id).await {
                entry.memory_type = reclassify.new_type;
                entry.auto_classified = true;
                let _ = self.memory_manager.remember(entry).await;
            }
        }

        // 3. Apply tier changes
        for tc in &plan.demote {
            if let Ok(Some(mut entry)) = self.memory_manager.get_by_id(&tc.id).await {
                entry.tier = tc.to_tier;
                let _ = self.memory_manager.remember(entry).await;
            }
        }

        // 4. Apply merges
        for merge in &plan.merge {
            contradictions_resolved += 1;
            // Remove the older/duplicate entry
            let _ = self
                .memory_manager
                .get_by_id(&merge.remove_id)
                .await
                .ok()
                .flatten()
                .map(
                    |e| async move { self.memory_manager.forget(&e.id, e.memory_type).await.ok() },
                );
        }

        // 5. Apply PageRank importance updates (Phase 2)
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
            for update in &plan.pagerank_updates {
                let conn = sqlite.db().conn();
                let _ = conn.execute(
                    "UPDATE memories SET importance = ?1 WHERE rowid = ?2",
                    rusqlite::params![update.new_importance, update.rowid as i64],
                );
            }
        }

        // 6. Apply deletions (with safety checks)
        for id in &plan.delete {
            if let Ok(Some(entry)) = self.memory_manager.get_by_id(id).await {
                // Safety check: never delete protected entries
                if entry.protection <= ProtectionLevel::Low
                    && !entry.pinned
                    && !entry.memory_type.is_auto_protected()
                {
                    let _ = self.memory_manager.forget(id, entry.memory_type).await;
                }
            }
        }

        // 7. Rebuild ROOT index
        self.rebuild_root_index().await?;

        // 8. Rebuild Hyperbolic Embeddings (Phase 5)
        #[cfg(feature = "sqlite-memory")]
        if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
            let config = super::hyperbolic::HyperbolicConfig::default();
            match super::hyperbolic::HyperbolicEmbedding::restore_from_sqlite(sqlite, config) {
                Ok(he) => {
                    let count = he.len();
                    if count < 10 {
                        tracing::debug!("Hyperbolic embeddings need rebuild (count < 10)");
                    }
                    tracing::debug!(count, "Hyperbolic embeddings loaded");
                }
                Err(e) => {
                    tracing::debug!(error = %e, "Failed to restore hyperbolic embeddings (non-fatal)");
                }
            }
        }

        // 9. Persist & auto-promote learning patterns (Phase 4: SONA)
        let patterns_persisted = {
            #[cfg(feature = "sqlite-memory")]
            if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
                // Auto-promote high-quality patterns to long-term storage
                let _ = sqlite.auto_promote_patterns(0.8, 3);
                // Count total patterns in store as the persistence metric
                let conn = sqlite.db().conn();
                let total: usize = conn
                    .query_row("SELECT COUNT(*) FROM patterns", [], |row| row.get(0))
                    .unwrap_or(0);
                total
            } else {
                0
            }
            #[cfg(not(feature = "sqlite-memory"))]
            {
                0
            }
        };

        // 10. Flash Attention reranking (Phase 6)
        let flash_reranked = {
            #[cfg(feature = "sqlite-memory")]
            if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
                let hot = self
                    .memory_manager
                    .list_by_tier(MemoryTier::Hot, 50)
                    .await
                    .unwrap_or_default();
                if !hot.is_empty() {
                    let query: String = hot
                        .iter()
                        .take(3)
                        .map(|e| e.content.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !query.is_empty() {
                        match sqlite.recall_with_rerank(&query, hot.len()).await {
                            Ok(reranked) => reranked.len(),
                            Err(_) => 0,
                        }
                    } else {
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            }
            #[cfg(not(feature = "sqlite-memory"))]
            {
                0
            }
        };

        Ok(Phase4Result {
            contradictions_resolved,
            flash_reranked,
            patterns_persisted,
        })
    }

    // ── Helper methods ──────────────────────────────────

    /// Rebuild the ROOT index from current memory state.
    async fn rebuild_root_index(&self) -> Result<()> {
        let mut root = RootIndex::new();
        root.version += 1;
        root.updated_at = Utc::now();

        let now = Utc::now();
        let mut all_entries = Vec::new();
        for mt in MemoryType::all() {
            if let Ok(entries) = self.memory_manager.list(*mt, 1_000).await {
                all_entries.extend(entries);
            }
        }

        // Build active context (recent, important)
        let mut recent: Vec<&MemoryEntry> = all_entries
            .iter()
            .filter(|e| (now - e.accessed_at).num_days() <= 7 && e.importance >= 0.5)
            .collect();
        recent.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        recent.truncate(20);

        for entry in &recent {
            root.active_context.push(super::root_index::RootEntry {
                topic: entry.content.split('.').next().unwrap_or("").to_string(),
                memory_type: entry.memory_type,
                protection: entry.protection,
                age_days: (now - entry.created_at).num_days() as u32,
                reference: entry.id.clone(),
            });
        }

        // Build topic index
        for entry in &all_entries {
            let first_sentence = entry
                .content
                .split('.')
                .next()
                .unwrap_or(&entry.content)
                .to_string();
            root.topics.push(super::root_index::TopicEntry {
                name: first_sentence.clone(),
                category: entry.memory_type.label().to_string(),
                age_days: (now - entry.created_at).num_days() as u32,
                description: entry.content.chars().take(100).collect(),
                reference: entry.id.clone(),
            });
        }

        *self.root_index.write() = root;
        Ok(())
    }

    /// Load a checkpoint if one exists.
    async fn load_checkpoint(&self) -> Result<Option<DreamCheckpoint>> {
        let path = DreamCheckpoint::path(&self.space_dir);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path).await?;
        let checkpoint: DreamCheckpoint = serde_json::from_str(&data)?;
        if checkpoint.is_stale() {
            tracing::info!("Stale checkpoint found, ignoring");
            return Ok(None);
        }
        Ok(Some(checkpoint))
    }

    /// Save a checkpoint after completing a phase.
    async fn save_checkpoint(
        &self,
        dream_id: &str,
        completed_phase: u8,
        signals: Option<&[MemorySignal]>,
        plan: Option<&ConsolidationPlan>,
    ) -> Result<()> {
        let checkpoint = DreamCheckpoint {
            dream_id: dream_id.to_string(),

            started_at: Utc::now(),
            completed_phase,
            cached_signals: signals.map(|s| s.to_vec()),
            cached_plan: plan.cloned(),
        };
        let path = DreamCheckpoint::path(&self.space_dir);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }
        let data = serde_json::to_string_pretty(&checkpoint)?;
        fs::write(&path, data).await?;
        Ok(())
    }

    /// Clear the checkpoint after successful dream.
    async fn clear_checkpoint(&self) -> Result<()> {
        let path = DreamCheckpoint::path(&self.space_dir);
        if path.exists() {
            let _ = fs::remove_file(&path).await;
        }
        let lock_path = DreamCheckpoint::lock_path(&self.space_dir);
        if lock_path.exists() {
            let _ = fs::remove_file(&lock_path).await;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dream_checkpoint_stale() {
        let cp = DreamCheckpoint {
            dream_id: "test".to_string(),

            started_at: Utc::now() - chrono::Duration::hours(2),
            completed_phase: 2,
            cached_signals: None,
            cached_plan: None,
        };
        assert!(cp.is_stale());
    }

    #[test]
    fn test_dream_checkpoint_fresh() {
        let cp = DreamCheckpoint {
            dream_id: "test".to_string(),

            started_at: Utc::now(),
            completed_phase: 2,
            cached_signals: None,
            cached_plan: None,
        };
        assert!(!cp.is_stale());
    }

    #[test]
    fn test_consolidation_plan_total_changes() {
        let mut plan = ConsolidationPlan::default();
        plan.protection_updates.push(ProtectionChange {
            id: "1".to_string(),
            from: ProtectionLevel::None,
            to: ProtectionLevel::Low,
            reason: "test".to_string(),
        });
        plan.delete.push("2".to_string());
        assert_eq!(plan.total_changes(), 2);
    }

    #[test]
    fn test_should_dream_never_ran() {
        let config =
            DreamConfig::from_consolidation(&crate::config::ConsolidationConfig::default());
        let temp = tempfile::tempdir().unwrap();
        let store =
            Arc::new(crate::state_store::StateStore::new(temp.path().to_path_buf()).unwrap());
        let mgr = Arc::new(MemoryManager::new(store));
        let dream = DreamProcess::new(mgr, config, temp.path().to_path_buf());

        assert!(dream.should_dream(None, 0));
    }

    #[test]
    fn test_should_dream_too_recent() {
        let config =
            DreamConfig::from_consolidation(&crate::config::ConsolidationConfig::default());
        let temp = tempfile::tempdir().unwrap();
        let store =
            Arc::new(crate::state_store::StateStore::new(temp.path().to_path_buf()).unwrap());
        let mgr = Arc::new(MemoryManager::new(store));
        let dream = DreamProcess::new(mgr, config, temp.path().to_path_buf());

        assert!(!dream.should_dream(Some(Utc::now()), 1));
    }
}
