//! Archive old calendar events by moving them to an archive directory.

use chrono::Utc;
use tokio::fs;

use crate::engine::CalendarEngine;

/// Move events whose end time is older than `retention_days` to an `archive/`
/// subdirectory within the calendar directory.
///
/// Returns the number of archived events.
pub async fn archive_old_events(
    engine: &CalendarEngine,
    retention_days: u32,
) -> anyhow::Result<u32> {
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let archive_dir = engine.dir().join("archive");
    fs::create_dir_all(&archive_dir).await?;

    // List all events and find old ones
    let all = engine
        .list(
            chrono::DateTime::from_timestamp(0, 0).unwrap_or_default(),
            cutoff,
        )
        .await?;

    let mut archived = 0u32;
    for event in &all {
        if event.end < cutoff && event.status != "CANCELLED" {
            let src = engine.dir().join(&event.filename);
            let dst = archive_dir.join(&event.filename);

            if src.exists() {
                fs::rename(&src, &dst).await?;
                engine.remove_from_index(&event.uid).await?;
                tracing::debug!("Archived event {} ({})", event.uid, event.title);
                archived += 1;
            }
        }
    }

    if archived > 0 {
        engine.save_index().await?;
        tracing::info!(
            "Archived {} old events (retention: {} days)",
            archived,
            retention_days
        );
    }

    Ok(archived)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventDraft;
    use chrono::TimeZone;

    #[tokio::test]
    async fn archive_old() {
        let dir = tempfile::tempdir().unwrap();
        let engine = CalendarEngine::new(dir.path().to_path_buf()).await.unwrap();

        // Create an event in the past
        let past_start = Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap();
        let past_end = Utc.with_ymd_and_hms(2020, 1, 1, 10, 0, 0).unwrap();
        let draft = EventDraft {
            title: "Old Meeting".into(),
            start: past_start,
            end: past_end,
            all_day: false,
            description: None,
            location: None,
            repeat: None,
            reminder_minutes: vec![],
            source: crate::types::EventSource::Agent,
        };

        let result = engine.create(draft).await.unwrap();
        assert!(engine.get(&result.uid).await.is_ok());

        // Archive events older than 30 days
        let count = archive_old_events(&engine, 30).await.unwrap();
        assert_eq!(count, 1);

        // Verify it's gone from the main index
        assert!(engine.get(&result.uid).await.is_err());

        // Verify it exists in archive
        let archive_path = dir.path().join("archive").join(&result.file);
        assert!(archive_path.exists());
    }
}
