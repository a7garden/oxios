//! Headless browser integration for Oxios agents.
//!
//! Uses the `oxibrowser-core` SDK directly — `Browser::browse()` for one-shot
//! reads and `Browser::new_tab()` for interactive sessions.
//!
//! ## Architecture
//!
//! ```text
//! Agent → BrowserTool (AgentTool) → oxibrowser_core::Browser / Tab
//! ```
//!
//! No external process is needed — OxiBrowser runs entirely in-process.
//!
//! ## Feature Gate
//!
//! This module is only available with the `browser` feature:
//! ```toml
//! oxios-kernel = { features = ["browser"] }
//! ```

mod browser_tool;

pub use browser_tool::BrowserTool;
