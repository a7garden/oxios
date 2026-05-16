//! Headless browser integration for Oxios agents.
//!
//! Single gateway: `BrowserTool` wraps `oxibrowser_core::Browser` behind
//! the `AgentTool` interface. Every browser operation — one-shot reads,
//! interactive sessions, and YAML script execution — goes through this
//! one tool.
//!
//! ## Architecture
//!
//! ```text
//! Agent → BrowserTool (AgentTool) → oxibrowser_core::Browser / Tab
//!                                     └── ScriptRunner (run_script action)
//! ```
//!
//! No external process is needed — OxiBrowser runs entirely in-process.
//! The `oxibrowser` CLI binary is a developer debugging tool only;
//! agents never invoke it via ExecTool.
//!
//! ## Feature Gate
//!
//! This module is only available with the `browser` feature:
//! ```toml
//! oxios-kernel = { features = ["browser"] }
//! ```

mod browser_tool;

pub use browser_tool::BrowserTool;
