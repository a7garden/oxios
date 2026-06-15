//! Automatic memory type classification from content.
//!
//! Infers `MemoryType` from content text using pattern matching.
//! Used when memories are stored without explicit type, and by
//! Dream Phase 2 for re-classification.

use crate::memory::types::MemoryType;

// ---------------------------------------------------------------------------
// Pattern constants
// ---------------------------------------------------------------------------

/// Patterns indicating a user correction (contradiction of previous info).
const CORRECTION_PATTERNS: &[&str] = &[
    "actually",
    "no, it's",
    "that's wrong",
    "correction",
    "i meant",
    "not that",
    "i was wrong",
];

/// Patterns indicating a preference or taste.
const PREFERENCE_PATTERNS: &[&str] = &[
    "i prefer",
    "always use",
    "i like",
    "i don't",
    "never use",
    "i'd rather",
    "my preference",
    "please use",
    "make sure to use",
];

/// Patterns indicating a decision.
const DECISION_PATTERNS: &[&str] = &[
    "decided to",
    "we chose",
    "let's go with",
    "we'll use",
    "i decided",
    "the decision is",
    "going with",
];

/// Patterns indicating a skill/procedure.
const SKILL_PATTERNS: &[&str] = &[
    "always run",
    "before commit",
    "every time",
    "make sure to",
    "workflow is",
    "standard procedure",
    "first, then",
    "step by step",
];

/// Patterns indicating profile information.
const PROFILE_PATTERNS: &[&str] = &[
    "my name is",
    "i work at",
    "i'm a ",
    "i am a ",
    "my role is",
    "my job is",
    "i specialize",
    "my background",
];

/// Patterns indicating an episode/event.
const EPISODE_PATTERNS: &[&str] = &[
    "deployed",
    "released",
    "launched",
    "completed",
    "finished",
    "started",
];

// ---------------------------------------------------------------------------
// AutoClassifier
// ---------------------------------------------------------------------------

/// Automatic memory type classifier.
///
/// Uses pattern matching to infer memory types from content text.
/// Falls back to `Fact` when no specific type is detected.
pub struct AutoClassifier;

impl AutoClassifier {
    /// Classify a new memory entry from its content and optional context.
    ///
    /// Returns the inferred `MemoryType`. Falls back to `Fact` if no
    /// specific type can be determined.
    pub fn infer_memory_type(content: &str, _context: &str) -> MemoryType {
        let content_lower = content.to_lowercase();

        // Priority order:
        // 1. Correction → Fact (overrides everything)
        // 2. Preference
        // 3. Decision
        // 4. Skill/Procedure
        // 5. Profile
        // 6. Episode
        // 7. Default → Fact

        if Self::is_correction(&content_lower) {
            return MemoryType::Fact;
        }

        if Self::is_preference(&content_lower) {
            return MemoryType::Preference;
        }

        if Self::is_decision(&content_lower) {
            return MemoryType::Decision;
        }

        if Self::is_skill(&content_lower) {
            return MemoryType::Skill;
        }

        if Self::is_profile(&content_lower) {
            return MemoryType::UserProfile;
        }

        if Self::is_episode(&content_lower) {
            return MemoryType::Episode;
        }

        MemoryType::Fact
    }

    fn is_correction(content_lower: &str) -> bool {
        CORRECTION_PATTERNS
            .iter()
            .any(|p| content_lower.contains(p))
    }

    fn is_preference(content_lower: &str) -> bool {
        PREFERENCE_PATTERNS
            .iter()
            .any(|p| content_lower.contains(p))
    }

    fn is_decision(content_lower: &str) -> bool {
        DECISION_PATTERNS.iter().any(|p| content_lower.contains(p))
    }

    fn is_skill(content_lower: &str) -> bool {
        SKILL_PATTERNS.iter().any(|p| content_lower.contains(p))
    }

    fn is_profile(content_lower: &str) -> bool {
        PROFILE_PATTERNS.iter().any(|p| content_lower.contains(p))
    }

    fn is_episode(content_lower: &str) -> bool {
        EPISODE_PATTERNS.iter().any(|p| content_lower.contains(p))
    }

    /// Extract tags from content for search indexing.
    ///
    /// Simple keyword extraction: split on whitespace, filter short words,
    /// take top N unique terms.
    pub fn extract_tags(content: &str, max_tags: usize) -> Vec<String> {
        let mut tags: Vec<String> = content
            .split_whitespace()
            .map(|w| {
                w.trim_matches(|c: char| c.is_ascii_punctuation())
                    .to_lowercase()
            })
            .filter(|w| w.len() > 3 && !Self::is_stop_word(w))
            .collect();

        tags.sort();
        tags.dedup();
        tags.truncate(max_tags);
        tags
    }

    fn is_stop_word(word: &str) -> bool {
        const STOP: &[&str] = &[
            "that", "this", "with", "from", "have", "been", "were", "will", "would", "could",
            "should", "about", "which", "their", "there", "these", "those", "other", "than",
            "then", "also", "some",
        ];
        STOP.contains(&word)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_correction() {
        assert_eq!(
            AutoClassifier::infer_memory_type("Actually, the port is 8080 not 3000", ""),
            MemoryType::Fact
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("Correction: the API key expired", ""),
            MemoryType::Fact
        );
    }

    #[test]
    fn test_classify_preference() {
        assert_eq!(
            AutoClassifier::infer_memory_type("I prefer dark mode for the editor", ""),
            MemoryType::Preference
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("Never use tabs, always use spaces", ""),
            MemoryType::Preference
        );
    }

    #[test]
    fn test_classify_decision() {
        assert_eq!(
            AutoClassifier::infer_memory_type("We decided to use Tokio for async runtime", ""),
            MemoryType::Decision
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("Let's go with the microservice approach", ""),
            MemoryType::Decision
        );
    }

    #[test]
    fn test_classify_skill() {
        assert_eq!(
            AutoClassifier::infer_memory_type("Always run tests before commit", ""),
            MemoryType::Skill
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("Standard procedure: lint, test, then deploy", ""),
            MemoryType::Skill
        );
    }

    #[test]
    fn test_classify_profile() {
        assert_eq!(
            AutoClassifier::infer_memory_type("My name is Won and I work at Oxios", ""),
            MemoryType::UserProfile
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("I'm a backend engineer", ""),
            MemoryType::UserProfile
        );
    }

    #[test]
    fn test_classify_episode() {
        assert_eq!(
            AutoClassifier::infer_memory_type("Released v0.2.0 with memory consolidation", ""),
            MemoryType::Episode
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("Deployed the new API gateway yesterday", ""),
            MemoryType::Episode
        );
    }

    #[test]
    fn test_classify_default_fact() {
        assert_eq!(
            AutoClassifier::infer_memory_type("API uses port 3000", ""),
            MemoryType::Fact
        );
        assert_eq!(
            AutoClassifier::infer_memory_type("The database has 42 tables", ""),
            MemoryType::Fact
        );
    }

    #[test]
    fn test_extract_tags() {
        let tags =
            AutoClassifier::extract_tags("Rust tokio async runtime memory consolidation system", 5);
        assert!(!tags.is_empty());
        assert!(
            tags.iter()
                .any(|t| t.contains("rust") || t.contains("memory"))
        );
    }
}
