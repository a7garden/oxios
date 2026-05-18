//! ConversationBuffer: in-memory circular buffer of recent conversation turns.
//!
//! Used by SpaceManager for topic shift detection. Maintains recent N turns
//! in memory (not persisted — restarts with empty buffer, which is fine since
//! Layer 1/2 detection doesn't need history).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use super::SpaceId;

/// A single conversation turn (user message + agent response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// User message.
    pub user: String,
    /// Agent response (truncated to first 200 chars for efficiency).
    pub agent: String,
    /// Active Space at the time.
    pub space_id: SpaceId,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

/// In-memory circular buffer of recent conversation turns.
#[derive(Debug, Clone)]
pub struct ConversationBuffer {
    /// Recent turns (bounded, oldest evicted first).
    turns: VecDeque<ConversationTurn>,
    /// Maximum number of turns to retain.
    max_turns: usize,
    /// Counter for topic check frequency limiting.
    turns_since_topic_check: usize,
    /// Last observed Space ID (for tracking Space switches).
    last_space_id: Option<SpaceId>,
}

impl Default for ConversationBuffer {
    fn default() -> Self {
        Self::new(50)
    }
}

impl ConversationBuffer {
    /// Create a new buffer with the given maximum size.
    pub fn new(max_turns: usize) -> Self {
        Self {
            turns: VecDeque::with_capacity(max_turns),
            max_turns,
            turns_since_topic_check: 0,
            last_space_id: None,
        }
    }

    /// Record a user message (before processing).
    pub fn push_user(&mut self, message: &str) {
        let turn = ConversationTurn {
            user: message.to_string(),
            agent: String::new(),     // filled later by push_agent
            space_id: SpaceId::nil(), // filled later
            timestamp: Utc::now(),
        };

        // If last turn has empty agent, it's the pending turn — replace
        if let Some(last) = self.turns.back_mut() {
            if last.agent.is_empty() && last.space_id == SpaceId::nil() {
                last.user = message.to_string();
                last.timestamp = Utc::now();
                return;
            }
        }

        self.turns.push_back(turn);

        // Evict oldest if over capacity
        while self.turns.len() > self.max_turns {
            self.turns.pop_front();
        }
    }

    /// Record an agent response and Space (call after processing completes).
    pub fn push_agent(&mut self, response: &str, space_id: &SpaceId) {
        if let Some(last) = self.turns.back_mut() {
            last.agent = truncate_response(response, 200);
            last.space_id = *space_id;
            self.last_space_id = Some(*space_id);
        }
    }

    /// Get the most recent N turns.
    pub fn recent(&self, n: usize) -> Vec<&ConversationTurn> {
        self.turns.iter().rev().take(n).collect()
    }

    /// Get all turns.
    pub fn turns(&self) -> std::collections::VecDeque<ConversationTurn> {
        self.turns.clone()
    }

    /// Get the total number of turns.
    pub fn len(&self) -> usize {
        self.turns.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Check if topic shift detection should run.
    ///
    /// Returns true if N or more turns have passed since the last check,
    /// or if the conversation pattern has visibly changed.
    pub fn should_check_topic(&self, min_turns: usize) -> bool {
        self.turns_since_topic_check >= min_turns || self.pattern_changed()
    }

    /// Record that a topic check was performed.
    pub fn mark_topic_checked(&mut self) {
        self.turns_since_topic_check = 0;
    }

    /// Increment the turn counter and check if topic check should run.
    pub fn record_turn(&mut self, min_turns: usize) -> bool {
        self.turns_since_topic_check += 1;
        self.should_check_topic(min_turns)
    }

    /// Static version of should_check_topic that works on a slice of ConversationTurns.
    ///
    /// Returns false if we don't have enough history (safe default — skips
    /// expensive LLM-based detection). Returns true if 3+ turns have passed
    /// since last check or pattern changed.
    pub fn should_check_topic_from_messages(turns: &[ConversationTurn], _min_turns: usize) -> bool {
        // Not enough history → skip expensive LLM detection (safe default)
        if turns.len() < 4 {
            return false;
        }

        // Check message length changes (basic pattern detection)
        let recent = &turns[turns.len() - 2..];
        let previous = &turns[turns.len() - 4..turns.len() - 2];

        let avg_recent =
            recent.iter().map(|t| word_count(&t.user)).sum::<usize>() as f64 / recent.len() as f64;
        let avg_prev =
            previous.iter().map(|t| word_count(&t.user)).sum::<usize>() as f64 / previous.len() as f64;

        let ratio = avg_recent / avg_prev.max(1.0);
        !(0.5..=2.0).contains(&ratio)
    }

    /// Detect if the conversation pattern has changed.
    ///
    /// Looks at average word count and average message length to detect
    /// topic or style shifts without LLM.
    pub fn pattern_changed(&self) -> bool {
        if self.turns.len() < 4 {
            return false;
        }

        let all_turns: Vec<_> = self.turns.iter().collect();

        // Compare recent 2 turns vs previous 2 turns
        let recent = &all_turns[all_turns.len() - 2..];
        let previous = &all_turns[all_turns.len() - 4..all_turns.len() - 2];

        let avg_word_count_recent =
            recent.iter().map(|t| word_count(&t.user)).sum::<usize>() as f64 / recent.len() as f64;

        let avg_word_count_prev = previous.iter().map(|t| word_count(&t.user)).sum::<usize>()
            as f64
            / previous.len() as f64;

        // If average word count changed by more than 50%, consider it a pattern change
        let ratio = avg_word_count_recent / avg_word_count_prev.max(1.0);
        if !(0.5..=2.0).contains(&ratio) {
            return true;
        }

        // Check for domain-specific keywords that suggest topic shift
        let domain_shift_keywords = [
            ("code", "food"),
            ("rust", "요리"),
            ("bug", "저녁"),
            ("file", "운동"),
            ("import", "영화"),
            ("commit", "음식"),
            ("function", "게임"),
            ("Cargo", "장보기"),
        ];

        let recent_text = recent
            .iter()
            .map(|t| t.user.to_lowercase())
            .collect::<String>();
        let prev_text = previous
            .iter()
            .map(|t| t.user.to_lowercase())
            .collect::<String>();

        for (prev_kw, recent_kw) in domain_shift_keywords {
            let has_prev = prev_text.contains(prev_kw);
            let has_recent = recent_text.contains(recent_kw);
            if has_prev && !has_recent {
                // Keyword disappeared — possible topic shift
                // But we need to be in the same Space still
                return true;
            }
        }

        false
    }

    /// Check if the Space has changed since the last turn.
    pub fn space_changed(&self) -> bool {
        if self.turns.len() < 2 {
            return false;
        }

        let all_turns: Vec<_> = self.turns.iter().collect();
        let last = &all_turns[all_turns.len() - 1];
        let prev = &all_turns[all_turns.len() - 2];

        last.space_id != prev.space_id
    }

    /// Get the last recorded Space ID.
    pub fn last_space_id(&self) -> Option<SpaceId> {
        self.last_space_id
    }

    /// Clear all turns.
    pub fn clear(&mut self) {
        self.turns.clear();
        self.turns_since_topic_check = 0;
    }
}

/// Count words in a string.
fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Truncate response to max_len bytes, respecting UTF-8 char boundaries.
fn truncate_response(response: &str, max_len: usize) -> String {
    if response.len() <= max_len {
        response.to_string()
    } else {
        // Find the nearest char boundary at or before max_len
        let end = response.char_indices()
            .take_while(|(idx, _)| *idx <= max_len)
            .last()
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);
        if end == 0 {
            "...".to_string()
        } else {
            format!("{}...", &response[..end])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_user_and_agent() {
        let mut buf = ConversationBuffer::new(10);
        assert!(buf.is_empty());

        buf.push_user("Hello, how are you?");
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.turns[0].user, "Hello, how are you?");
        assert!(buf.turns[0].agent.is_empty());

        buf.push_agent("I'm doing well!", &SpaceId::nil());
        assert_eq!(buf.turns[0].agent, "I'm doing well!");
    }

    #[test]
    fn test_max_capacity() {
        let mut buf = ConversationBuffer::new(3);
        let space = SpaceId::nil();

        buf.push_user("msg1");
        buf.push_agent("r1", &space);
        buf.push_user("msg2");
        buf.push_agent("r2", &space);
        buf.push_user("msg3");
        buf.push_agent("r3", &space);
        buf.push_user("msg4");
        buf.push_agent("r4", &space);
        buf.push_user("msg5");
        buf.push_agent("r5", &space);

        // Oldest should be evicted
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.recent(1)[0].user, "msg5");
    }

    #[test]
    fn test_should_check_topic() {
        let mut buf = ConversationBuffer::new(10);
        assert!(!buf.should_check_topic(3));

        for _ in 0..3 {
            buf.push_user("test");
            buf.mark_topic_checked();
        }
        // After mark_topic_checked, counter resets
        assert!(!buf.should_check_topic(3));
    }

    #[test]
    fn test_pattern_changed_word_count() {
        let mut buf = ConversationBuffer::new(10);
        let space = SpaceId::nil();

        // Short messages
        for _ in 0..3 {
            buf.push_user("hi");
            buf.push_agent("hi", &space);
        }

        assert!(!buf.pattern_changed());

        // Now a very long message
        buf.push_user("This is a very long message that contains many many many many many words to trigger the pattern detection");
        buf.push_agent("ok", &space);

        assert!(buf.pattern_changed());
    }

    #[test]
    fn test_truncate_response() {
        let short = "Hello";
        assert_eq!(truncate_response(short, 10), "Hello");

        let long = "This is a very long response";
        let truncated = truncate_response(long, 10);
        assert_eq!(truncated.len(), 13); // 10 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_recent_turns() {
        let mut buf = ConversationBuffer::new(10);
        let space = SpaceId::nil();

        for i in 0..5 {
            buf.push_user(&format!("msg{}", i));
            buf.push_agent(&format!("resp{}", i), &space);
        }

        let recent = buf.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].user, "msg4"); // most recent first
        assert_eq!(recent[2].user, "msg2");
    }
}
