# Progress

## Status
Completed

## Tasks
- [x] Add OTel feature to oxios-kernel Cargo.toml
- [x] Create telemetry module (feature-gated, compiles with and without otel)
- [x] Add trace spans to orchestrator.rs phases
- [x] Verify: cargo check (without otel) ✅
- [x] Verify: cargo check --features otel ✅
- [x] Verify: cargo test -p oxios-kernel ✅ (246/246 tests pass)
- [x] Verify: full project cargo check ✅

## Files Changed
- `crates/oxios-kernel/Cargo.toml` — added `[features]` section with `otel` flag + optional OTel deps + tracing-subscriber workspace dep
- `crates/oxios-kernel/src/lib.rs` — registered telemetry module (both cfg variants)
- `crates/oxios-kernel/src/telemetry_otel.rs` — OTel-enabled telemetry module (real layer init stub)
- `crates/oxios-kernel/src/telemetry_stub.rs` — No-op telemetry module (when otel feature is off)
- `crates/oxios-kernel/src/orchestrator.rs` — added structured trace logging to all phases

## Notes
- Used `tracing::info!` with structured fields instead of `#[instrument]` / `info_span!().entered()` because `EnteredSpan` is `!Send`, which breaks `tokio::spawn` in test code (the orchestrator's `handle_message` is spawned in integration/e2e tests)
- The `TelemetryConfig` and `init_telemetry_layers()` are available as `oxios_kernel::telemetry::*` regardless of whether the `otel` feature is enabled
- The OTel dependencies successfully resolve and compile: tracing-opentelemetry 0.28, opentelemetry 0.27, opentelemetry_sdk 0.27, opentelemetry-otlp 0.27, opentelemetry-stdout 0.27
