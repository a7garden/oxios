//! Subcommand implementations for the `oxios` binary.
//!
//! Each subcommand is a self-contained module. New subcommands add a new
//! `pub mod foo;` here and wire up `Cli::command()` in `main.rs`.

pub mod run;
pub mod update;
