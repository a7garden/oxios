//! Oxios Benchmark Binary
//!
//! Entry point for the benchmark CLI.

use std::process;

#[tokio::main]
async fn main() {
    if let Err(e) = oxios_bench::cli::run().await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}