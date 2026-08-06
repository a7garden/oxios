//! Timeline API — first-party app-module Facade (oxiline integration).
//!
//! Wraps [`oxiline_core`] (SQLite-backed time tracking) behind a clean async,
//! read-only API for the rest of the kernel. **v1 is context-in only**: agents
//! observe the user's current activity, today's plan compliance, and recent
//! records — they do not mutate the timeline. oxios is a *co-client* of the
//! oxiline store: it shares oxiline's canonical SQLite database but never
//! replaces it as the owner.
//!
//! Only present when the `timeline` cargo feature is enabled. When the feature
//! is off, `TimelineApi` is an empty unit struct so the
//! `KernelHandle.timeline: Option<TimelineApi>` field still type-checks (it is
//! always `None`).
//!
//! Concurrency: oxiline's WAL mode + `busy_timeout` make concurrent GUI/CLI/
//! oxios access safe. oxios holds its own `Connection` (one more reader);
//! `open_and_migrate` is idempotent.

// `oxiline_core::CoreError` is fine; no large-err concern here, but keep parity
// with the memo facade's lint hygiene.
#![allow(clippy::result_large_err)]

#[cfg(feature = "timeline")]
use std::sync::Arc;

#[cfg(feature = "timeline")]
use chrono::Utc;

#[cfg(feature = "timeline")]
use oxiline_core::{activities, db, model, paths, record, util};

#[cfg(feature = "timeline")]
const TS_FMT: &str = "%Y-%m-%dT%H:%M:%SZ";

/// Timeline facade over the user's oxiline store (read-only, context-in).
///
/// Held behind `Arc` and shared via `Arc<Mutex<rusqlite::Connection>>` because
/// `rusqlite::Connection` is `!Sync`; every query locks, does sync DB work on a
/// `spawn_blocking` thread, then releases.
#[cfg(feature = "timeline")]
pub struct TimelineApi {
    conn: Arc<parking_lot::Mutex<rusqlite::Connection>>,
}

#[cfg(feature = "timeline")]
impl TimelineApi {
    /// Open (and migrate, idempotently) the oxiline database, wrapping it in a
    /// facade. `db_path = None` uses oxiline's default location
    /// (`oxiline_core::paths::db_path`, honoring `OXILINE_DB_PATH`).
    pub fn open(db_path: Option<&std::path::Path>) -> anyhow::Result<Arc<Self>> {
        let path = db_path
            .map(std::path::PathBuf::from)
            .unwrap_or_else(paths::db_path);
        let conn = db::open_and_migrate(&path)
            .map_err(|e| anyhow::anyhow!("open oxiline db at {}: {e}", path.display()))?;
        Ok(Arc::new(Self {
            conn: Arc::new(parking_lot::Mutex::new(conn)),
        }))
    }

    /// Current activity + today's plan compliance (the `now` view).
    pub async fn now(&self) -> anyhow::Result<model::RecordState> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            record::current(&conn, Utc::now(), &util::today_local())
        })
        .await
        .map_err(|e| anyhow::anyhow!("timeline task join error: {e}"))?
        .map_err(|e| anyhow::anyhow!("oxiline: {e}"))
    }

    /// Activities defined in the store (`active_only` filters soft-deleted).
    pub async fn activities(&self, active_only: bool) -> anyhow::Result<Vec<model::Activity>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            activities::list_activities(&conn, active_only)
        })
        .await
        .map_err(|e| anyhow::anyhow!("timeline task join error: {e}"))?
        .map_err(|e| anyhow::anyhow!("oxiline: {e}"))
    }

    /// Recent records (newest-first), spanning the last `days_back` days,
    /// capped at `limit`.
    pub async fn timeline(&self, days_back: u32, limit: u32) -> anyhow::Result<Vec<model::Record>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> oxiline_core::Result<Vec<model::Record>> {
            let conn = conn.lock();
            let to = Utc::now();
            let from = to - chrono::Duration::days(days_back.max(1) as i64);
            let mut recs = record::list_records(
                &conn,
                None,
                &from.format(TS_FMT).to_string(),
                &to.format(TS_FMT).to_string(),
            )?;
            // newest-first, then cap (list_records returns oldest-first).
            recs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
            recs.truncate(limit.max(1) as usize);
            Ok(recs)
        })
        .await
        .map_err(|e| anyhow::anyhow!("timeline task join error: {e}"))?
        .map_err(|e| anyhow::anyhow!("oxiline: {e}"))
    }
}

// ── Feature-off stub ──────────────────────────────────────────────────────
/// Empty placeholder so `Option<TimelineApi>` type-checks without the feature.
#[cfg(not(feature = "timeline"))]
#[derive(Debug, Clone, Copy)]
pub struct TimelineApi;

#[cfg(all(test, feature = "timeline"))]
mod tests {
    use super::*;

    /// Round-trip: seed an oxiline store (activity + a completed record) via
    /// oxiline-core's own write API on one connection, then read it back
    /// through the `TimelineApi` facade — which opens its own connection to
    /// the same WAL database. Proves the facade exercises oxiline-core's real
    /// schema: `open_and_migrate` runs, and all three read paths
    /// (`activities`, `now`, `timeline`) see the seeded data.
    #[tokio::test]
    async fn timeline_read_round_trip() {
        let dir = tempfile::TempDir::new().expect("temp timeline dir");
        let db_path = dir.path().join("oxiline.db");

        // Seed on a writer connection (the facade is read-only by design).
        {
            let conn = db::open_and_migrate(&db_path).expect("migrate");
            let activity = activities::create_activity(
                &conn,
                model::ActivityInput {
                    name: Some("writing".into()),
                    is_active: Some(true),
                    ..Default::default()
                },
            )
            .expect("create activity");
            let today = util::today_local();
            let started = Utc::now();
            record::start(&conn, &activity.id, started, &today).expect("start");
            record::stop(&conn, started + chrono::Duration::minutes(2), &today).expect("stop");
        }

        // Read back through the facade (separate connection, same WAL db).
        let api = TimelineApi::open(Some(&db_path)).expect("open facade");

        let acts = api.activities(true).await.expect("activities");
        assert!(
            acts.iter().any(|a| a.name == "writing"),
            "seeded activity must be visible through the facade"
        );

        // After `stop` there is no active record.
        let now_state = api.now().await.expect("now");
        assert!(now_state.active.is_none(), "no active record after stop");

        // The completed record shows up in the recent window.
        let recs = api.timeline(1, 10).await.expect("timeline");
        assert!(
            !recs.is_empty(),
            "seeded record must appear in the recent timeline"
        );
    }
}
