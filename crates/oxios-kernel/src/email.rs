//! SMTP email client — lettre wrapper.
//!
//! Sends HTML/plain emails via SMTP. The client is configured once from
//! `config.toml` `[email]` section and reused across all `send_email` calls.
//!
//! ## Providers
//!
//! Preset providers auto-fill SMTP host/port/TLS settings:
//! - `gmail` → smtp.gmail.com:465 / TLS
//! - `icloud` → smtp.mail.me.com:587 / STARTTLS
//! - `fastmail` → smtp.fastmail.com:465 / TLS
//! - `resend` → smtp.resend.com:587 / STARTTLS (API key as password)
//! - `custom` → manual host/port/tls required

use std::sync::Arc;

use chrono::Utc;
use lettre::AsyncTransport;
use lettre::Message;
use lettre::Tokio1Executor;
use lettre::message::MultiPart;
use lettre::message::SinglePart;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use serde::{Deserialize, Serialize};

use crate::config::EmailConfig;

/// SMTP transport type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpTls {
    /// Implicit TLS (port 465).
    Tls,
    /// STARTTLS upgrade (port 587).
    StartTls,
}

/// Preset SMTP provider configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpProvider {
    /// Gmail (smtp.gmail.com:465, TLS).
    Gmail,
    /// iCloud (smtp.mail.me.com:587, STARTTLS).
    Icloud,
    /// Fastmail (smtp.fastmail.com:465, TLS).
    Fastmail,
    /// Resend (smtp.resend.com:587, STARTTLS).
    /// Uses API key as SMTP password; username is always `resend`.
    Resend,
    /// Custom SMTP server (manual host/port/tls).
    Custom,
}

impl SmtpProvider {
    /// Return the default host, port, and TLS mode for this provider.
    pub fn defaults(&self) -> (&'static str, u16, SmtpTls) {
        match self {
            SmtpProvider::Gmail => ("smtp.gmail.com", 465, SmtpTls::Tls),
            SmtpProvider::Icloud => ("smtp.mail.me.com", 587, SmtpTls::StartTls),
            SmtpProvider::Fastmail => ("smtp.fastmail.com", 465, SmtpTls::Tls),
            SmtpProvider::Resend => ("smtp.resend.com", 587, SmtpTls::StartTls),
            SmtpProvider::Custom => ("", 0, SmtpTls::Tls),
        }
    }
}

/// SMTP transport wrapper.
type SmtpTransport = lettre::AsyncSmtpTransport<Tokio1Executor>;

/// Receipt returned after a successful send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendReceipt {
    /// SMTP message ID (from server response).
    pub message_id: String,
    /// Timestamp of successful send.
    pub sent_at: chrono::DateTime<Utc>,
}

/// SMTP client — wraps lettre for sending emails.
///
/// Thread-safe via `Arc<SmtpTransport>`. Created once during kernel init
/// from `[email]` config and stored in `EmailApi`.
pub struct SmtpClient {
    transport: Arc<SmtpTransport>,
    from: String,
    default_to: String,
}

impl std::fmt::Debug for SmtpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpClient")
            .field("from", &self.from)
            .field("default_to", &self.default_to)
            .finish()
    }
}

impl SmtpClient {
    /// Build an `SmtpClient` from config and credentials.
    ///
    /// `password` is the SMTP auth password (app password for Gmail).
    /// It is never stored beyond the lettre transport internals.
    pub fn from_config(config: &EmailConfig, password: &str) -> anyhow::Result<Self> {
        let (default_host, default_port, default_tls) = config.provider().defaults();

        let host = if config.host.is_empty() {
            default_host
        } else {
            &config.host
        };
        let port = if config.port == 0 {
            default_port
        } else {
            config.port
        };
        let tls_mode = config.tls.unwrap_or(default_tls);

        let user = match config.provider {
            SmtpProvider::Resend => "resend".to_string(),
            _ => {
                if config.user.is_empty() {
                    config.my_email.clone()
                } else {
                    config.user.clone()
                }
            }
        };

        anyhow::ensure!(!host.is_empty(), "SMTP host is required");
        anyhow::ensure!(port > 0, "SMTP port is required");

        let creds = Credentials::new(user.clone(), password.to_string());

        let transport = match tls_mode {
            SmtpTls::Tls => {
                // Implicit TLS (port 465)
                lettre::AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
                    .port(port)
                    .credentials(creds)
                    .build()
            }
            SmtpTls::StartTls => {
                // STARTTLS (port 587)
                lettre::AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
                    .port(port)
                    .credentials(creds)
                    .build()
            }
        };

        Ok(Self {
            transport: Arc::new(transport),
            from: config.my_email.clone(),
            default_to: config.my_email.clone(),
        })
    }

    /// Send an email.
    ///
    /// In v1, `to` is ignored — always sends to `default_to` (the user's own email).
    /// If `text` is `None`, a minimal plain-text fallback is generated from the subject.
    pub async fn send(
        &self,
        _to: &str,
        subject: &str,
        html: &str,
        text: Option<&str>,
    ) -> anyhow::Result<SendReceipt> {
        let text_body = text
            .map(|s| s.to_string())
            .unwrap_or_else(|| subject.to_string());

        let email = Message::builder()
            .from(self.from.parse()?)
            .to(self.default_to.parse()?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.to_string()),
                    ),
            )?;

        let _response = self.transport.send(email).await?;
        let message_id = format!("<{}>", uuid::Uuid::new_v4());

        Ok(SendReceipt {
            message_id,
            sent_at: Utc::now(),
        })
    }

    /// Test the SMTP connection by sending a simple test email.
    pub async fn test_connection(&self) -> anyhow::Result<()> {
        let email = Message::builder()
            .from(self.from.parse()?)
            .to(self.default_to.parse()?)
            .subject("Oxios Email Test")
            .body("If you see this, Oxios email is working.".to_string())?;

        self.transport.send(email).await?;
        Ok(())
    }

    /// The "from" address (user's own email).
    pub fn from_addr(&self) -> &str {
        &self.from
    }

    /// The default "to" address (user's own email, same as from in v1).
    pub fn default_to(&self) -> &str {
        &self.default_to
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_defaults() {
        let (host, port, tls) = SmtpProvider::Gmail.defaults();
        assert_eq!(host, "smtp.gmail.com");
        assert_eq!(port, 465);
        assert_eq!(tls, SmtpTls::Tls);

        let (host, port, tls) = SmtpProvider::Icloud.defaults();
        assert_eq!(host, "smtp.mail.me.com");
        assert_eq!(port, 587);
        assert_eq!(tls, SmtpTls::StartTls);

        let (host, port, tls) = SmtpProvider::Fastmail.defaults();
        assert_eq!(host, "smtp.fastmail.com");
        assert_eq!(port, 465);
        assert_eq!(tls, SmtpTls::Tls);

        let (host, port, tls) = SmtpProvider::Resend.defaults();
        assert_eq!(host, "smtp.resend.com");
        assert_eq!(port, 587);
        assert_eq!(tls, SmtpTls::StartTls);

        let (host, port, _) = SmtpProvider::Custom.defaults();
        assert!(host.is_empty());
        assert_eq!(port, 0);
    }
}
