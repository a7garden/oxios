//! Gateway: routes messages between channels and the kernel.
//!
//! The gateway is channel-agnostic. It receives messages from any
//! registered channel via a shared mpsc channel, dispatches them
//! concurrently to the kernel via the orchestrator, and returns
//! responses through the appropriate channel.
//!
//! Architecture:
//! ```text
//! ┌────────────┐  ┌────────────┐  ┌──────────────┐
//! │ Web task   │  │ CLI task   │  │Telegram task │   ← independent receive
//! └─────┬──────┘  └─────┬──────┘  └──────┬───────┘
//!       └───────────────┼────────────────┘
//!                       ▼
//!              ┌────────────────┐
//!              │  Gateway rx    │  ← tokio::select!
//!              │  (mpsc 1024)   │
//!              └───────┬────────┘
//!                      │  tokio::spawn per message
//!               ┌──────┼──────┐
//!               ▼      ▼      ▼
//!            ┌─────┐┌─────┐┌─────┐
//!            │route ││route ││route │  ← Semaphore limits concurrency
//!            └──┬──┘└──┬──┘└──┬──┘
//!               └──────┼──────┘
//!                      ▼
//!               channel.send()
//! ```

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::Duration;

use crate::channel::Channel;
use crate::error_classify::classify_error;
use crate::message::{IncomingMessage, OutgoingMessage, ResponseMeta};
use crate::meta::meta;
use crate::GatewayInbox;

/// Gateway receive buffer size.
///
/// All channels push into this shared buffer. 1024 ≈ 100 concurrent sessions × 10 messages each.
const GATEWAY_BUFFER: usize = 1024;

/// Maximum concurrent orchestrations.
///
/// LLM calls are I/O-bound, so this is higher than CPU core count.
/// 32 = 4 cores × 8× headroom (other tasks run during I/O waits).
const MAX_CONCURRENT_ROUTES: usize = 32;

/// Graceful shutdown timeout per channel task.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Registered channel entry.
struct ChannelEntry {
    /// Channel trait object — for send() calls.
    channel: Arc<dyn Channel>,
    /// Per-channel shutdown signal.
    shutdown_tx: watch::Sender<bool>,
    /// Task handle from start() — for lifetime tracking.
    task: JoinHandle<()>,
}

/// Gateway: routes messages between channels and the kernel.
pub struct Gateway {
    /// Channel registry (register/unregister/send).
    channels: Arc<RwLock<HashMap<String, ChannelEntry>>>,

    /// Unified receive endpoint — all channels push here.
    rx: Mutex<mpsc::Receiver<GatewayInbox>>,

    /// Sender clone — passed to new channels during register().
    tx: mpsc::Sender<GatewayInbox>,

    /// Orchestrator reference for message dispatch.
    orchestrator: Arc<oxios_kernel::Orchestrator>,

    /// Gateway-wide shutdown signal.
    shutdown: watch::Sender<bool>,

    /// Concurrency limiter for route tasks.
    concurrency: Arc<Semaphore>,
}

impl Gateway {
    /// Creates a new gateway with the given orchestrator.
    pub fn new(orchestrator: Arc<oxios_kernel::Orchestrator>) -> Self {
        let (tx, rx) = mpsc::channel(GATEWAY_BUFFER);
        let (shutdown, _) = watch::channel(false);
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            rx: Mutex::new(rx),
            tx,
            orchestrator,
            shutdown,
            concurrency: Arc::new(Semaphore::new(MAX_CONCURRENT_ROUTES)),
        }
    }

    /// Signal the gateway to stop its event loop.
    pub fn signal_shutdown(&self) {
        let _ = self.shutdown.send(true);
        tracing::info!("Gateway shutdown signal sent");
    }

    /// Check if shutdown has been signalled.
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.borrow()
    }

    // ── Channel management ──────────────────────────────────

    /// Registers a channel with the gateway and starts its background receive task.
    pub async fn register(&self, channel: Box<dyn Channel>) -> Result<()> {
        let name = channel.name().to_owned();
        let (ch_shutdown, ch_shutdown_rx) = watch::channel(false);

        // Wrap in Arc for shared access from route tasks.
        let ch_arc: Arc<dyn Channel> = Arc::from(channel);

        // Start the channel's background receive loop.
        let task = ch_arc.start(self.tx.clone(), ch_shutdown_rx).await?;

        self.channels.write().await.insert(
            name.clone(),
            ChannelEntry {
                channel: ch_arc,
                shutdown_tx: ch_shutdown,
                task,
            },
        );

        tracing::info!(channel = %name, "Channel registered and started");
        Ok(())
    }

    /// Unregisters a channel, signalling it to stop and waiting for its task to finish.
    pub async fn unregister(&self, name: &str) -> Result<()> {
        let entry = self.channels.write().await.remove(name);
        if let Some(entry) = entry {
            let _ = entry.shutdown_tx.send(true);
            let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, entry.task).await;
            tracing::info!(channel = %name, "Channel unregistered");
        }
        Ok(())
    }

    /// Returns the names of all registered channels.
    pub async fn channel_names(&self) -> Vec<String> {
        self.channels.read().await.keys().cloned().collect()
    }

    // ── Event loop ──────────────────────────────────────────

    /// Runs the gateway event loop.
    ///
    /// Receives from the shared mpsc channel (any channel can push).
    /// Each message is dispatched to an independent task for concurrent processing.
    /// Returns when all senders are dropped or shutdown is signalled.
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Gateway event loop started");
        let mut rx = self.rx.lock().await;
        let mut shutdown = self.shutdown.subscribe();

        loop {
            tokio::select! {
                inbox = rx.recv() => {
                    match inbox {
                        Some((channel_name, msg)) => {
                            self.dispatch(channel_name, msg);
                        }
                        None => {
                            tracing::info!("All channels disconnected, exiting");
                            break;
                        }
                    }
                }

                _ = shutdown.changed() => {
                    tracing::info!("Gateway shutting down");
                    let channels = self.channels.read().await;
                    for (name, entry) in channels.iter() {
                        let _ = entry.shutdown_tx.send(true);
                        tracing::info!(channel = %name, "Shutdown signal sent");
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// Dispatch an incoming message to an independent task.
    ///
    /// The event loop is not blocked — it can immediately receive the next message.
    /// Semaphore limits concurrency to MAX_CONCURRENT_ROUTES.
    fn dispatch(&self, channel_name: String, msg: IncomingMessage) {
        let orchestrator = self.orchestrator.clone();
        let channels = self.channels.clone();
        let semaphore = self.concurrency.clone();

        tokio::spawn(async move {
            // Concurrency limit — excess requests wait.
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    tracing::warn!("Semaphore closed, dropping message");
                    return;
                }
            };

            tracing::info!(
                channel = %msg.channel,
                user = %msg.user_id,
                content_len = msg.content.len(),
                request_id = %msg.id,
                "Routing incoming message"
            );

            // ── Duration measurement (includes semaphore wait = user-perceived latency) ──
            let start = std::time::Instant::now();

            let session_id = msg.metadata.get(meta::SESSION_ID).cloned();
            let project_ids = msg.metadata.get(meta::PROJECT_IDS).cloned();
            let request_id = msg.id.to_string();
            let result = orchestrator
                .handle_message(
                    &msg.user_id,
                    &msg.content,
                    session_id.as_deref(),
                    project_ids.as_deref(),
                    &request_id,
                )
                .await;

            let duration_ms = start.elapsed().as_millis() as u64;

            let guard = channels.read().await;
            let entry = guard.get(&channel_name);

            match (result, entry) {
                (Ok(orchestration), Some(entry)) => {
                    tracing::info!(
                        phase = %orchestration.phase_reached,
                        seed_id = ?orchestration.seed_id,
                        duration_ms = duration_ms,
                        "Orchestration complete"
                    );

                    // Channel-specific metadata (chat_id, message_id, etc.)
                    let mut channel_meta = HashMap::new();
                    if let Some(ref sid) = orchestration.session_id {
                        channel_meta.insert(meta::SESSION_ID.to_owned(), sid.clone());
                    }
                    if let Some(ref pid) = orchestration.primary_project_id {
                        channel_meta.insert(meta::PROJECT_IDS.to_owned(), pid.to_string());
                    }

                    // Typed orchestration metadata (RFC-014)
                    let response_meta = ResponseMeta {
                        session_id: orchestration.session_id,
                        project_id: orchestration.primary_project_id.map(|u| u.to_string()),
                        project_tag: orchestration.project_tag,
                        seed_id: orchestration.seed_id.map(|u| u.to_string()),
                        phase: orchestration.phase_reached.to_string(),
                        evaluation_passed: orchestration.evaluation_passed,
                        duration_ms: Some(duration_ms),
                        error: None,
                    };

                    let outgoing = OutgoingMessage::success(
                        msg.id,
                        &msg.channel,
                        &msg.user_id,
                        &orchestration.response,
                        channel_meta,
                        response_meta,
                    );
                    if let Err(e) = entry.channel.send(outgoing).await {
                        tracing::error!(error = %e, "Failed to send response");
                    }
                }
                (Err(e), Some(entry)) => {
                    tracing::error!(error = %e, "Orchestration failed");
                    let user_err = classify_error(&e);

                    // Preserve session_id in error response for conversation continuity
                    let mut outgoing =
                        OutgoingMessage::error(msg.id, &msg.channel, &msg.user_id, user_err);
                    if let Some(sid) = msg.metadata.get(meta::SESSION_ID).cloned() {
                        outgoing.metadata.insert(meta::SESSION_ID.to_string(), sid);
                    }

                    if let Err(e) = entry.channel.send(outgoing).await {
                        tracing::error!(error = %e, "Failed to send error response");
                    }
                }
                (_, None) => {
                    tracing::warn!(channel = %channel_name, "Channel no longer registered");
                }
            }
        });
    }

    // ── Public utilities ────────────────────────────────────

    /// Sends a message through the named channel.
    pub async fn send_to(&self, channel_name: &str, msg: OutgoingMessage) -> Result<()> {
        let channels = self.channels.read().await;
        if let Some(entry) = channels.get(channel_name) {
            entry.channel.send(msg).await?;
        } else {
            tracing::warn!(channel = %channel_name, "No such channel registered");
        }
        Ok(())
    }
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway").finish()
    }
}
