//! Core memory types — extracted from `oxios-kernel`.
//!
//! These types form the foundation of the memory subsystem and are
//! shared across all memory modules.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Content hashing
// ---------------------------------------------------------------------------

/// Compute a stable hash of content for deduplication.
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// MemoryType
// ---------------------------------------------------------------------------

/// Memory entry type — 9 types derived from the SOAR/ACT-R cognitive model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Conversation compaction summary (auto-generated).
    Conversation,
    /// Session-end summary (auto-generated).
    Session,
    /// A factual statement (e.g., "API uses port 3000").
    Fact,
    /// An event or experience (e.g., "deployed v0.2.0").
    Episode,
    /// Static knowledge (knowledge-base synced, user/program-provided).
    Knowledge,
    /// A learned procedure or pattern (e.g., "run cargo test before commit").
    Skill,
    /// A user preference (e.g., "always use dark mode").
    Preference,
    /// A recorded decision with rationale (e.g., "chose HNSW over FAISS").
    Decision,
    /// User identity and expertise profile.
    UserProfile,
}

impl MemoryType {
    /// Category name used in StateStore.
    pub fn category(&self) -> &'static str {
        match self {
            MemoryType::Conversation => "memory/conversations",
            MemoryType::Session => "memory/sessions",
            MemoryType::Fact => "memory/facts",
            MemoryType::Episode => "memory/episodes",
            MemoryType::Knowledge => "memory/knowledge",
            MemoryType::Skill => "memory/skills",
            MemoryType::Preference => "memory/preferences",
            MemoryType::Decision => "memory/decisions",
            MemoryType::UserProfile => "memory/profiles",
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            MemoryType::Conversation => "conversation",
            MemoryType::Session => "session",
            MemoryType::Fact => "fact",
            MemoryType::Episode => "episode",
            MemoryType::Knowledge => "knowledge",
            MemoryType::Skill => "skill",
            MemoryType::Preference => "preference",
            MemoryType::Decision => "decision",
            MemoryType::UserProfile => "user_profile",
        }
    }

    /// Base importance for each type.
    pub fn base_importance(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.95,
            MemoryType::Preference => 0.90,
            MemoryType::Decision => 0.80,
            MemoryType::Knowledge => 0.75,
            MemoryType::Skill => 0.75,
            MemoryType::Fact => 0.60,
            MemoryType::Episode => 0.50,
            MemoryType::Session => 0.40,
            MemoryType::Conversation => 0.35,
        }
    }

    /// Base decay rate for each type.
    pub fn base_decay_rate(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.001,
            MemoryType::Preference => 0.002,
            MemoryType::Decision => 0.005,
            MemoryType::Knowledge => 0.006,
            MemoryType::Skill => 0.008,
            MemoryType::Fact => 0.015,
            MemoryType::Episode => 0.025,
            MemoryType::Session => 0.040,
            MemoryType::Conversation => 0.060,
        }
    }

    /// Initial tier for new entries of this type.
    pub fn initial_tier(&self) -> MemoryTier {
        match self {
            MemoryType::UserProfile
            | MemoryType::Preference
            | MemoryType::Decision
            | MemoryType::Fact => MemoryTier::Hot,
            MemoryType::Knowledge
            | MemoryType::Skill
            | MemoryType::Episode
            | MemoryType::Session
            | MemoryType::Conversation => MemoryTier::Warm,
        }
    }

    /// Whether this type is automatically protected from deletion.
    pub fn is_auto_protected(&self) -> bool {
        matches!(self, MemoryType::UserProfile | MemoryType::Preference)
    }

    /// Whether this type is stored globally (cross-Space).
    pub fn is_global(&self) -> bool {
        matches!(self, MemoryType::UserProfile | MemoryType::Preference)
    }

    /// All memory type variants.
    pub fn all() -> &'static [MemoryType] {
        &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
            MemoryType::Skill,
            MemoryType::Preference,
            MemoryType::Decision,
            MemoryType::UserProfile,
        ]
    }
}

// ---------------------------------------------------------------------------
// MemoryTier
// ---------------------------------------------------------------------------

/// Memory tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// Always loaded into agent context (~3K tokens).
    Hot,
    /// Loaded on demand (recent sessions, knowledge).
    Warm,
    /// Compressed archive (long-term storage).
    Cold,
}

impl MemoryTier {
    /// Maximum entries per tier (configurable).
    pub fn default_max_entries(&self) -> usize {
        match self {
            MemoryTier::Hot => 50,
            MemoryTier::Warm => 500,
            MemoryTier::Cold => 10_000,
        }
    }

    /// Maximum token budget per tier.
    pub fn default_token_budget(&self) -> usize {
        match self {
            MemoryTier::Hot => 3_000,
            MemoryTier::Warm => 50_000,
            MemoryTier::Cold => usize::MAX,
        }
    }
}

fn default_tier() -> MemoryTier {
    MemoryTier::Warm
}

// ---------------------------------------------------------------------------
// ProtectionLevel
// ---------------------------------------------------------------------------

/// Auto-protection level. Users never need to know about this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ProtectionLevel {
    /// No protection. Normal decay + deletion.
    #[default]
    None = 0,
    /// Slow decay, deletion possible.
    /// Trigger: 2+ accesses.
    Low = 1,
    /// Very slow decay. Deletion only after retention_days × 2.
    /// Trigger: 3+ accesses or 2+ session appearances.
    Medium = 2,
    /// Near-permanent. Preserved in LLM compaction.
    /// Trigger: 5+ accesses, 3+ sessions, or user "remember this".
    High = 3,
    /// Absolute protection. Never deleted or compressed.
    /// Trigger: UserProfile/Preference type, or explicit user pin.
    Permanent = 4,
}

impl ProtectionLevel {
    /// Decay multiplier based on protection level.
    pub fn decay_multiplier(&self) -> f32 {
        match self {
            ProtectionLevel::None => 1.0,
            ProtectionLevel::Low => 0.5,
            ProtectionLevel::Medium => 0.2,
            ProtectionLevel::High => 0.05,
            ProtectionLevel::Permanent => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryEntry
// ---------------------------------------------------------------------------

fn default_importance() -> f32 {
    0.5
}

fn default_now() -> DateTime<Utc> {
    Utc::now()
}

/// A single memory entry with lifecycle and auto-protection metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // ── Identity ──────────────────────────────────────
    /// Unique ID.
    pub id: String,
    /// Memory type (auto-classified if not explicitly set).
    pub memory_type: MemoryType,
    /// Current tier (auto-managed by Dream).
    #[serde(default = "default_tier")]
    pub tier: MemoryTier,

    // ── Content ───────────────────────────────────────
    /// Content (Markdown).
    pub content: String,
    /// Content hash for deduplication.
    #[serde(default)]
    pub content_hash: u64,
    /// Tags (auto-extracted from content).
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Source ────────────────────────────────────────
    /// Creator (agent name, "compaction", "system", "dream", "auto-classify").
    pub source: String,
    /// Related session ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    // ── Importance ────────────────────────────────────
    /// Base importance (0.0–1.0), set by type or auto-computed.
    #[serde(default = "default_importance")]
    pub importance: f32,
    /// Whether user explicitly pinned (optional override).
    #[serde(default)]
    pub pinned: bool,

    // ── Auto-Protection ───────────────────────────────
    /// Auto-computed protection level. Dream recomputes each run.
    #[serde(default)]
    pub protection: ProtectionLevel,
    /// Whether the type was auto-classified (vs explicit).
    #[serde(default)]
    pub auto_classified: bool,
    /// Number of distinct sessions this entry appeared in.
    #[serde(default)]
    pub session_appearances: u32,
    /// Whether the user has corrected/contradicted this entry's topic.
    #[serde(default)]
    pub user_corrected: bool,
    /// Session IDs that have accessed this entry (for dedup of session_appearances).
    /// Max 100 entries; oldest evicted first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen_in_sessions: Vec<String>,

    // ── Lifecycle ─────────────────────────────────────
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last access timestamp.
    pub accessed_at: DateTime<Utc>,
    /// Last modification timestamp.
    #[serde(default = "default_now")]
    pub modified_at: DateTime<Utc>,
    /// Access count.
    #[serde(default)]
    pub access_count: u32,
    /// Current decay score (0.0–1.0), computed by DecayEngine.
    #[serde(default = "default_importance")]
    pub decay_score: f32,
    /// Compaction level (0 = raw, 1 = daily, 2 = weekly, 3 = monthly, 4 = root).
    #[serde(default)]
    pub compaction_level: u8,
    /// IDs of entries this was compacted from.
    #[serde(default)]
    pub compacted_from: Vec<String>,

    // ── Relationships ─────────────────────────────────
    /// IDs of related memory entries.
    #[serde(default)]
    pub related_ids: Vec<String>,
    /// Contradicts a previous entry (ID of the contradicted entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<String>,
}

// ---------------------------------------------------------------------------
// TextVector
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Simple TF-IDF vector for text similarity.
///
/// Tokenizes text into terms, computes normalized term frequency,
/// and supports cosine similarity comparison. No external embedding
/// model needed — language-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextVector {
    /// Term frequencies (normalized).
    tf: HashMap<String, f64>,
}

impl TextVector {
    /// Create a text vector from input text.
    pub fn from_text(text: &str) -> Self {
        let mut tf: HashMap<String, f64> = HashMap::new();
        let terms = Self::tokenize(text);
        let total = terms.len() as f64;

        for term in terms {
            *tf.entry(term).or_insert(0.0) += 1.0;
        }

        // Normalize by total term count
        if total > 0.0 {
            for v in tf.values_mut() {
                *v /= total;
            }
        }

        Self { tf }
    }

    /// Tokenize text into terms (language-agnostic).
    /// Splits on whitespace and punctuation, lowercases.
    /// Preserves non-ASCII alphanumeric runs (CJK, Hangul, etc.) within tokens.
    pub fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && !('\u{AC00}'..='\u{D7A3}').contains(&c))
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Returns a reference to the term-frequency map.
    pub fn tf_map(&self) -> &HashMap<String, f64> {
        &self.tf
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(&self, other: &TextVector) -> f64 {
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for (term, &a) in &self.tf {
            norm_a += a * a;
            if let Some(&b) = other.tf.get(term) {
                dot += a * b;
            }
        }
        for &b in other.tf.values() {
            norm_b += b * b;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract search keywords from a query string.
///
/// Simple implementation: split on whitespace, lowercase, filter stop words.
pub fn extract_keywords(query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "can", "shall",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into", "through",
        "during", "before", "after", "above", "below", "between", "out", "off", "over", "under",
        "again", "further", "then", "once", "and", "but", "or", "nor", "not", "so", "yet", "both",
        "either", "neither", "each", "every", "all", "any", "few", "more", "most", "other", "some",
        "such", "no", "only", "own", "same", "than", "too", "very", "just", "because", "if",
        "when", "where", "how", "what", "which", "who", "whom", "this", "that", "these", "those",
        "i", "me", "my", "we", "our", "you", "your", "he", "him", "his", "she", "her", "it", "its",
        "they", "them", "their",
    ];

    query
        .split_whitespace()
        .map(|w| {
            // Strip trailing punctuation
            let w = w.trim_end_matches(|c: char| c.is_ascii_punctuation());
            w.to_lowercase()
        })
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Remove duplicate entries by ID, keeping the first occurrence.
pub fn dedup_by_id(entries: &mut Vec<MemoryEntry>) {
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.id.clone()));
}
