# Release Profile — Configuration & Trade-offs

**Date:** 2025-05-31  
**Status:** ✅ Applied to root `Cargo.toml`

---

## Configuration

```toml
[profile.release]
lto = "thin"          # Link-Time Optimization
codegen-units = 1     # Single codegen unit
strip = true          # Remove debug symbols
panic = "abort"       # No unwinding overhead
opt-level = 3         # Maximum speed (explicit)
```

## Binary Size Comparison

| Profile | Size | Features | Notes |
|---------|------|----------|-------|
| `dev` (default) | **84 MB** | All default | Unoptimized + debug info |
| `release` (new) | **50 MB** | `web,cli,sqlite-memory` | LTO thin + stripped, CI/Docker config |
| `release` (new) | **65 MB** | All default (+browser) | Browser feature adds ~15 MB |
| **Reduction** | **40%** | | 34 MB saved (from 84→50 MB) |

Measured on macOS arm64 (Apple Silicon). Linux x86_64 binaries will differ slightly.

## Trade-off Analysis

### `lto = "thin"`

| Aspect | Detail |
|--------|--------|
| **What** | Performs cross-crate optimization at link time |
| **Benefit** | ~15-25% binary size reduction; marginal runtime speedup from inlining across crate boundaries |
| **Cost** | Increases link time by ~30-60s on a typical machine |
| **Alternative** | `lto = "fat"` — slightly smaller but 2-5× link time; `lto = false` — no benefit |
| **Verdict** | Best balance. `fat` LTO adds minutes to CI for <5% additional savings |

### `codegen-units = 1`

| Aspect | Detail |
|--------|--------|
| **What** | Compiles the entire crate as a single translation unit |
| **Benefit** | Allows LLVM to optimize across more code boundaries; better inlining decisions |
| **Cost** | Slower compile time (no parallelism within a crate) |
| **Alternative** | Default = 16 (parallel compilation, less optimization) |
| **Verdict** | Worth it for release. Compile time increases are one-time; runtime benefits are permanent |

### `strip = true`

| Aspect | Detail |
|--------|--------|
| **What** | Removes debug symbols from the final binary |
| **Benefit** | ~30-50% binary size reduction |
| **Cost** | No stack traces with line numbers in crash reports |
| **Mitigation** | Keep debug symbols separately using `cargo build --release` then `objcopy --only-keep-debug` before strip |
| **Verdict** | Essential for production. Use separate debug symbol files for crash diagnostics |

### `panic = "abort"`

| Aspect | Detail |
|--------|--------|
| **What** | Terminates the process immediately on panic instead of unwinding the stack |
| **Benefit** | Smaller binary (no unwinding tables); faster panic path |
| **Cost** | `catch_unwind` / `std::panic::catch_unwind` becomes unusable; destructors of panicking thread's stack are not run |
| **Verification** | `grep -r "catch_unwind" --include="*.rs" .` → **zero matches** ✅ |
| **Impact on codebase** | Circuit Breaker uses `Result<T,E>` (not panic catching) — unaffected. Graceful shutdown uses `tokio::signal` — unaffected. |
| **Verdict** | Safe to use. No code in the codebase relies on panic unwinding |

### `opt-level = 3`

| Aspect | Detail |
|--------|--------|
| **What** | Maximum LLVM optimization level (default for release profile, made explicit) |
| **Benefit** | Fastest runtime performance |
| **Cost** | None beyond default release behavior |
| **Verdict** | Explicit for documentation clarity |

## CI Impact

The `release-check` job in CI already builds with `--release`. The new profile will automatically apply.

**Expected CI time increase:** ~1-2 minutes for the `release-check` job due to LTO and single codegen unit. This is acceptable for a job that runs on merge to main.

## Bug Fix Applied

While verifying the release profile, a pre-existing compile error in `src/otel.rs` was discovered and fixed:

- `OtelGuard` struct used `Self` constructor syntax (unit-struct style) but was defined with braces
- Fixed by adding a `_phantom: ()` field for the non-`otel` cfg variant and using `OtelGuard::default()` at call sites
- This was a latent bug that only manifested when compiling without the (unimplemented) `otel` feature

## Recommendations

1. **Separate debug symbols**: In the release workflow, add a step to extract debug symbols before stripping:
   ```bash
   objcopy --only-keep-debug target/release/oxios oxios.debug
   objcopy --strip-debug --strip-unneeded target/release/oxios
   objcopy --add-gnu-debuglink=oxios.debug target/release/oxios
   ```
   Upload `oxios.debug` alongside the release binary.

2. **Consider `lto = "fat"` for tagged releases only**: The release workflow could use a `[profile.release-lto]` profile with `lto = "fat"` for maximum optimization on tagged builds, while CI uses `lto = "thin"` for faster iteration.

3. **Monitor binary size over time**: Add a CI check that fails if the release binary exceeds a threshold (e.g., 50 MB).
