//! Calendar API — 15th KernelHandle domain Facade.
//!
//! Provides typed access to the calendar engine for the rest of the kernel.
//! Wraps [`oxios_calendar::CalendarEngine`] behind a clean async API.
//! Publishes [`KernelEvent`] variants for calendar mutations.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use oxios_calendar::{
    AlarmEvent, CalendarEngine, CreateResult, Event, EventDraft, EventPatch, FreeBusySlot,
    UpdateResult,
};

use crate::event_bus::{EventBus, KernelEvent};

/// Calendar API facade — 15th typed API in [`KernelHandle`].
///
/// Delegates all operations to the underlying [`CalendarEngine`]
/// and publishes [`KernelEvent`] variants for each mutation.
/// Constructed during kernel assembly and stored in `KernelHandle.calendar`.
pub struct CalendarApi {
    /// The calendar engine.
    pub engine: Arc<CalendarEngine>,
    /// Optional event bus for publishing calendar events.
    event_bus: Option<EventBus>,
}

impl CalendarApi {
    /// Create a new CalendarApi.
    ///
    /// Pass `Some(event_bus)` to publish `CalendarEventCreated/Updated/Deleted`
    /// events; pass `None` for a silent engine-only API. This signature
    /// mirrors `EmailApi::new` so the facade constructors stay symmetric —
    /// callers should not have to remember which facades take an Option and
    /// which split into `new` vs `with_event_bus`.
    pub fn new(engine: Arc<CalendarEngine>, event_bus: Option<EventBus>) -> Self {
        Self { engine, event_bus }
    }

    /// Convenience: same as [`Self::new`] with `Some(event_bus)`.
    pub fn with_event_bus(engine: Arc<CalendarEngine>, event_bus: EventBus) -> Self {
        Self::new(engine, Some(event_bus))
    }

    /// Create a new event and publish a `CalendarEventCreated` event.
    pub async fn create(&self, draft: EventDraft) -> anyhow::Result<CreateResult> {
        let title = draft.title.clone();
        let start = draft.start.to_rfc3339();
        let end = draft.end.to_rfc3339();
        let result = self.engine.create(draft).await?;

        if let Some(bus) = &self.event_bus {
            let _ = bus.publish(KernelEvent::CalendarEventCreated {
                uid: result.uid.clone(),
                title,
                start,
                end,
            });
        }

        Ok(result)
    }

    /// Update an existing event and publish a `CalendarEventUpdated` event.
    pub async fn update(&self, uid: &str, patch: EventPatch) -> anyhow::Result<UpdateResult> {
        let result = self.engine.update(uid, patch).await?;

        if let Some(bus) = &self.event_bus {
            // Fetch the event to get the current title
            let title = self
                .engine
                .get(uid)
                .await
                .map(|e| e.title.clone())
                .unwrap_or_default();

            let _ = bus.publish(KernelEvent::CalendarEventUpdated {
                uid: result.uid.clone(),
                title,
            });
        }

        Ok(result)
    }

    /// Delete an event by UID and publish a `CalendarEventDeleted` event.
    pub async fn delete(&self, uid: &str) -> anyhow::Result<()> {
        // Fetch title before deletion for the event
        let title = self
            .engine
            .get(uid)
            .await
            .map(|e| e.title.clone())
            .unwrap_or_default();

        self.engine.delete(uid).await?;

        if let Some(bus) = &self.event_bus {
            let _ = bus.publish(KernelEvent::CalendarEventDeleted {
                uid: uid.to_string(),
                title,
            });
        }

        Ok(())
    }

    /// Get a single event by UID.
    pub async fn get(&self, uid: &str) -> anyhow::Result<Event> {
        self.engine.get(uid).await
    }

    /// List events in a time range `[from, to)`.
    pub async fn list(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> anyhow::Result<Vec<Event>> {
        self.engine.list(from, to).await
    }

    /// Search events by text query.
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<Event>> {
        self.engine.search(query).await
    }

    /// Compute free/busy slots in a time range.
    pub async fn freebusy(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> anyhow::Result<Vec<FreeBusySlot>> {
        self.engine.freebusy(from, to).await
    }

    /// Find pending alarms in a time range.
    pub fn find_pending_alarms(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<AlarmEvent> {
        self.engine.find_pending_alarms(from, to)
    }
}
