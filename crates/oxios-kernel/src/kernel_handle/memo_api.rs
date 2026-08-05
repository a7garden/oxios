//! Memo API — first-party app-module Facade (oximemo integration).
//!
//! Wraps [`oximemo_core::Vault`] behind a clean async API for the rest of the
//! kernel and publishes [`KernelEvent`](crate::event_bus::KernelEvent) variants
//! for memo mutations.
//!
//! Only present when the `memo` cargo feature is enabled. When the feature is
//! off, `MemoApi` is an empty unit struct so `KernelHandle.memo: Option<MemoApi>`
//! still type-checks (it is always `None`).
//!
//! oxios is a *co-client* of the oximemo vault: it shares oximemo's canonical
//! store but never replaces it as the owner. The vault's per-operation advisory
//! locks ([`oximemo_core`] `lock` module) make concurrent CLI/GUI/oxios access
//! safe.
// `oximemo_core::CoreError` carries redb's large error enums inline (176B Err
// variant) — harmless for this facade's workload; silence the pedantic lint,
// mirroring oximemo-core's own `lib.rs`.
#![allow(clippy::result_large_err)]

#[cfg(feature = "memo")]
use std::sync::Arc;

#[cfg(feature = "memo")]
use oximemo_core::{Memo, MemoFilter, MemoId, MemoSummary, Vault};

#[cfg(feature = "memo")]
use crate::event_bus::{EventBus, KernelEvent};

/// The oximemo Facade — real when the `memo` feature is on.
#[cfg(feature = "memo")]
pub struct MemoApi {
    /// The shared vault handle. `Vault` methods take a per-op advisory lock, so
    /// cheap to share across tasks.
    vault: Arc<Vault>,
    /// Optional event bus for publishing memo mutation events.
    event_bus: Option<EventBus>,
}

/// Empty stub so `Option<MemoApi>` type-checks without the `memo` feature.
#[cfg(not(feature = "memo"))]
#[derive(Debug, Default, Clone)]
pub struct MemoApi;

#[cfg(feature = "memo")]
impl MemoApi {
    /// Open the user's oximemo vault and wrap it. `vault_path = None` uses
    /// oximemo's default location. The caller runs this on a blocking thread
    /// (`Vault::open` does filesystem setup).
    pub fn open(
        vault_path: Option<&std::path::Path>,
        event_bus: Option<EventBus>,
    ) -> anyhow::Result<std::sync::Arc<Self>> {
        let vault = Vault::open(vault_path)?;
        Ok(std::sync::Arc::new(Self::new(Arc::new(vault), event_bus)))
    }

    /// Wrap an already-open vault. Pass `Some(event_bus)` to publish memo events.
    pub fn new(vault: Arc<Vault>, event_bus: Option<EventBus>) -> Self {
        Self { vault, event_bus }
    }

    /// Create a memo, then publish [`KernelEvent::MemoCreated`].
    ///
    /// `category` is an optional category id (e.g. `"inbox"`, `"todo"`).
    pub async fn create_memo(
        &self,
        body: String,
        category: Option<String>,
    ) -> anyhow::Result<Memo> {
        let vault = self.vault.clone();
        // Vault methods are synchronous (per-op flock); off-load to a blocking
        // thread so the async runtime is never stalled.
        let memo = tokio::task::spawn_blocking(move || vault.create_memo(body, category)).await??;
        self.publish(KernelEvent::MemoCreated {
            id: memo.id.to_string(),
        });
        Ok(memo)
    }

    /// Fetch a single memo by id.
    pub async fn get_memo(&self, id: &str) -> anyhow::Result<Memo> {
        let mid = MemoId::parse(id).map_err(anyhow::Error::new)?;
        let vault = self.vault.clone();
        Ok(tokio::task::spawn_blocking(move || vault.get_memo(mid)).await??)
    }

    /// Full-text search memos — the context-in path: memos as agent-searchable
    /// context. This is an *additional lens*, never the knowledge backend.
    pub async fn search(&self, query: &str, limit: u32) -> anyhow::Result<Vec<MemoSummary>> {
        let query = query.to_string();
        let vault = self.vault.clone();
        Ok(tokio::task::spawn_blocking(move || vault.search_memos(&query, limit)).await??)
    }

    /// List recent memos (newest-first by `updated_at`), excluding soft-deleted.
    pub async fn list(&self, limit: u32) -> anyhow::Result<Vec<MemoSummary>> {
        let vault = self.vault.clone();
        let page = tokio::task::spawn_blocking(move || {
            vault.list_memos(None, limit, MemoFilter::default())
        })
        .await??;
        Ok(page.items)
    }

    /// Soft-delete a memo, then publish [`KernelEvent::MemoDeleted`].
    pub async fn delete_memo(&self, id: &str) -> anyhow::Result<()> {
        let mid = MemoId::parse(id).map_err(anyhow::Error::new)?;
        let vault = self.vault.clone();
        tokio::task::spawn_blocking(move || vault.delete_memo(mid)).await??;
        self.publish(KernelEvent::MemoDeleted { id: id.to_string() });
        Ok(())
    }

    /// Best-effort event publish; a closed bus is silently ignored.
    fn publish(&self, event: KernelEvent) {
        if let Some(bus) = &self.event_bus {
            let _ = bus.publish(event);
        }
    }
}

#[cfg(all(test, feature = "memo"))]
mod tests {
    use super::*;

    /// Round-trip: create → search → delete proves the facade exercises the
    /// vault's real store + tantivy index end-to-end. Uses a temp vault (the
    /// index is namespaced under `by-vault/<hash>` so it never collides with
    /// the user's default oximemo vault).
    #[tokio::test]
    async fn memo_create_search_delete_round_trip() {
        let dir = tempfile::TempDir::new().expect("temp vault dir");
        let api = MemoApi::open(Some(dir.path()), None).expect("open vault");

        let memo = api
            .create_memo("remember the milk #errands".into(), None)
            .await
            .expect("create");

        // Context-in path: a created memo is immediately full-text searchable.
        let found = api.search("milk", 10).await.expect("search");
        assert!(
            found.iter().any(|m| m.id == memo.id),
            "created memo must be searchable"
        );

        // Soft-delete drops it from search (the memo is trashed, not erased).
        api.delete_memo(&memo.id.to_string()).await.expect("delete");
        let found_after = api.search("milk", 10).await.expect("search");
        assert!(
            !found_after.iter().any(|m| m.id == memo.id),
            "soft-deleted memo must drop from search"
        );
    }
}
