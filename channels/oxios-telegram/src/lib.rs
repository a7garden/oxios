pub mod plugin;

pub use plugin::TelegramPlugin;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use oxios_gateway::channel::Channel;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-chat session state for Telegram.
#[derive(Debug, Clone)]
struct ChatSession {
    /// Current session ID used for multi-turn conversations.
    session_id: String,
    /// When the session was created.
    created_at: DateTime<Utc>,
    /// When the last message was sent/received in this session.
    last_active_at: DateTime<Utc>,
    /// Number of messages exchanged in this session.
    message_count: usize,
}

impl ChatSession {
    fn new() -> Self {
        let now = Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            created_at: now,
            last_active_at: now,
            message_count: 0,
        }
    }

    /// Check if this session should be rotated based on configuration.
    fn should_rotate(&self, rotation_hours: u64, max_messages: usize) -> bool {
        // Time-based rotation
        if rotation_hours > 0 {
            let elapsed = Utc::now() - self.last_active_at;
            if elapsed > chrono::Duration::hours(rotation_hours as i64) {
                return true;
            }
        }
        // Message-count based rotation
        if max_messages > 0 && self.message_count >= max_messages {
            return true;
        }
        false
    }

    /// Touch the session (update last_active_at and increment message count).
    fn touch(&mut self) {
        self.last_active_at = Utc::now();
        self.message_count += 1;
    }

    /// Rotate to a new session, returning the old session ID.
    fn rotate(&mut self) -> String {
        let old_id = self.session_id.clone();
        let now = Utc::now();
        self.session_id = uuid::Uuid::new_v4().to_string();
        self.created_at = now;
        self.last_active_at = now;
        self.message_count = 0;
        old_id
    }
}

/// Telegram session configuration.
#[derive(Debug, Clone)]
pub struct TelegramSessionSettings {
    /// Automatically rotate sessions after this many hours of inactivity.
    pub rotation_hours: u64,
    /// Rotate after this many messages (0 = unlimited).
    pub max_messages_per_session: usize,
}

impl Default for TelegramSessionSettings {
    fn default() -> Self {
        Self {
            rotation_hours: 2,
            max_messages_per_session: 0,
        }
    }
}

/// Telegram channel adapter.
///
/// Uses long polling (getUpdates) to receive messages
/// and the Bot API to send responses.
///
/// Session management:
/// - Each `chat_id` gets its own session tracked in memory.
/// - Sessions auto-rotate after a configurable period of inactivity.
/// - Users can force a new session with the `/new` command.
pub struct TelegramChannel {
    bot_token: String,
    api_base: String,
    allowed_users: Vec<i64>,
    client: reqwest::Client,
    offset: Arc<RwLock<i64>>,
    /// Maps chat_id → per-chat session state
    chat_sessions: Arc<RwLock<HashMap<i64, ChatSession>>>,
    /// Session rotation settings
    session_settings: TelegramSessionSettings,
}

impl TelegramChannel {
    /// Create a new Telegram channel.
    ///
    /// # Arguments
    /// * `bot_token` - Telegram Bot API token from @BotFather
    /// * `allowed_users` - List of allowed Telegram user IDs (empty = allow all)
    pub fn new(bot_token: String, allowed_users: Vec<i64>) -> Self {
        Self {
            bot_token,
            api_base: "https://api.telegram.org".to_string(),
            allowed_users,
            client: reqwest::Client::new(),
            offset: Arc::new(RwLock::new(0)),
            chat_sessions: Arc::new(RwLock::new(HashMap::new())),
            session_settings: TelegramSessionSettings::default(),
        }
    }

    /// Override API base URL (for local Bot API servers).
    pub fn with_api_base(mut self, base: String) -> Self {
        self.api_base = base;
        self
    }

    /// Set session management settings.
    pub fn with_session_settings(mut self, settings: TelegramSessionSettings) -> Self {
        self.session_settings = settings;
        self
    }

    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.api_base, self.bot_token, method)
    }

    /// Check if user is allowed.
    fn is_user_allowed(&self, user_id: i64) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(&user_id)
    }

    /// Get or create a session for a chat, auto-rotating if needed.
    async fn get_or_create_session(&self, chat_id: i64) -> String {
        let mut sessions = self.chat_sessions.write().await;
        let session = sessions.entry(chat_id).or_insert_with(ChatSession::new);

        // Check if rotation is needed
        if session.should_rotate(
            self.session_settings.rotation_hours,
            self.session_settings.max_messages_per_session,
        ) {
            session.rotate();
            tracing::info!(
                chat_id = chat_id,
                new_session = %session.session_id,
                "Telegram session auto-rotated"
            );
        }

        session.touch();
        session.session_id.clone()
    }

    /// Force-rotate a chat's session (used for /new command).
    async fn force_new_session(&self, chat_id: i64) -> String {
        let mut sessions = self.chat_sessions.write().await;
        let session = sessions.entry(chat_id).or_insert_with(ChatSession::new);
        let old_id = session.rotate();
        tracing::info!(
            chat_id = chat_id,
            old_session = %old_id,
            new_session = %session.session_id,
            "Telegram session force-rotated via /new command"
        );
        session.session_id.clone()
    }

    /// Poll for updates using getUpdates (long polling).
    async fn poll_updates(&self) -> Result<Vec<serde_json::Value>> {
        let offset = *self.offset.read().await;
        let mut body = serde_json::json!({
            "timeout": 30,
            "limit": 100,
        });
        if offset > 0 {
            body["offset"] = serde_json::Value::Number(offset.into());
        }

        let resp = self
            .client
            .post(self.api_url("getUpdates"))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("Telegram getUpdates failed: {err}");
        }

        let json: serde_json::Value = resp.json().await?;
        let updates = json
            .get("result")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        // Update offset
        if let Some(last) = updates.last() {
            if let Some(id) = last.get("update_id").and_then(|id| id.as_i64()) {
                *self.offset.write().await = id + 1;
            }
        }

        Ok(updates)
    }

    /// Send a text message to a chat.
    async fn send_text(&self, chat_id: i64, text: &str, reply_to: Option<i64>) -> Result<()> {
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
        });
        if let Some(msg_id) = reply_to {
            body["reply_to_message_id"] = serde_json::Value::Number(msg_id.into());
        }

        // Telegram message limit is 4096 chars
        // If message is too long, split it
        if text.len() > 4000 {
            for chunk in text
                .as_bytes()
                .chunks(4000)
                .map(|c| String::from_utf8_lossy(c).to_string())
            {
                body["text"] = serde_json::Value::String(chunk);
                self.client
                    .post(self.api_url("sendMessage"))
                    .json(&body)
                    .send()
                    .await?;
            }
        } else {
            let resp = self
                .client
                .post(self.api_url("sendMessage"))
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                // Fallback: send without parse_mode
                body["parse_mode"] = serde_json::Value::Null;
                self.client
                    .post(self.api_url("sendMessage"))
                    .json(&body)
                    .send()
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn receive(&self) -> Result<Option<IncomingMessage>> {
        loop {
            let updates = self.poll_updates().await?;

            for update in updates {
                // Extract message from update
                let message = update
                    .get("message")
                    .or_else(|| update.get("channel_post"))
                    .or_else(|| update.get("edited_message"));

                if let Some(msg) = message {
                    let chat_id = msg
                        .get("chat")
                        .and_then(|c| c.get("id"))
                        .and_then(|id| id.as_i64());

                    let user_id = msg
                        .get("from")
                        .and_then(|f| f.get("id"))
                        .and_then(|id| id.as_i64());

                    let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");

                    let message_id = msg
                        .get("message_id")
                        .and_then(|id| id.as_i64())
                        .unwrap_or(0);

                    // Skip empty messages
                    if text.is_empty() {
                        continue;
                    }

                    // Check user permission
                    if let Some(uid) = user_id {
                        if !self.is_user_allowed(uid) {
                            tracing::warn!(user_id = uid, "Unauthorized Telegram user");
                            if let Some(cid) = chat_id {
                                let _ = self
                                    .send_text(
                                        cid,
                                        "Unauthorized. Your user ID is not in the allowed list.",
                                        None,
                                    )
                                    .await;
                            }
                            continue;
                        }
                    }

                    if let Some(cid) = chat_id {
                        let user_id_str = user_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        // Handle /new command — start a new session
                        if text.trim() == "/new" || text.trim() == "/new@me" {
                            let new_session_id = self.force_new_session(cid).await;
                            let _ = self
                                .send_text(
                                    cid,
                                    &format!("🔄 새 세션을 시작합니다.\\n`{}`", &new_session_id[..8]),
                                    Some(message_id),
                                )
                                .await;
                            continue;
                        }

                        // Handle /session command — show current session info
                        if text.trim() == "/session" || text.trim() == "/session@me" {
                            let sessions = self.chat_sessions.read().await;
                            if let Some(session) = sessions.get(&cid) {
                                let info = format!(
                                    "📋 현재 세션\\n• ID: `{}`\\n• 메시지: {}개\\n• 시작: {}\\n• 마지막 활동: {}",
                                    &session.session_id[..8],
                                    session.message_count,
                                    session.created_at.format("%m/%d %H:%M"),
                                    session.last_active_at.format("%m/%d %H:%M"),
                                );
                                let _ = self.send_text(cid, &info, Some(message_id)).await;
                            } else {
                                let _ = self
                                    .send_text(cid, "📋 활성 세션이 없습니다.", Some(message_id))
                                    .await;
                            }
                            continue;
                        }

                        // Skip other /command messages (let other bots handle)
                        if text.starts_with('/') {
                            continue;
                        }

                        // Get or auto-rotate session
                        let session_id = self.get_or_create_session(cid).await;

                        let mut metadata = HashMap::new();
                        metadata.insert("chat_id".to_string(), cid.to_string());
                        metadata.insert("message_id".to_string(), message_id.to_string());
                        metadata.insert("session_id".to_string(), session_id);

                        let incoming = IncomingMessage {
                            channel: "telegram".to_string(),
                            user_id: user_id_str,
                            content: text.to_string(),
                            metadata,
                            ..Default::default()
                        };

                        tracing::info!(chat_id = cid, text = %text.chars().take(50).collect::<String>(), "Telegram message received");
                        return Ok(Some(incoming));
                    }
                }
            }

            // No valid messages in this poll, continue polling
            continue;
        }
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // Look up chat_id from metadata or sessions
        let chat_id: i64 = msg
            .metadata
            .get("chat_id")
            .and_then(|id| id.parse().ok())
            .or_else(|| {
                // Try to parse user_id as chat_id
                msg.user_id.parse().ok()
            })
            .ok_or_else(|| anyhow::anyhow!("No chat_id for Telegram message"))?;

        let reply_to = msg
            .metadata
            .get("message_id")
            .and_then(|id| id.parse().ok());

        self.send_text(chat_id, &msg.content, reply_to).await?;
        tracing::debug!(chat_id = chat_id, "Telegram response sent");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_channel_new() {
        let channel = TelegramChannel::new("test-token".to_string(), vec![12345]);
        assert_eq!(channel.name(), "telegram");
        assert!(channel.is_user_allowed(12345));
        assert!(!channel.is_user_allowed(99999));
    }

    #[test]
    fn test_telegram_channel_allow_all() {
        let channel = TelegramChannel::new("test-token".to_string(), vec![]);
        assert!(channel.is_user_allowed(12345));
        assert!(channel.is_user_allowed(99999));
    }

    #[test]
    fn test_api_url() {
        let channel = TelegramChannel::new("123:ABC".to_string(), vec![]);
        assert_eq!(
            channel.api_url("getMe"),
            "https://api.telegram.org/bot123:ABC/getMe"
        );
    }

    #[test]
    fn test_chat_session_rotation_by_time() {
        let mut session = ChatSession::new();
        assert!(!session.should_rotate(2, 0)); // Just created, should not rotate

        // Simulate 3 hours of inactivity
        session.last_active_at = Utc::now() - chrono::Duration::hours(3);
        assert!(session.should_rotate(2, 0)); // Should rotate
        assert!(!session.should_rotate(0, 0)); // Disabled, should not rotate
    }

    #[test]
    fn test_chat_session_rotation_by_message_count() {
        let mut session = ChatSession::new();
        session.message_count = 50;
        assert!(session.should_rotate(0, 50)); // At limit
        assert!(session.should_rotate(0, 49)); // Over limit
        assert!(!session.should_rotate(0, 51)); // Under limit
        assert!(!session.should_rotate(0, 0)); // Disabled
    }

    #[test]
    fn test_chat_session_rotate_resets_state() {
        let mut session = ChatSession::new();
        let original_id = session.session_id.clone();
        session.message_count = 100;

        let old_id = session.rotate();
        assert_eq!(old_id, original_id);
        assert_ne!(session.session_id, original_id);
        assert_eq!(session.message_count, 0);
    }

    #[test]
    fn test_chat_session_touch() {
        let mut session = ChatSession::new();
        assert_eq!(session.message_count, 0);
        session.touch();
        assert_eq!(session.message_count, 1);
        session.touch();
        assert_eq!(session.message_count, 2);
    }
}
