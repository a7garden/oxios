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
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::Duration;

use crate::channel::Channel;
use crate::error_classify::classify_error;
use crate::message::{ErrorKind, IncomingMessage, OutgoingMessage, ResponseMeta, UserFacingError};
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

    /// Engine API for model switching (action-based routing).
    engine_api: Option<Arc<oxios_kernel::EngineApi>>,

    /// Persona API for persona switching (action-based routing).
    persona_api: Option<Arc<oxios_kernel::PersonaApi>>,

    /// Gateway-wide shutdown signal.
    shutdown: watch::Sender<bool>,

    /// Concurrency limiter for route tasks.
    concurrency: Arc<Semaphore>,

    /// Keywords that trigger spec (Ouroboros) mode. Prefix-only match.
    spec_keywords: Vec<String>,
}

/// Default spec keywords (used when no config is available).
fn default_spec_keywords() -> Vec<String> {
    vec!["#spec".into(), "#plan".into()]
}

/// Detect whether a message should be routed to Ouroboros (spec) mode.
/// Checks: metadata["mode"] == "spec", or content starts with a spec keyword.
fn detect_spec_mode(msg: &IncomingMessage, spec_keywords: &[String]) -> bool {
    // 1. Explicit metadata flag
    if msg.metadata.get(meta::MODE).is_some_and(|v| v == "spec") {
        return true;
    }
    // 2. Prefix keyword match
    let content = msg.content.trim();
    spec_keywords
        .iter()
        .any(|kw| content.starts_with(kw.as_str()))
}

/// Strip spec keyword prefix from content if present.
fn strip_spec_keyword<'a>(content: &'a str, spec_keywords: &[String]) -> &'a str {
    let trimmed = content.trim_start();
    for kw in spec_keywords {
        if let Some(rest) = trimmed.strip_prefix(kw.as_str()) {
            return rest.trim_start();
        }
    }
    content
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
            engine_api: None,
            persona_api: None,
            shutdown,
            concurrency: Arc::new(Semaphore::new(MAX_CONCURRENT_ROUTES)),
            spec_keywords: default_spec_keywords(),
        }
    }

    /// Creates a new gateway with engine and persona APIs for action-based routing.
    pub fn with_apis(
        orchestrator: Arc<oxios_kernel::Orchestrator>,
        engine_api: Arc<oxios_kernel::EngineApi>,
        persona_api: Arc<oxios_kernel::PersonaApi>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(GATEWAY_BUFFER);
        let (shutdown, _) = watch::channel(false);
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            rx: Mutex::new(rx),
            tx,
            orchestrator,
            engine_api: Some(engine_api),
            persona_api: Some(persona_api),
            shutdown,
            concurrency: Arc::new(Semaphore::new(MAX_CONCURRENT_ROUTES)),
            spec_keywords: default_spec_keywords(),
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
    ///
    /// If the message contains an `action` metadata key, it is routed to the
    /// appropriate API handler instead of the orchestrator.
    fn dispatch(&self, channel_name: String, msg: IncomingMessage) {
        // ── Action-based routing ───────────────────────────────────
        // Check if the message is an action command (e.g., switch_model, switch_persona)
        // rather than a regular user message.
        if let Some(action) = msg.metadata.get(meta::ACTION).cloned() {
            match action.as_str() {
                "switch_model" => {
                    self.dispatch_switch_model(channel_name, msg);
                    return;
                }
                "switch_persona" => {
                    self.dispatch_switch_persona(channel_name, msg);
                    return;
                }
                _ => {
                    tracing::warn!(action = %action, "Unknown action metadata, forwarding to orchestrator");
                    // Fall through to normal routing
                }
            }
        }

        // ── Normal orchestrator routing ────────────────────────────
        let orchestrator = self.orchestrator.clone();
        let channels = self.channels.clone();
        let semaphore = self.concurrency.clone();
        let spec_keywords = self.spec_keywords.clone();

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
            let conn_id = msg.metadata.get("conn_id").cloned();
            let request_id = msg.id.to_string();

            // ── Mode detection: spec vs chat ──
            let is_spec = detect_spec_mode(&msg, &spec_keywords);
            let effective_content = if is_spec {
                strip_spec_keyword(&msg.content, &spec_keywords).to_string()
            } else {
                msg.content.clone()
            };

            let result = if is_spec {
                orchestrator
                    .handle_message(
                        &msg.user_id,
                        &effective_content,
                        session_id.as_deref(),
                        project_ids.as_deref(),
                        &request_id,
                    )
                    .await
            } else {
                orchestrator
                    .chat(
                        &msg.user_id,
                        &msg.content,
                        session_id.as_deref(),
                        project_ids.as_deref(),
                        &request_id,
                    )
                    .await
            };

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
                    // Serialize tool_calls into metadata so web routes can read them.
                    if !orchestration.tool_calls.is_empty() {
                        if let Ok(json) = serde_json::to_string(&orchestration.tool_calls) {
                            channel_meta.insert("tool_calls".to_owned(), json);
                        }
                    }
                    // Persist execution mode in channel_meta so session persistence can read it.
                    channel_meta.insert("mode".to_owned(), orchestration.mode.clone());

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
                        // Chat UI redesign: interactive interview payload.
                        // None when the LLM did not produce structured
                        // questions — the frontend falls back to markdown.
                        interview_questions: orchestration.interview_questions,
                        interview_round: orchestration.interview_round,
                        interview_ambiguity: orchestration.interview_ambiguity,
                        mode: Some(orchestration.mode.clone()),
                    };

                    let mut outgoing = OutgoingMessage::success(
                        msg.id,
                        &msg.channel,
                        &msg.user_id,
                        &orchestration.response,
                        channel_meta,
                        response_meta,
                    );
                    outgoing.target_conn_id = conn_id;
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
                    outgoing.target_conn_id = conn_id;

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

    // ── Action dispatch handlers ───────────────────────────

    /// Handle a `switch_model` action by calling EngineApi::set_model().
    fn dispatch_switch_model(&self, channel_name: String, msg: IncomingMessage) {
        let engine_api = self.engine_api.clone();
        let channels = self.channels.clone();
        let conn_id = msg.metadata.get("conn_id").cloned();

        tokio::spawn(async move {
            let model_id = msg
                .metadata
                .get(meta::MODEL_ID)
                .cloned()
                .unwrap_or_default();

            tracing::info!(
                channel = %msg.channel,
                model_id = %model_id,
                request_id = %msg.id,
                "Routing switch_model action"
            );

            let guard = channels.read().await;
            let entry = guard.get(&channel_name);

            match (engine_api, entry) {
                (Some(api), Some(entry)) => match api.set_model(&model_id) {
                    Ok(()) => {
                        let response = format!("✅ 모델이 {model_id}(으)로 전환되었습니다.");
                        let mut outgoing = OutgoingMessage::success(
                            msg.id,
                            &msg.channel,
                            &msg.user_id,
                            &response,
                            HashMap::new(),
                            ResponseMeta {
                                session_id: None,
                                project_id: None,
                                project_tag: None,
                                seed_id: None,
                                phase: "action".to_string(),
                                evaluation_passed: Some(true),
                                duration_ms: None,
                                error: None,
                                interview_questions: None,
                                interview_round: None,
                                interview_ambiguity: None,
                                mode: None,
                            },
                        );
                        outgoing.target_conn_id = conn_id;
                        if let Err(e) = entry.channel.send(outgoing).await {
                            tracing::error!(error = %e, "Failed to send switch_model response");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "switch_model failed");
                        let user_err = UserFacingError {
                            message: format!("❌ 모델 전환 실패: {e}"),
                            kind: ErrorKind::Internal,
                            suggestion: Some(
                                "모델 ID가 올바른지 확인하세요. (예: anthropic/claude-sonnet-4)"
                                    .to_string(),
                            ),
                        };
                        let mut outgoing =
                            OutgoingMessage::error(msg.id, &msg.channel, &msg.user_id, user_err);
                        outgoing.target_conn_id = conn_id;
                        if let Err(e) = entry.channel.send(outgoing).await {
                            tracing::error!(error = %e, "Failed to send switch_model error");
                        }
                    }
                },
                (None, _) => {
                    tracing::warn!("switch_model action received but no EngineApi configured");
                }
                (_, None) => {
                    tracing::warn!(channel = %channel_name, "Channel no longer registered");
                }
            }
        });
    }

    /// Handle a `switch_persona` action by calling PersonaApi::set_active().
    fn dispatch_switch_persona(&self, channel_name: String, msg: IncomingMessage) {
        let persona_api = self.persona_api.clone();
        let channels = self.channels.clone();
        let conn_id = msg.metadata.get("conn_id").cloned();

        tokio::spawn(async move {
            let persona_id = msg
                .metadata
                .get(meta::PERSONA_ID)
                .cloned()
                .unwrap_or_default();

            tracing::info!(
                channel = %msg.channel,
                persona_id = %persona_id,
                request_id = %msg.id,
                "Routing switch_persona action"
            );

            let guard = channels.read().await;
            let entry = guard.get(&channel_name);

            match (persona_api, entry) {
                (Some(api), Some(entry)) => match api.set_active(&persona_id) {
                    Ok(()) => {
                        let response =
                            format!("✅ 페르소나가 '{persona_id}'(으)로 전환되었습니다.");
                        let mut outgoing = OutgoingMessage::success(
                            msg.id,
                            &msg.channel,
                            &msg.user_id,
                            &response,
                            HashMap::new(),
                            ResponseMeta {
                                session_id: None,
                                project_id: None,
                                project_tag: None,
                                seed_id: None,
                                phase: "action".to_string(),
                                evaluation_passed: Some(true),
                                duration_ms: None,
                                error: None,
                                interview_questions: None,
                                interview_round: None,
                                interview_ambiguity: None,
                                mode: None,
                            },
                        );
                        outgoing.target_conn_id = conn_id;
                        if let Err(e) = entry.channel.send(outgoing).await {
                            tracing::error!(error = %e, "Failed to send switch_persona response");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "switch_persona failed");
                        let user_err = UserFacingError {
                                message: format!("❌ 페르소나 전환 실패: {e}"),
                                kind: ErrorKind::Internal,
                                suggestion: Some("페르소나 ID가 올바른지 확인하세요. (.help를 입력하여 명령어를 확인하세요.)".to_string()),
                            };
                        let mut outgoing =
                            OutgoingMessage::error(msg.id, &msg.channel, &msg.user_id, user_err);
                        outgoing.target_conn_id = conn_id;
                        if let Err(e) = entry.channel.send(outgoing).await {
                            tracing::error!(error = %e, "Failed to send switch_persona error");
                        }
                    }
                },
                (None, _) => {
                    tracing::warn!("switch_persona action received but no PersonaApi configured");
                }
                (_, None) => {
                    tracing::warn!(channel = %channel_name, "Channel no longer registered");
                }
            }
        });
    }
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway").finish()
    }
}
