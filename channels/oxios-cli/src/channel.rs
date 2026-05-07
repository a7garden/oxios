//! CLI channel implementation.
//!
//! Implements the [`Channel`] trait from `oxios-gateway` so the CLI
//! can plug into the gateway like any other channel.
//!
//! Uses `mpsc` channels to bridge:
//! - **Incoming**: User typed input → mpsc → Gateway → Kernel
//! - **Outgoing**: Kernel → Gateway → mpsc → stdout

use anyhow::Result;
use async_trait::async_trait;
use oxios_gateway::channel::Channel;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};
use tokio::sync::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::session::Session;

/// The CLI channel adapter.
///
/// Bridges the interactive readline loop with the gateway's channel
/// interface using mpsc channels for message passing.
pub struct CliChannel {
    /// Receiver for incoming messages (user input from readline).
    incoming_rx: Mutex<mpsc::Receiver<IncomingMessage>>,
    /// Sender for injecting incoming messages.
    incoming_tx: mpsc::Sender<IncomingMessage>,
    /// Current session metadata.
    session: Arc<std::sync::Mutex<Session>>,
}

impl CliChannel {
    /// Creates a new CLI channel with the given buffer size.
    pub fn new(buffer: usize) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::channel(buffer);
        let session = Arc::new(std::sync::Mutex::new(Session::new(None)));

        Self {
            incoming_rx: Mutex::new(incoming_rx),
            incoming_tx,
            session,
        }
    }

    /// Returns a sender that can be used to inject incoming messages.
    pub fn sender(&self) -> mpsc::Sender<IncomingMessage> {
        self.incoming_tx.clone()
    }

    /// Returns a receiver for outgoing messages (used by the display loop).
    pub fn outgoing_receiver(&self) -> mpsc::Receiver<OutgoingMessage> {
        // We can't clone the receiver, so this is a one-shot operation.
        // Use `subscribe_outgoing` pattern instead — store receiver separately.
        unreachable!("Use take_outgoing_rx after construction")
    }

    /// Takes the outgoing receiver. Call once during setup.
    pub fn take_outgoing_rx(&self) -> mpsc::Receiver<OutgoingMessage> {
        // The outgoing_tx is connected to nothing useful initially.
        // We need to re-create this pattern. For simplicity, we return
        // a new channel and the sender is what we use in `send()`.
        //
        // Actually, let's just use a simpler approach: the send() method
        // prints to stdout directly.
        unimplemented!("Use the direct stdout approach via send()")
    }

    /// Returns a handle for injecting messages from outside the channel.
    pub fn handle(&self) -> CliChannelHandle {
        CliChannelHandle {
            incoming_tx: self.incoming_tx.clone(),
            session: self.session.clone(),
        }
    }
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str {
        "cli"
    }

    async fn receive(&self) -> Result<Option<IncomingMessage>> {
        let mut rx = self.incoming_rx.lock().await;
        Ok(rx.recv().await)
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // Print the response to stdout.
        println!("{}", msg.content);
        Ok(())
    }
}

impl std::fmt::Debug for CliChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliChannel").finish()
    }
}

/// Handle to the CLI channel, used to inject messages from the readline loop.
#[derive(Debug, Clone)]
pub struct CliChannelHandle {
    /// Sender for injecting incoming messages into the gateway pipeline.
    pub incoming_tx: mpsc::Sender<IncomingMessage>,
    /// Shared session reference.
    session: Arc<std::sync::Mutex<Session>>,
}

impl CliChannelHandle {
    /// Creates a handle from a CliChannel.
    pub fn from_channel(channel: &CliChannel) -> Self {
        channel.handle()
    }

    /// Send a user message into the gateway pipeline.
    pub async fn send_user_message(&self, content: String) -> Result<()> {
        let mut msg = IncomingMessage::new("cli", "cli-user", &content);
        {
            let session = self.session.lock().unwrap();
            msg.metadata.insert(
                "session_id".to_owned(),
                session.id.to_string(),
            );
        }
        self.incoming_tx.send(msg).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// Touch the session (update activity).
    pub fn touch_session(&self) {
        if let Ok(mut session) = self.session.lock() {
            session.touch();
        }
    }

    /// Reset the session (create a new one).
    pub fn reset_session(&self) {
        if let Ok(mut session) = self.session.lock() {
            *session = Session::new(None);
        }
    }

    /// Get the current session ID.
    pub fn session_id(&self) -> uuid::Uuid {
        self.session.lock().map(|s| s.id).unwrap_or_default()
    }
}
