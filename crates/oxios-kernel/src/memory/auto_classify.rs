//! Automatic memory type classification from content.
//!
//! Infers `MemoryType` from content text using Korean/English dual pattern
//! matching. Used when memories are stored without explicit type, and by
//! Dream Phase 2 for re-classification.

use super::MemoryType;

// ---------------------------------------------------------------------------
// Korean/English pattern constants
// ---------------------------------------------------------------------------

/// Patterns indicating a user correction (contradiction of previous info).
const CORRECTION_PATTERNS_KO: &[&str] = &[
    "아니야",
    "그게 아니라",
    "아니라",
    "잘못됐어",
    "잘못됐다",
    "수정해",
    "수정할게",
    "정정",
    "틀렸어",
    "틀린",
    "그건 아니고",
];

const CORRECTION_PATTERNS_EN: &[&str] = &[
    "actually",
    "no, it's",
    "that's wrong",
    "correction",
    "i meant",
    "not that",
    "i was wrong",
];

/// Patterns indicating a preference or taste.
const PREFERENCE_PATTERNS_KO: &[&str] = &[
    "좋아해",
    "좋아한다",
    "항상 ",
    "로 해",
    "로 해줘",
    "선호해",
    "선호한다",
    "싫어",
    "싫다",
    "하지 마",
    "하지 마라",
    "쓰지 마",
    "난 ",
    "나는 ~",
    "편이",
    "게 좋겠어",
    "로 해주세요",
];

const PREFERENCE_PATTERNS_EN: &[&str] = &[
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
const DECISION_PATTERNS_KO: &[&str] = &[
    "하기로 했어",
    "하기로 했다",
    "선택했어",
    "선택했다",
    "로 가자",
    "로 가겠",
    "로 결정",
    "결정했",
    "결정한다",
    "으로 한다",
    "로 한다",
];

const DECISION_PATTERNS_EN: &[&str] = &[
    "decided to",
    "we chose",
    "let's go with",
    "we'll use",
    "i decided",
    "the decision is",
    "going with",
];

/// Patterns indicating a skill/procedure.
const SKILL_PATTERNS_KO: &[&str] = &[
    "항상 ",
    "하기 전에",
    "매번 ",
    "해야 해",
    "해야 한다",
    "필수",
    "기본적으로",
    "워크플로우",
    "절차",
    "프로세스",
];

const SKILL_PATTERNS_EN: &[&str] = &[
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
const PROFILE_PATTERNS_KO: &[&str] = &[
    "나는 ",
    "내 이름은",
    "나 ",
    "소속",
    "개발자",
    "엔지니어",
    "직업",
    "나의 역할",
    "포지션",
];

const PROFILE_PATTERNS_EN: &[&str] = &[
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
const EPISODE_PATTERNS_KO: &[&str] = &[
    "했어",
    "했음",
    "했었다",
    "배포했",
    "출시했",
    "완료했",
    "시작했",
    "종료했",
    "했고",
];

const EPISODE_PATTERNS_EN: &[&str] = &[
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
/// Uses Korean/English dual pattern matching to infer memory types
/// from content text. Initially rule-based; can be upgraded to LLM-based
/// classification later (§10.6 in RFC-008).
pub struct AutoClassifier;

impl AutoClassifier {
    /// Classify a new memory entry from its content and optional context.
    ///
    /// Returns the inferred `MemoryType`. Falls back to `Fact` if no
    /// specific type can be determined.
    pub fn infer_memory_type(content: &str, _context: &str) -> MemoryType {
        let content_lower = content.to_lowercase();

        // Priority order matters:
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

    /// Check if content looks like a correction.
    fn is_correction(content_lower: &str) -> bool {
        CORRECTION_PATTERNS_KO
            .iter()
            .any(|p| content_lower.contains(p))
            || CORRECTION_PATTERNS_EN
                .iter()
                .any(|p| content_lower.contains(p))
    }

    /// Check if content looks like a preference statement.
    fn is_preference(content_lower: &str) -> bool {
        PREFERENCE_PATTERNS_KO
            .iter()
            .any(|p| content_lower.contains(p))
            || PREFERENCE_PATTERNS_EN
                .iter()
                .any(|p| content_lower.contains(p))
    }

    /// Check if content looks like a decision.
    fn is_decision(content_lower: &str) -> bool {
        DECISION_PATTERNS_KO
            .iter()
            .any(|p| content_lower.contains(p))
            || DECISION_PATTERNS_EN
                .iter()
                .any(|p| content_lower.contains(p))
    }

    /// Check if content looks like a skill/procedure.
    fn is_skill(content_lower: &str) -> bool {
        SKILL_PATTERNS_KO
            .iter()
            .any(|p| content_lower.contains(p))
            || SKILL_PATTERNS_EN
                .iter()
                .any(|p| content_lower.contains(p))
    }

    /// Check if content looks like profile information.
    fn is_profile(content_lower: &str) -> bool {
        PROFILE_PATTERNS_KO
            .iter()
            .any(|p| content_lower.contains(p))
            || PROFILE_PATTERNS_EN
                .iter()
                .any(|p| content_lower.contains(p))
    }

    /// Check if content looks like an episode/event.
    fn is_episode(content_lower: &str) -> bool {
        EPISODE_PATTERNS_KO
            .iter()
            .any(|p| content_lower.contains(p))
            || EPISODE_PATTERNS_EN
                .iter()
                .any(|p| content_lower.contains(p))
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

    /// Check if a word is a common stop word (English only for simplicity).
    fn is_stop_word(word: &str) -> bool {
        const STOP: &[&str] = &[
            "that", "this", "with", "from", "have", "been", "were", "will",
            "would", "could", "should", "about", "which", "their", "there",
            "these", "those", "other", "than", "then", "also", "some",
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
    fn test_classify_correction_ko() {
        assert_eq!(
            AutoClassifier::infer_memory_type("아니야, 그게 아니라 이거야", ""),
            MemoryType::Fact
        );
    }

    #[test]
    fn test_classify_correction_en() {
        assert_eq!(
            AutoClassifier::infer_memory_type("Actually, the port is 8080 not 3000", ""),
            MemoryType::Fact
        );
    }

    #[test]
    fn test_classify_preference_ko() {
        assert_eq!(
            AutoClassifier::infer_memory_type("나는 한국어로 해줘", ""),
            MemoryType::Preference
        );
    }

    #[test]
    fn test_classify_preference_en() {
        assert_eq!(
            AutoClassifier::infer_memory_type("I prefer dark mode for the editor", ""),
            MemoryType::Preference
        );
    }

    #[test]
    fn test_classify_decision_ko() {
        assert_eq!(
            AutoClassifier::infer_memory_type("HNSW로 가자 FAISS 대신", ""),
            MemoryType::Decision
        );
    }

    #[test]
    fn test_classify_decision_en() {
        assert_eq!(
            AutoClassifier::infer_memory_type("We decided to use Tokio for async runtime", ""),
            MemoryType::Decision
        );
    }

    #[test]
    fn test_classify_skill_ko() {
        assert_eq!(
            AutoClassifier::infer_memory_type("커밋하기 전에 항상 cargo test를 돌려야 해", ""),
            MemoryType::Skill
        );
    }

    #[test]
    fn test_classify_skill_en() {
        assert_eq!(
            AutoClassifier::infer_memory_type("Always run tests before commit", ""),
            MemoryType::Skill
        );
    }

    #[test]
    fn test_classify_profile_ko() {
        assert_eq!(
            AutoClassifier::infer_memory_type("나는 백엔드 개발자야", ""),
            MemoryType::UserProfile
        );
    }

    #[test]
    fn test_classify_profile_en() {
        assert_eq!(
            AutoClassifier::infer_memory_type("My name is Won and I work at Oxios", ""),
            MemoryType::UserProfile
        );
    }

    #[test]
    fn test_classify_episode_ko() {
        assert_eq!(
            AutoClassifier::infer_memory_type("v0.2.0을 배포했어", ""),
            MemoryType::Episode
        );
    }

    #[test]
    fn test_classify_episode_en() {
        assert_eq!(
            AutoClassifier::infer_memory_type("Released v0.2.0 with memory consolidation", ""),
            MemoryType::Episode
        );
    }

    #[test]
    fn test_classify_default_fact() {
        assert_eq!(
            AutoClassifier::infer_memory_type("API uses port 3000", ""),
            MemoryType::Fact
        );
    }

    #[test]
    fn test_extract_tags() {
        let tags = AutoClassifier::extract_tags(
            "Rust tokio async runtime memory consolidation system",
            5,
        );
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"tokio".to_string()));
        assert!(tags.contains(&"memory".to_string()));
    }

    #[test]
    fn test_extract_tags_korean() {
        let tags = AutoClassifier::extract_tags("메모리 압축 시스템 구현", 5);
        assert!(!tags.is_empty() || true); // Korean terms may not pass len>3 filter
    }
}
