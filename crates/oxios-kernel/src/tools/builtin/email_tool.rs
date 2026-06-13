//! Email tool — wraps `SmtpClient` behind the `AgentTool` interface.
//!
//! Provides agents with email sending capabilities. Agents compose HTML,
//! manage templates, and decide content — we only provide the SMTP pipe.
//!
//! ## Actions
//!
//! | Mode | Description | Required params |
//! |------|-------------|-----------------|
//! | Send (direct) | Send an HTML email | `subject`, `body_html` |
//! | Send (template) | Send using a saved template | `subject`, `use_template` |
//! | Save template | Send + save as template | `subject`, `body_html` or `use_template`, `save_template_as` |
//! | List templates | List available templates | `list_templates: true` |

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use oxi_sdk::{AgentTool as OxiAgentTool, AgentToolResult, ToolContext};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::kernel_handle::EmailApi;

/// Maximum HTML body size (1 MB).
const MAX_HTML_BYTES: usize = 1_000_000;
/// Maximum subject length.
const MAX_SUBJECT_LEN: usize = 200;

/// Arguments for the `send_email` tool.
#[derive(Debug, Deserialize)]
struct EmailArgs {
    /// Email subject line.
    subject: Option<String>,
    /// HTML body (full document or body fragment).
    body_html: Option<String>,
    /// Plain text fallback (recommended but optional).
    body_text: Option<String>,
    /// Save this email as a reusable template.
    save_template_as: Option<String>,
    /// Use a saved template (body_html is ignored).
    use_template: Option<String>,
    /// Key-value pairs to substitute in template. `{{key}}` → value.
    template_vars: Option<HashMap<String, String>>,
    /// If true, list available templates and return.
    list_templates: Option<bool>,
}

/// A single sent email record (stored in `email_sent/`).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SentRecord {
    /// Unique ID.
    id: String,
    /// Timestamp.
    sent_at: String,
    /// Email subject.
    subject: String,
    /// Recipient (always `my_email` in v1).
    to: String,
    /// Template used (if any).
    template_used: Option<String>,
    /// SMTP message ID.
    message_id: String,
    /// First 500 chars of HTML for preview.
    html_preview: String,
    /// Full HTML body (원문).
    html_full: String,
    /// Plain text fallback.
    body_text: Option<String>,
    /// Associated cron job name (if triggered by cron).
    cron_job: Option<String>,
}

/// Email tool — provides `send_email` to agents.
///
/// Wraps [`EmailApi`] and adds:
/// - Template loading/saving/rendering
/// - Rate limiting
/// - Sent history recording
/// - EventBus notification on success
pub struct EmailTool {
    api: Arc<EmailApi>,
}

impl EmailTool {
    /// Create a new `EmailTool` from a `KernelHandle`.
    ///
    /// Returns `None` if email is not configured.
    pub fn try_from_kernel(kernel: &crate::KernelHandle) -> Option<Self> {
        kernel.email.as_ref().map(|api| Self {
            api: Arc::new(api.clone()),
        })
    }
}

impl std::fmt::Debug for EmailTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailTool").finish()
    }
}

#[async_trait]

impl OxiAgentTool for EmailTool {
    fn name(&self) -> &str {
        "send_email"
    }

    fn label(&self) -> &str {
        "Send Email"
    }

    fn description(&self) -> &'static str {
        "Compose and send an HTML email. You decide the format, layout, and content. \
         For recurring sends, save as template and reuse. Templates are stored in \
         ~/.oxios/workspace/email_templates/."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "Email subject line"
                },
                "body_html": {
                    "type": "string",
                    "description": "HTML body. Full <html> document or <body> fragment. Inline CSS only (email clients strip <style>)."
                },
                "body_text": {
                    "type": "string",
                    "description": "Plain text fallback. Optional but recommended for accessibility."
                },
                "save_template_as": {
                    "type": "string",
                    "description": "Save this email as a reusable template with this name. Stored in email_templates/<name>.html"
                },
                "use_template": {
                    "type": "string",
                    "description": "Name of a saved template to use. body_html is ignored; template_vars are substituted."
                },
                "template_vars": {
                    "type": "object",
                    "description": "Key-value pairs to substitute in template. {{key}} → value."
                },
                "list_templates": {
                    "type": "boolean",
                    "description": "If true, list available templates and return. All other params ignored."
                }
            }
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let args: EmailArgs =
            serde_json::from_value(params).map_err(|e| format!("Invalid arguments: {e}"))?;

        // ── List templates mode ───────────────────────────────────
        if args.list_templates.unwrap_or(false) {
            let templates = self
                .api
                .list_templates()
                .map_err(|e| format!("Failed to list templates: {e}"))?;
            return Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "templates": templates,
                }))
                .unwrap_or_default(),
            ));
        }

        // ── Validate subject ──────────────────────────────────────
        let subject = args.subject.as_deref().ok_or("subject is required")?;

        if subject.len() > MAX_SUBJECT_LEN {
            return Err(format!(
                "Subject too long ({} chars, max {})",
                subject.len(),
                MAX_SUBJECT_LEN
            ));
        }

        // ── Resolve HTML body ─────────────────────────────────────
        let html = if let Some(name) = &args.use_template {
            let template = self
                .api
                .load_template(name)
                .map_err(|e| format!("Template error: {e}"))?;
            render_template(&template, &args.template_vars.unwrap_or_default())
                .map_err(|e| format!("Template render error: {e}"))?
        } else {
            args.body_html
                .as_deref()
                .ok_or("body_html or use_template is required")?
                .to_string()
        };

        // ── Validate HTML size ────────────────────────────────────
        if html.len() > MAX_HTML_BYTES {
            return Err(format!(
                "HTML body too large ({} bytes, max {} bytes)",
                html.len(),
                MAX_HTML_BYTES
            ));
        }

        // ── Rate limit check ──────────────────────────────────────
        let rate_limit = self.api.rate_limit();
        let sent_count = self
            .api
            .count_recent_sent(1)
            .await
            .map_err(|e| format!("Rate limit check failed: {e}"))?;
        if sent_count >= rate_limit {
            return Err(format!(
                "Rate limit: {rate_limit} emails per hour. Try later."
            ));
        }

        // ── Send ──────────────────────────────────────────────────
        let receipt = self
            .api
            .send(subject, &html, args.body_text.as_deref())
            .await
            .map_err(|e| format!("SMTP send failed: {e}"))?;

        // ── Save template (if requested) ──────────────────────────
        if let Some(name) = &args.save_template_as {
            self.api
                .save_template(name, &html)
                .map_err(|e| format!("Failed to save template: {e}"))?;
        }

        // ── Record sent history ───────────────────────────────────
        let record = SentRecord {
            id: uuid::Uuid::new_v4().to_string(),
            sent_at: receipt.sent_at.to_rfc3339(),
            subject: subject.to_string(),
            to: self.api.default_to().to_string(),
            template_used: args.use_template.clone().or(args.save_template_as.clone()),
            message_id: receipt.message_id.clone(),
            html_preview: html.chars().take(500).collect(),
            html_full: html,
            body_text: args.body_text,
            cron_job: None,
        };
        if let Err(e) = self.api.save_sent_record(&record).await {
            tracing::warn!(error = %e, "Failed to save email sent record");
        }

        // ── EventBus notification ─────────────────────────────────
        self.api.notify_sent(
            subject.to_string(),
            receipt.message_id.clone(),
            args.save_template_as.clone(),
        );

        Ok(AgentToolResult::success(
            serde_json::to_string_pretty(&json!({
                "status": "sent",
                "message_id": receipt.message_id,
                "template_saved": args.save_template_as.is_some(),
            }))
            .unwrap_or_default(),
        ))
    }
}

/// Render a template by substituting `{{key}}` placeholders.
///
/// Keys not found in `vars` are left as-is (not stripped).
fn render_template(template: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}"); // {{key}}
        result = result.replace(&placeholder, value);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_template_basic() {
        let template = "<h1>Hello {{name}}</h1><p>{{message}}</p>";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "World".to_string());
        vars.insert("message".to_string(), "Welcome!".to_string());

        let result = render_template(template, &vars).unwrap();
        assert_eq!(result, "<h1>Hello World</h1><p>Welcome!</p>");
    }

    #[test]
    fn test_render_template_missing_vars_left_as_is() {
        let template = "<h1>Hello {{name}}</h1><p>{{missing}}</p>";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "World".to_string());

        let result = render_template(template, &vars).unwrap();
        assert_eq!(result, "<h1>Hello World</h1><p>{{missing}}</p>");
    }

    #[test]
    fn test_render_template_empty_vars() {
        let template = "<h1>Hello {{name}}</h1>";
        let vars = HashMap::new();

        let result = render_template(template, &vars).unwrap();
        assert_eq!(result, "<h1>Hello {{name}}</h1>");
    }

    #[test]
    fn test_render_template_html_in_values() {
        let template = "<ul>{{items}}</ul>";
        let mut vars = HashMap::new();
        vars.insert("items".to_string(), "<li>A</li><li>B</li>".to_string());

        let result = render_template(template, &vars).unwrap();
        assert_eq!(result, "<ul><li>A</li><li>B</li></ul>");
    }
}
