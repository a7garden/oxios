//! Persona tool — wraps `PersonaApi` behind the `AgentTool` interface.
//!
//! Provides agents with persona management capabilities.
//! Actions: list, get, set_active.
//!
//! ## Example
//!
//! ```json
//! { "action": "list" }
//! { "action": "get", "id": "persona-id" }
//! { "action": "set_active", "id": "persona-id" }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::kernel_handle::KernelHandle;
use crate::persona::PersonaManager;

/// Agent tool for persona management.
///
/// Wraps the `PersonaApi` domain of the `KernelHandle`. Allows agents
/// to query and switch between active personas.
///
/// ## Actions
///
/// | Action       | Description              | Required params |
/// |--------------|--------------------------|-----------------|
/// | `list`       | List all personas        | —               |
/// | `get`        | Get persona by ID        | `id`            |
/// | `set_active` | Set the active persona   | `id`            |
pub struct PersonaTool {
    persona_manager: Arc<PersonaManager>,
}

impl PersonaTool {
    /// Create a new `PersonaTool` from a `KernelHandle`.
    ///
    /// Extracts the `PersonaManager` Arc from the kernel's Persona API.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self {
            persona_manager: kernel.persona.persona_manager.clone(),
        }
    }
}

impl std::fmt::Debug for PersonaTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersonaTool").finish()
    }
}

#[async_trait]
impl AgentTool for PersonaTool {
    fn name(&self) -> &str {
        "persona"
    }

    fn label(&self) -> &str {
        "Persona"
    }

    fn description(&self) -> &'static str {
        "Manage personas — list, inspect, or switch the active persona. \
         Actions: list, get, set_active."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "set_active"],
                    "description": "Persona operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Persona identifier (required for get and set_active)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        // Build a temporary PersonaApi to delegate to.
        let api = crate::kernel_handle::PersonaApi::new(self.persona_manager.clone());

        match action {
            "list" => {
                let personas = api.list();
                if personas.is_empty() {
                    return Ok(AgentToolResult::success("No personas defined."));
                }

                // Get active persona ID for display.
                let active_id = api.active().map(|p| p.id.clone());

                let mut output = format!("Found {} persona(s):\n\n", personas.len());
                for p in &personas {
                    let marker = if active_id.as_deref() == Some(&p.id) {
                        " ← active"
                    } else {
                        ""
                    };
                    output.push_str(&format!(
                        "- {} ({}) enabled={}{}\n",
                        p.name, p.id, p.enabled, marker,
                    ));
                }
                Ok(AgentToolResult::success(output))
            }

            "get" => {
                let id = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "get requires 'id' parameter".to_string())?;

                match api.get(id) {
                    Some(p) => Ok(AgentToolResult::success(
                        serde_json::to_string_pretty(&json!({
                            "id": p.id,
                            "name": p.name,
                            "description": p.description,
                            "enabled": p.enabled,
                            "system_prompt": p.system_prompt,
                            "traits": p.personality_traits,
                        }))
                        .unwrap_or_default(),
                    )),
                    None => Ok(AgentToolResult::error(format!("Persona '{id}' not found"))),
                }
            }

            "set_active" => {
                let id = params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "set_active requires 'id' parameter".to_string())?;

                match api.set_active(id) {
                    Ok(()) => Ok(AgentToolResult::success(format!(
                        "Active persona set to '{id}'."
                    ))),
                    Err(e) => Ok(AgentToolResult::error(format!(
                        "Failed to set active persona: {e}"
                    ))),
                }
            }

            other => Err(format!(
                "Unknown persona action '{other}'. Valid: list, get, set_active"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_structure() {
        let schema = json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "set_active"]
                },
                "id": { "type": "string" }
            },
            "required": ["action"]
        });

        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().any(|a| a == "list"));
        assert!(actions.iter().any(|a| a == "get"));
        assert!(actions.iter().any(|a| a == "set_active"));
    }
}
