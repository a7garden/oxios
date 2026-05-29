//! Report generation — JSON, console, and regression comparison.

pub mod compare;
pub mod console;
pub mod json_report;

pub use compare::compare_runs;
pub use console::print_report;
pub use json_report::save_report;
