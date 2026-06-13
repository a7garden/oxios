//! Calendar tool — wraps `CalendarApi` behind the `AgentTool` interface.
//!
//! Provides agents with calendar event management capabilities.
//! Operations: create, update, delete, list, get, search, freebusy.
//!
//! ## Example
//!
//! ```json
//! { "op": "create", "title": "Team standup", "start": "2026-06-07T09:00:00Z", "end": "2026-06-07T09:30:00Z" }
//! { "op": "list", "from": "2026-06-07T00:00:00Z", "to": "2026-06-14T00:00:00Z" }
//! { "op": "search", "query": "standup" }
//! { "op": "freebusy", "from": "2026-06-07T00:00:00Z", "to": "2026-06-14T00:00:00Z" }
//! { "op": "delete", "uid": "event-uid-here" }
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
use serde_json::{Value, json};

use crate::kernel_handle::KernelHandle;
use oxios_calendar::{CalendarEngine, EventDraft, EventPatch, Repeat};

/// Agent tool for calendar event management.
///
/// Wraps the [`CalendarApi`](crate::kernel_handle::CalendarApi) domain. Allows agents
/// to create, update, delete, list, get, search events and query free-busy slots.
///
/// ## Operations
///
/// | Op         | Description               | Required params                     | Optional params                          |
/// |------------|---------------------------|-------------------------------------|------------------------------------------|
/// | `create`   | Create a new event        | `title`, `start`, `end`             | `all_day`, `description`, `location`, `repeat`, `reminder_minutes` |
/// | `update`   | Update an existing event  | `uid`                               | `title`, `start`, `end`, `all_day`, `description`, `location`, `repeat`, `reminder_minutes` |
/// | `delete`   | Delete an event           | `uid`                               | —                                        |
/// | `list`     | List events in range      | `from`, `to`                        | —                                        |
/// | `get`      | Get a single event        | `uid`                               | —                                        |
/// | `search`   | Full-text search events   | `query`                             | —                                        |
/// | `freebusy` | Free/busy slots in range  | `from`, `to`                        | —                                        |
pub struct CalendarTool {
    engine: Arc<CalendarEngine>,
}

impl CalendarTool {
    /// Create a new `CalendarTool` from a `KernelHandle`.
    ///
    /// Returns `None` if calendar is not configured.
    pub fn try_from_kernel(kernel: &KernelHandle) -> Option<Self> {
        kernel.calendar.as_ref().map(|api| Self {
            engine: api.engine.clone(),
        })
    }

    /// Create a new `CalendarTool` from a `KernelHandle` (required).
    ///
    /// Panics if calendar is not configured. Use [`try_from_kernel`] for
    /// the optional variant.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self::try_from_kernel(kernel).expect("CalendarTool requires calendar to be configured")
    }
}

impl std::fmt::Debug for CalendarTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CalendarTool").finish()
    }
}

/// Parse an ISO 8601 datetime string into a `chrono::DateTime<chrono::Utc>`.
fn parse_dt(s: &str) -> Result<chrono::DateTime<chrono::Utc>, String> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| {
            format!(
                "Invalid datetime '{s}': {e}. Use ISO 8601 / RFC 3339 format, e.g. \"2026-06-07T09:00:00Z\""
            )
        })
}

/// Extract an optional string parameter.
fn opt_str<'a>(params: &'a Value, key: &str) -> Option<&'a str> {
    params.get(key).and_then(|v| v.as_str())
}

/// Extract an optional boolean parameter.
fn opt_bool(params: &Value, key: &str) -> Option<bool> {
    params.get(key).and_then(|v| v.as_bool())
}

/// Extract an optional `reminder_minutes` array (Vec<u32>).
fn opt_reminder_minutes(params: &Value) -> Option<Vec<u32>> {
    params
        .get("reminder_minutes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect()
        })
}

/// Extract an optional `repeat` object and parse into a [`Repeat`].
fn opt_repeat(params: &Value) -> Option<Repeat> {
    let obj = params.get("repeat")?.as_object()?;
    let frequency = obj
        .get("frequency")
        .and_then(|v| v.as_str())
        .unwrap_or("daily")
        .to_string();
    let interval = obj.get("interval").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let days = obj
        .get("days")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let until = obj
        .get("until")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let count = obj.get("count").and_then(|v| v.as_u64()).map(|v| v as u32);

    Some(Repeat {
        frequency,
        days,
        interval,
        until,
        count,
    })
}

#[async_trait]

impl AgentTool for CalendarTool {
    fn name(&self) -> &str {
        "calendar"
    }

    fn label(&self) -> &str {
        "Calendar"
    }

    fn description(&self) -> &'static str {
        "Manage calendar events — create, update, delete, list, search, freebusy. \
         All datetimes use ISO 8601 / RFC 3339 format (e.g. \"2026-06-07T09:00:00Z\")."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "op": {
                    "type": "string",
                    "enum": ["create", "update", "delete", "list", "get", "search", "freebusy"],
                    "description": "Calendar operation to perform"
                },
                "title": {
                    "type": "string",
                    "description": "Event title (required for create, optional for update)"
                },
                "start": {
                    "type": "string",
                    "description": "Event start time (ISO 8601). Required for create, optional for update."
                },
                "end": {
                    "type": "string",
                    "description": "Event end time (ISO 8601). Required for create, optional for update."
                },
                "all_day": {
                    "type": "boolean",
                    "description": "Whether this is an all-day event"
                },
                "description": {
                    "type": "string",
                    "description": "Event description / notes"
                },
                "location": {
                    "type": "string",
                    "description": "Event location"
                },
                "repeat": {
                    "type": "object",
                    "description": "Recurrence rule",
                    "properties": {
                        "frequency": {
                            "type": "string",
                            "enum": ["daily", "weekly", "monthly", "yearly"],
                            "description": "Recurrence frequency"
                        },
                        "interval": {
                            "type": "integer",
                            "description": "Recurrence interval (default: 1)"
                        },
                        "days": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "For weekly: ['mon','wed','fri']"
                        },
                        "count": {
                            "type": "integer",
                            "description": "Max number of occurrences"
                        },
                        "until": {
                            "type": "string",
                            "description": "End date for recurrence (ISO date, e.g. '2026-12-31')"
                        }
                    }
                },
                "reminder_minutes": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Minutes before event to trigger reminders, e.g. [5, 15, 60]"
                },
                "uid": {
                    "type": "string",
                    "description": "Event UID (required for update, delete, get)"
                },
                "from": {
                    "type": "string",
                    "description": "Range start time for list/freebusy (ISO 8601)"
                },
                "to": {
                    "type": "string",
                    "description": "Range end time for list/freebusy (ISO 8601)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query for event title/description (search op)"
                }
            },
            "required": ["op"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<tokio::sync::oneshot::Receiver<()>>,
        _ctx: &ToolContext,
    ) -> Result<AgentToolResult, oxi_sdk::ToolError> {
        let op = params
            .get("op")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: op".to_string())?;

        match op {
            "create" => self.exec_create(&params).await,
            "update" => self.exec_update(&params).await,
            "delete" => self.exec_delete(&params).await,
            "list" => self.exec_list(&params).await,
            "get" => self.exec_get(&params).await,
            "search" => self.exec_search(&params).await,
            "freebusy" => self.exec_freebusy(&params).await,
            other => Err(format!(
                "Unknown calendar op '{other}'. Valid: create, update, delete, list, get, search, freebusy"
            )),
        }
    }
}

impl CalendarTool {
    async fn exec_create(&self, params: &Value) -> Result<AgentToolResult, String> {
        let title = opt_str(params, "title")
            .ok_or_else(|| "create requires 'title' parameter".to_string())?;
        let start = opt_str(params, "start")
            .ok_or_else(|| "create requires 'start' parameter".to_string())?;
        let end =
            opt_str(params, "end").ok_or_else(|| "create requires 'end' parameter".to_string())?;

        let start_dt = parse_dt(start)?;
        let end_dt = parse_dt(end)?;

        let draft = EventDraft {
            title: title.to_string(),
            start: start_dt,
            end: end_dt,
            all_day: opt_bool(params, "all_day").unwrap_or(false),
            description: opt_str(params, "description").map(|s| s.to_string()),
            location: opt_str(params, "location").map(|s| s.to_string()),
            repeat: opt_repeat(params),
            reminder_minutes: opt_reminder_minutes(params).unwrap_or_default(),
            source: oxios_calendar::EventSource::Agent,
        };

        match self.engine.create(draft).await {
            Ok(result) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "uid": result.uid,
                    "status": "created",
                    "conflicts": result.conflicts,
                    "file": result.file,
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!(
                "Failed to create event: {e}"
            ))),
        }
    }

    async fn exec_update(&self, params: &Value) -> Result<AgentToolResult, String> {
        let uid =
            opt_str(params, "uid").ok_or_else(|| "update requires 'uid' parameter".to_string())?;

        let patch = EventPatch {
            title: opt_str(params, "title").map(|s| s.to_string()),
            start: opt_str(params, "start").and_then(|s| parse_dt(s).ok()),
            end: opt_str(params, "end").and_then(|s| parse_dt(s).ok()),
            all_day: opt_bool(params, "all_day"),
            description: opt_str(params, "description").map(|s| Some(s.to_string())),
            location: opt_str(params, "location").map(|s| Some(s.to_string())),
            repeat: opt_repeat(params).map(Some),
            reminder_minutes: opt_reminder_minutes(params),
        };

        match self.engine.update(uid, patch).await {
            Ok(result) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "uid": result.uid,
                    "status": "updated",
                    "conflicts": result.conflicts,
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!(
                "Failed to update event: {e}"
            ))),
        }
    }

    async fn exec_delete(&self, params: &Value) -> Result<AgentToolResult, String> {
        let uid =
            opt_str(params, "uid").ok_or_else(|| "delete requires 'uid' parameter".to_string())?;

        match self.engine.delete(uid).await {
            Ok(()) => Ok(AgentToolResult::success(format!("Event '{uid}' deleted."))),
            Err(e) => Ok(AgentToolResult::error(format!(
                "Failed to delete event: {e}"
            ))),
        }
    }

    async fn exec_list(&self, params: &Value) -> Result<AgentToolResult, String> {
        let from =
            opt_str(params, "from").ok_or_else(|| "list requires 'from' parameter".to_string())?;
        let to = opt_str(params, "to").ok_or_else(|| "list requires 'to' parameter".to_string())?;

        let from_dt = parse_dt(from)?;
        let to_dt = parse_dt(to)?;

        match self.engine.list(from_dt, to_dt).await {
            Ok(events) => {
                if events.is_empty() {
                    return Ok(AgentToolResult::success("No events in the given range."));
                }
                let display: Vec<Value> = events
                    .iter()
                    .map(|e| {
                        json!({
                            "uid": e.uid,
                            "title": e.title,
                            "start": e.start.to_rfc3339(),
                            "end": e.end.to_rfc3339(),
                            "status": e.status,
                        })
                    })
                    .collect();
                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&json!({
                        "events": display,
                        "count": display.len(),
                    }))
                    .unwrap_or_default(),
                ))
            }
            Err(e) => Ok(AgentToolResult::error(format!(
                "Failed to list events: {e}"
            ))),
        }
    }

    async fn exec_get(&self, params: &Value) -> Result<AgentToolResult, String> {
        let uid =
            opt_str(params, "uid").ok_or_else(|| "get requires 'uid' parameter".to_string())?;

        match self.engine.get(uid).await {
            Ok(event) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "uid": event.uid,
                    "title": event.title,
                    "start": event.start.to_rfc3339(),
                    "end": event.end.to_rfc3339(),
                    "all_day": event.all_day,
                    "description": event.description,
                    "location": event.location,
                    "rrule": event.rrule,
                    "status": event.status,
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!("Failed to get event: {e}"))),
        }
    }

    async fn exec_search(&self, params: &Value) -> Result<AgentToolResult, String> {
        let query = opt_str(params, "query")
            .ok_or_else(|| "search requires 'query' parameter".to_string())?;

        match self.engine.search(query).await {
            Ok(events) => {
                if events.is_empty() {
                    return Ok(AgentToolResult::success(format!(
                        "No events matching '{query}'."
                    )));
                }
                let display: Vec<Value> = events
                    .iter()
                    .map(|e| {
                        json!({
                            "uid": e.uid,
                            "title": e.title,
                            "start": e.start.to_rfc3339(),
                            "end": e.end.to_rfc3339(),
                        })
                    })
                    .collect();
                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&json!({
                        "events": display,
                        "count": display.len(),
                        "query": query,
                    }))
                    .unwrap_or_default(),
                ))
            }
            Err(e) => Ok(AgentToolResult::error(format!(
                "Failed to search events: {e}"
            ))),
        }
    }

    async fn exec_freebusy(&self, params: &Value) -> Result<AgentToolResult, String> {
        let from = opt_str(params, "from")
            .ok_or_else(|| "freebusy requires 'from' parameter".to_string())?;
        let to =
            opt_str(params, "to").ok_or_else(|| "freebusy requires 'to' parameter".to_string())?;

        let from_dt = parse_dt(from)?;
        let to_dt = parse_dt(to)?;

        match self.engine.freebusy(from_dt, to_dt).await {
            Ok(slots) => Ok(AgentToolResult::success(
                serde_json::to_string_pretty(&json!({
                    "slots": slots,
                    "count": slots.len(),
                }))
                .unwrap_or_default(),
            )),
            Err(e) => Ok(AgentToolResult::error(format!(
                "Failed to compute freebusy: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_dt_valid() {
        let dt = parse_dt("2026-06-07T09:00:00Z").unwrap();
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 7);
    }

    #[test]
    fn test_parse_dt_invalid() {
        assert!(parse_dt("not-a-date").is_err());
    }

    #[test]
    fn test_opt_repeat_basic() {
        let params = json!({
            "repeat": {
                "frequency": "weekly",
                "days": ["mon", "wed"],
                "interval": 2,
                "count": 10
            }
        });
        let rule = opt_repeat(&params).unwrap();
        assert_eq!(rule.frequency, "weekly");
        assert_eq!(rule.days, vec!["mon", "wed"]);
        assert_eq!(rule.interval, 2);
        assert_eq!(rule.count, Some(10));
        assert!(rule.until.is_none());
    }
}
