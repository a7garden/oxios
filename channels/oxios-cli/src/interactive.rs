//! Interactive readline loop using reedline.
//!
//! Runs the main REPL: read user input, dispatch meta-commands,
//! forward messages to the channel, and display responses.
//!
//! Implements the "sequential input" model from RFC-014 Phase 2:
//! while a request is being processed, new input is rejected with a
//! message instead of being queued. This prevents the fire-and-forget
//! confusion where responses arrive mid-typing.

use anyhow::Result;
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};
use std::sync::Arc;

use crate::channel::CliChannelHandle;
use crate::commands::MetaCommand;

/// The interactive read-eval-print loop.
pub struct InteractiveLoop {
    /// Handle to inject messages into the gateway.
    handle: CliChannelHandle,
    /// The reedline line editor.
    editor: Reedline,
    /// The prompt to display.
    prompt: DefaultPrompt,
    /// Optional kernel handle for Space management.
    kernel: Option<Arc<oxios_kernel::KernelHandle>>,
}

impl InteractiveLoop {
    /// Create a new interactive loop.
    pub fn new(handle: CliChannelHandle) -> Self {
        Self::with_kernel(handle, None)
    }

    /// Create with an optional kernel handle for Space management.
    pub fn with_kernel(handle: CliChannelHandle, kernel: Option<Arc<oxios_kernel::KernelHandle>>) -> Self {
        let editor = Reedline::create();
        let prompt = DefaultPrompt::default();

        Self {
            handle,
            editor,
            prompt,
            kernel,
        }
    }

    /// Create with a custom prompt label.
    pub fn with_prompt_label(handle: CliChannelHandle, left: &str) -> Self {
        Self::with_prompt_label_and_kernel(handle, left, None)
    }

    /// Create with a custom prompt label and optional kernel handle.
    pub fn with_prompt_label_and_kernel(
        handle: CliChannelHandle,
        left: &str,
        kernel: Option<Arc<oxios_kernel::KernelHandle>>,
    ) -> Self {
        let editor = Reedline::create();
        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic(left.to_string()),
            DefaultPromptSegment::Empty,
        );

        Self {
            handle,
            editor,
            prompt,
            kernel,
        }
    }

    /// Run the interactive loop until `.quit` or EOF.
    ///
    /// This is a blocking call. For use inside `tokio::task::spawn_blocking`
    /// or a dedicated thread.
    pub async fn run(&mut self) -> Result<()> {
        println!("Oxios CLI — type .help for commands\n");

        loop {
            let signal = self.editor.read_line(&self.prompt);

            match signal {
                Ok(Signal::Success(line)) => {
                    let trimmed = line.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Check for meta-commands.
                    if let Some(cmd) = MetaCommand::parse(&trimmed) {
                        if self.handle_meta(cmd).await? {
                            break; // .quit
                        }
                        continue;
                    }

                    // Reject input while a previous request is still in-flight.
                    if self.handle.is_processing() {
                        println!("⏳ 이전 요청을 처리 중입니다. 잠시만 기다려주세요.");
                        continue;
                    }

                    // Mark as processing, then forward to the gateway.
                    self.handle.set_processing(true);
                    self.handle.send_user_message(trimmed).await?;
                    self.handle.touch_session();

                    // NOTE: The response will arrive asynchronously via the
                    // Channel::send() implementation (printed to stdout).
                    // In a future iteration, we could wait for a response here
                    // for a synchronous feel, but for now the gateway routes
                    // the response back through the channel.
                }
                Ok(Signal::CtrlC) => {
                    println!("\n(Ctrl+C again to quit, or type .quit)");
                }
                Ok(Signal::CtrlD) => {
                    println!("\nGoodbye!");
                    break;
                }
                Err(err) => {
                    tracing::error!("Readline error: {err}");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a meta-command. Returns `true` if we should quit.
    async fn handle_meta(&self, cmd: MetaCommand) -> Result<bool> {
        match cmd {
            MetaCommand::Quit => {
                println!("Goodbye!");
                Ok(true)
            }
            MetaCommand::Help => {
                print!("{}", MetaCommand::help_text());
                Ok(false)
            }
            MetaCommand::Reset => {
                self.handle.reset_session();
                println!("Session reset.");
                Ok(false)
            }
            MetaCommand::Model(Some(name)) => {
                println!("Switching model to: {name}");
                // TODO: wire to kernel model switching
                Ok(false)
            }
            MetaCommand::Model(None) => {
                println!("Current model: (default)");
                Ok(false)
            }
            MetaCommand::Persona(Some(name)) => {
                println!("Switching persona to: {name}");
                // TODO: wire to kernel persona switching
                Ok(false)
            }
            MetaCommand::Persona(None) => {
                println!("Current persona: (default)");
                Ok(false)
            }
            MetaCommand::Space(None) => {
                if let Some(ref kernel) = self.kernel {
                    match kernel.spaces.current_space() {
                        Some(space) => println!(
                            "📋 현재 Space: {} ({}) — {}",
                            space.name,
                            &space.id[..8],
                            if space.active { "활성" } else { "비활성" }
                        ),
                        None => println!("📋 현재 Space: (기본)"),
                    }
                } else {
                    println!("Space 관리를 사용할 수 없습니다.");
                }
                Ok(false)
            }
            MetaCommand::Space(Some(id_or_name)) => {
                if let Some(ref kernel) = self.kernel {
                    // Try as UUID first, then search by name
                    let resolved_id = if uuid::Uuid::parse_str(&id_or_name).is_ok() {
                        id_or_name.clone()
                    } else {
                        let spaces = kernel.spaces.list_spaces();
                        spaces
                            .iter()
                            .find(|s| s.name == id_or_name)
                            .map(|s| s.id.clone())
                            .unwrap_or_else(|| id_or_name.clone())
                    };
                    match kernel.spaces.activate(&resolved_id).await {
                        Ok(()) => {
                            let spaces = kernel.spaces.list_spaces();
                            if let Some(space) = spaces.iter().find(|s| s.id == resolved_id) {
                                println!("✅ Space 전환: {} ({})", space.name, &space.id[..8]);
                            } else {
                                println!("✅ Space 전환됨");
                            }
                        }
                        Err(e) => println!("❌ Space 전환 실패: {e}"),
                    }
                } else {
                    println!("Space 관리를 사용할 수 없습니다.");
                }
                Ok(false)
            }
            MetaCommand::Spaces => {
                if let Some(ref kernel) = self.kernel {
                    let spaces = kernel.spaces.list_spaces();
                    if spaces.is_empty() {
                        println!("📋 등록된 Space가 없습니다.");
                    } else {
                        println!("📋 Spaces:");
                        for space in &spaces {
                            let marker = if space.active { "→ " } else { "  " };
                            println!(
                                "{}{} ({}) — {} interactions",
                                marker, space.name, &space.id[..8], space.interaction_count
                            );
                        }
                    }
                } else {
                    println!("Space 관리를 사용할 수 없습니다.");
                }
                Ok(false)
            }
            MetaCommand::Clear => {
                // ANSI clear screen.
                print!("\x1b[2J\x1b[H");
                Ok(false)
            }
        }
    }
}
