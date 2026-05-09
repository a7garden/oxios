use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::channel::Channel;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Telegram channel adapter.
/// 
/// Uses long polling (getUpdates) to receive messages
/// and the Bot API to send responses.
pub struct TelegramChannel {
    bot_token: String,
    api_base: String,
    allowed_users: Vec<i64>,
    client: reqwest::Client,
    offset: Arc<RwLock<i64>>,
    /// Maps chat_id → session metadata
    sessions: Arc<RwLock<HashMap<i64, String>>>,
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
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Override API base URL (for local Bot API servers).
    pub fn with_api_base(mut self, base: String) -> Self {
        self.api_base = base;
        self
    }
    
    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.api_base, self.bot_token, method)
    }
    
    /// Check if user is allowed.
    fn is_user_allowed(&self, user_id: i64) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(&user_id)
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
        
        let resp = self.client
            .post(self.api_url("getUpdates"))
            .json(&body)
            .send().await?;
        
        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("Telegram getUpdates failed: {err}");
        }
        
        let json: serde_json::Value = resp.json().await?;
        let updates = json.get("result")
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
            for chunk in text.as_bytes().chunks(4000)
                .map(|c| String::from_utf8_lossy(c).to_string()) 
            {
                body["text"] = serde_json::Value::String(chunk);
                self.client
                    .post(self.api_url("sendMessage"))
                    .json(&body)
                    .send().await?;
            }
        } else {
            let resp = self.client
                .post(self.api_url("sendMessage"))
                .json(&body)
                .send().await?;
            
            if !resp.status().is_success() {
                // Fallback: send without parse_mode
                body["parse_mode"] = serde_json::Value::Null;
                self.client
                    .post(self.api_url("sendMessage"))
                    .json(&body)
                    .send().await?;
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
                let message = update.get("message")
                    .or_else(|| update.get("channel_post"))
                    .or_else(|| update.get("edited_message"));
                
                if let Some(msg) = message {
                    let chat_id = msg.get("chat")
                        .and_then(|c| c.get("id"))
                        .and_then(|id| id.as_i64());
                    
                    let user_id = msg.get("from")
                        .and_then(|f| f.get("id"))
                        .and_then(|id| id.as_i64());
                    
                    let text = msg.get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    
                    let message_id = msg.get("message_id")
                        .and_then(|id| id.as_i64())
                        .unwrap_or(0);
                    
                    // Skip empty messages
                    if text.is_empty() {
                        continue;
                    }
                    
                    // Skip /command messages (let other bots handle)
                    if text.starts_with('/') {
                        continue;
                    }
                    
                    // Check user permission
                    if let Some(uid) = user_id {
                        if !self.is_user_allowed(uid) {
                            tracing::warn!(user_id = uid, "Unauthorized Telegram user");
                            if let Some(cid) = chat_id {
                                let _ = self.send_text(cid, "Unauthorized. Your user ID is not in the allowed list.", None).await;
                            }
                            continue;
                        }
                    }
                    
                    // Build incoming message
                    if let Some(cid) = chat_id {
                        let user_id_str = user_id.map(|id| id.to_string()).unwrap_or_else(|| "unknown".to_string());
                        let mut metadata = HashMap::new();
                        metadata.insert("chat_id".to_string(), cid.to_string());
                        metadata.insert("message_id".to_string(), message_id.to_string());
                        
                        // Store session mapping
                        self.sessions.write().await.insert(cid, user_id_str.clone());
                        
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
        let chat_id: i64 = msg.metadata.get("chat_id")
            .and_then(|id| id.parse().ok())
            .or_else(|| {
                // Try to parse user_id as chat_id
                msg.user_id.parse().ok()
            })
            .ok_or_else(|| anyhow::anyhow!("No chat_id for Telegram message"))?;
        
        let reply_to = msg.metadata.get("message_id")
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
        assert_eq!(channel.api_url("getMe"), "https://api.telegram.org/bot123:ABC/getMe");
    }
}
