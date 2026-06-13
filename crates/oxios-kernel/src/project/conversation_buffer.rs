//! ConversationBuffer: in-memory circular buffer of recent conversation turns.
//!
//! Maintains recent N turns in memory (not persisted — restarts with empty
//! buffer). Used for topic shift detection and project context tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// A single conversation turn (user message + agent response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// User message.
    pub user: String,
    /// Agent response (truncated to first 200 chars for efficiency).
    pub agent: String,
    /// Active project at the time (nil if no project).
    pub project_id: Option<Uuid>,
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
        }
    }

    /// Record a user message (before processing).
    pub fn push_user(&mut self, message: &str) {
        let turn = ConversationTurn {
            user: message.to_string(),
            agent: String::new(),
            project_id: None,
            timestamp: Utc::now(),
        };

        // If last turn has empty agent, it's the pending turn — replace
        if let Some(last) = self.turns.back_mut()
            && last.agent.is_empty()
            && last.project_id.is_none()
        {
            last.user = message.to_string();
            last.timestamp = Utc::now();
            return;
        }

        self.turns.push_back(turn);

        // Evict oldest if over capacity
        while self.turns.len() > self.max_turns {
            self.turns.pop_front();
        }
    }

    /// Record an agent response and project context.
    pub fn push_agent(&mut self, response: &str, project_id: Option<Uuid>) {
        if let Some(last) = self.turns.back_mut() {
            last.agent = truncate_response(response, 200);
            last.project_id = project_id;
        }
    }

    /// Get the most recent N turns.
    pub fn recent(&self, n: usize) -> Vec<&ConversationTurn> {
        self.turns.iter().rev().take(n).collect()
    }

    /// Get all turns.
    pub fn turns(&self) -> VecDeque<ConversationTurn> {
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

    /// Detect if the conversation pattern has changed.
    pub fn pattern_changed(&self) -> bool {
        if self.turns.len() < 4 {
            return false;
        }

        let all_turns: Vec<_> = self.turns.iter().collect();
        let recent = &all_turns[all_turns.len() - 2..];
        let previous = &all_turns[all_turns.len() - 4..all_turns.len() - 2];

        let avg_recent =
            recent.iter().map(|t| word_count(&t.user)).sum::<usize>() as f64 / recent.len() as f64;
        let avg_prev = previous.iter().map(|t| word_count(&t.user)).sum::<usize>() as f64
            / previous.len() as f64;

        let ratio = avg_recent / avg_prev.max(1.0);
        !(0.5..=2.0).contains(&ratio)
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
        let end = response
            .char_indices()
            .take_while(|(idx, _)| *idx < max_len)
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

        buf.push_agent("I'm doing well!", None);
        assert_eq!(buf.turns[0].agent, "I'm doing well!");
    }

    #[test]
    fn test_max_capacity() {
        let mut buf = ConversationBuffer::new(3);

        for i in 1..=5 {
            buf.push_user(&format!("msg{}", i));
            buf.push_agent("r", None);
        }

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.recent(1)[0].user, "msg5");
    }

    #[test]
    fn test_pattern_changed() {
        let mut buf = ConversationBuffer::new(10);

        for _ in 0..3 {
            buf.push_user("hi");
            buf.push_agent("hi", None);
        }
        assert!(!buf.pattern_changed());

        buf.push_user("This is a very long message with many many many words to trigger detection");
        buf.push_agent("ok", None);
        assert!(buf.pattern_changed());
    }
}
