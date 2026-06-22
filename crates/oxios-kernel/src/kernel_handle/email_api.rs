//! Email API — KernelHandle domain facade for email.
//!
//! Wraps [`SmtpClient`] and provides:
//! - Email sending (delegated to `SmtpClient`)
//! - Template management (load/save/list)
//! - Sent history recording (via `StateStore`)
//! - EventBus notification on send
//! - Rate limit tracking

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;

use crate::email::SmtpClient;
use crate::event_bus::{EventBus, KernelEvent};
use crate::state_store::StateStore;

/// Email API facade — typed API in [`KernelHandle`].
///
/// Constructed during kernel assembly (only when `[email]` is configured)
/// and stored in `KernelHandle.email`.
#[derive(Clone)]
pub struct EmailApi {
    /// SMTP client for sending emails.
    smtp: Arc<SmtpClient>,
    /// Template directory (`~/.oxios/workspace/email_templates/`).
    template_dir: PathBuf,
    /// State store for sent history.
    state_store: Arc<StateStore>,
    /// Optional event bus for notifications.
    event_bus: Option<EventBus>,
    /// Rate limit (emails per hour).
    rate_limit: usize,
}

impl std::fmt::Debug for EmailApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailApi")
            .field("template_dir", &self.template_dir)
            .finish()
    }
}

impl EmailApi {
    /// Create a new `EmailApi`.
    pub fn new(
        smtp: SmtpClient,
        template_dir: PathBuf,
        state_store: Arc<StateStore>,
        event_bus: Option<EventBus>,
        rate_limit: usize,
    ) -> Self {
        // Ensure template directory exists
        let _ = std::fs::create_dir_all(&template_dir);
        Self {
            smtp: Arc::new(smtp),
            template_dir,
            state_store,
            event_bus,
            rate_limit,
        }
    }

    /// Send an email (delegated to `SmtpClient`).
    pub async fn send(
        &self,
        subject: &str,
        html: &str,
        text: Option<&str>,
    ) -> anyhow::Result<crate::email::SendReceipt> {
        self.smtp.send("", subject, html, text).await
    }

    /// Test the SMTP connection.
    pub async fn test_connection(&self) -> anyhow::Result<()> {
        self.smtp.test_connection().await
    }

    /// The default recipient address (user's own email).
    pub fn default_to(&self) -> &str {
        self.smtp.default_to()
    }

    /// The sender address.
    pub fn from_addr(&self) -> &str {
        self.smtp.from_addr()
    }

    // ── Templates ──────────────────────────────────────────────────

    /// Load a template by name.
    ///
    /// Templates are stored as `email_templates/<name>.html`.
    pub fn load_template(&self, name: &str) -> anyhow::Result<String> {
        let path = self.template_dir.join(format!("{name}.html"));
        anyhow::ensure!(path.exists(), "Template '{name}' not found");
        let content = std::fs::read_to_string(&path)?;
        Ok(content)
    }

    /// Save a template.
    pub fn save_template(&self, name: &str, html: &str) -> anyhow::Result<()> {
        let _ = std::fs::create_dir_all(&self.template_dir);
        let path = self.template_dir.join(format!("{name}.html"));
        std::fs::write(&path, html)?;
        tracing::info!(template = %name, "Email template saved");
        Ok(())
    }

    /// List all available template names.
    pub fn list_templates(&self) -> anyhow::Result<Vec<String>> {
        if !self.template_dir.exists() {
            return Ok(Vec::new());
        }
        let mut templates = Vec::new();
        for entry in std::fs::read_dir(&self.template_dir)? {
            let entry = entry?;
            if let Some(name) = entry.path().file_stem()
                && entry.path().extension().is_some_and(|ext| ext == "html")
            {
                templates.push(name.to_string_lossy().to_string());
            }
        }
        templates.sort();
        Ok(templates)
    }

    // ── Sent History ───────────────────────────────────────────────

    /// Save a sent email record to the state store.
    ///
    /// Filename format: `{timestamp}_{short_id}.json` for rate-limit parsing.
    pub async fn save_sent_record<T: Serialize>(&self, record: &T) -> anyhow::Result<()> {
        let val = serde_json::to_value(record)?;
        let id = val.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let sent_at = val.get("sent_at").and_then(|v| v.as_str()).unwrap_or("");
        // Build filename: 2026-06-06_080012_{short_id}.json
        let ts = sent_at
            .get(..19)
            .unwrap_or("unknown")
            .replace([':', '-'], "");
        // sent_at is RFC3339: "2026-06-06T08:00:12+09:00"
        // Extract YYYYMMDD_HHMMSS
        let ts_filename = if sent_at.len() >= 19 {
            let d = &sent_at[..10].replace('-', ""); // 20260606
            let t = &sent_at[11..19].replace(':', ""); // 080012
            format!("{d}_{t}")
        } else {
            ts
        };
        let short_id = &id[..8.min(id.len())];
        let filename = format!("{ts_filename}_{short_id}");
        self.state_store
            .save_json("email_sent", &filename, record)
            .await
    }

    /// Count emails sent in the last `hours` hours (for rate limiting).
    ///
    /// Expects filenames: `YYYYMMDD_HHMMSS_shortid.json`
    pub async fn count_recent_sent(&self, hours: u64) -> anyhow::Result<usize> {
        let sent_dir = self.state_store.base_path.join("email_sent");
        if !sent_dir.exists() {
            return Ok(0);
        }

        let cutoff = Utc::now() - chrono::Duration::hours(hours as i64);
        let mut count = 0;

        for entry in std::fs::read_dir(&sent_dir)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            // Filename: 20260606_080012_abcd1234.json
            // Validate the 15-byte timestamp prefix byte-wise before slicing,
            // so an externally-written or legacy file with the wrong shape
            // (shorter than 15 bytes, non-ASCII, missing `_`) is skipped
            // instead of panicking the kernel during a rate-limit check.
            let filename = entry.file_name().to_string_lossy().into_owned();
            let stem = filename.strip_suffix(".json").unwrap_or(&filename);
            let bytes = stem.as_bytes();
            if bytes.len() < 15 || bytes[8] != b'_' {
                tracing::debug!(
                    filename = %filename,
                    "email_sent: skipping file with unexpected name format"
                );
                continue;
            }
            let digits_ok = bytes[0..8].iter().all(|b| b.is_ascii_digit())
                && bytes[9..15].iter().all(|b| b.is_ascii_digit());
            if !digits_ok {
                continue;
            }
            // ASCII-only prefix → byte slicing is char-aligned.
            let take = |range: std::ops::Range<usize>| -> &str {
                std::str::from_utf8(&bytes[range]).expect("validated ASCII prefix")
            };
            let datetime_str = format!(
                "{}-{}-{}T{}:{}:{}",
                take(0..4),
                take(4..6),
                take(6..8),
                take(9..11),
                take(11..13),
                take(13..15)
            );
            if let Ok(dt) =
                chrono::NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%dT%H:%M:%S")
                && dt.and_utc() > cutoff
            {
                count += 1;
            }
        }

        Ok(count)
    }

    // ── EventBus ───────────────────────────────────────────────────

    /// Publish an `EmailSent` event to the event bus.
    pub fn notify_sent(&self, subject: String, message_id: String, template_name: Option<String>) {
        if let Some(bus) = &self.event_bus {
            let _ = bus.publish(KernelEvent::EmailSent {
                subject,
                message_id,
                template_name,
            });
        }
    }

    /// Rate limit (emails per hour).
    pub fn rate_limit(&self) -> usize {
        self.rate_limit
    }
}
