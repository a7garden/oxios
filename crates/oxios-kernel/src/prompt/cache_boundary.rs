//! Cache boundary management for prompt caching.
//!
//! Splits the system prompt into a **stable prefix** and a **volatile suffix**
//! separated by an invisible boundary marker. Provider-specific caching (Anthropic
//! `cache_control`, OpenAI `prompt_cache_key`) uses this boundary to determine
//! what can be cached across turns.
//!
//! # Design
//!
//! ```text
//! [Stable Prefix]              ← Cached across turns
//!   - Tool summaries
//!   - Safety advisory
//!   - Context files (AGENTS.md, SOUL.md)
//!   - Skills catalog
//!   - Workspace info
//! ─── Cache Boundary ───
//! [Volatile Suffix]            ← Changes every turn
//!   - Runtime timestamp
//!   - Channel-specific guidance
//!   - Provider contributions (dynamic)
//!   - Model identity line
//! ```

/// Invisible boundary marker separating stable and volatile sections.
///
/// This marker is a comment that models ignore but the caching layer uses
/// to determine where to place cache breakpoints.
pub const CACHE_BOUNDARY: &str = "\n<!-- OXIOS_CACHE_BOUNDARY -->\n";

/// A prompt split at the cache boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSplit {
    /// Content above the cache boundary (stable, cacheable).
    pub stable_prefix: String,
    /// Content below the cache boundary (volatile, changes per turn).
    pub volatile_suffix: String,
}

impl PromptSplit {
    /// Split a complete prompt at the cache boundary marker.
    ///
    /// If no boundary marker is found, the entire prompt is treated as stable.
    pub fn split(prompt: &str) -> Self {
        if let Some(idx) = prompt.find(CACHE_BOUNDARY) {
            let (stable, rest) = prompt.split_at(idx);
            let volatile = &rest[CACHE_BOUNDARY.len()..];
            Self {
                stable_prefix: stable.to_string(),
                volatile_suffix: volatile.to_string(),
            }
        } else {
            Self {
                stable_prefix: prompt.to_string(),
                volatile_suffix: String::new(),
            }
        }
    }

    /// Reassemble the prompt from stable and volatile parts.
    pub fn join(&self) -> String {
        if self.volatile_suffix.is_empty() {
            self.stable_prefix.clone()
        } else {
            format!(
                "{}{}{}",
                self.stable_prefix, CACHE_BOUNDARY, self.volatile_suffix
            )
        }
    }

    /// Compute a SHA-256 fingerprint of the stable prefix for cache keying.
    pub fn stable_fingerprint(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.stable_prefix.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

/// Normalize a prompt section for deterministic byte output.
///
/// Ensures cache stability across turns by normalizing:
/// - Line endings to `\n`
/// - Trailing whitespace per line
/// - Leading/trailing whitespace of the whole section
pub fn normalize_section(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_with_boundary() {
        let prompt = format!("stable content{}volatile content", CACHE_BOUNDARY);
        let split = PromptSplit::split(&prompt);
        assert_eq!(split.stable_prefix, "stable content");
        assert_eq!(split.volatile_suffix, "volatile content");
    }

    #[test]
    fn test_split_without_boundary() {
        let prompt = "all stable content";
        let split = PromptSplit::split(prompt);
        assert_eq!(split.stable_prefix, "all stable content");
        assert!(split.volatile_suffix.is_empty());
    }

    #[test]
    fn test_join_roundtrip() {
        let prompt = format!("stable{}volatile", CACHE_BOUNDARY);
        let split = PromptSplit::split(&prompt);
        assert_eq!(split.join(), prompt);
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let split = PromptSplit {
            stable_prefix: "hello world".to_string(),
            volatile_suffix: String::new(),
        };
        assert_eq!(split.stable_fingerprint(), split.stable_fingerprint());
    }

    #[test]
    fn test_fingerprint_changes_on_content_change() {
        let a = PromptSplit {
            stable_prefix: "hello".to_string(),
            volatile_suffix: String::new(),
        };
        let b = PromptSplit {
            stable_prefix: "world".to_string(),
            volatile_suffix: String::new(),
        };
        assert_ne!(a.stable_fingerprint(), b.stable_fingerprint());
    }

    #[test]
    fn test_normalize_trailing_whitespace() {
        assert_eq!(normalize_section("hello  \nworld  \n"), "hello\nworld");
    }

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(normalize_section("hello\r\nworld"), "hello\nworld");
    }
}
