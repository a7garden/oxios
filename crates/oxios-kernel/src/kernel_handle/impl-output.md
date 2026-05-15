# KernelHandle Extension — Implementation Output

## Summary

Added 3 new domain API facades to `KernelHandle`, expanding it from 8 to 11 facades.

## New Files Created

### 1. `crates/oxios-kernel/src/kernel_handle/exec_api.rs`
- **ExecApi** — wraps `Arc<ExecConfig>` + `Arc<parking_lot::Mutex<AccessManager>>`
- Methods: `new()`, `config()`, `access_manager()`

### 2. `crates/oxios-kernel/src/kernel_handle/browser_api.rs`
- **BrowserApi** — conditionally compiled:
  - `#[cfg(feature = "browser")]`: wraps `Arc<OxibrowserBackend>`, methods: `new()`, `backend()`
  - `#[cfg(not(feature = "browser"))]`: zero-sized unit struct
  - Both variants implement `Default` for the deprecated `from_subsystems()` path

### 3. `crates/oxios-kernel/src/kernel_handle/a2a_api.rs`
- **A2aApi** — wraps `Arc<A2AProtocol>`
- Methods: `new()`, `protocol()`

## Modified Files

### `crates/oxios-kernel/src/kernel_handle/mod.rs`
- Added `pub mod exec_api`, `pub mod browser_api`, `pub mod a2a_api`
- Added `pub use` re-exports for all 3 new types
- Added imports for `ExecConfig` and `A2AProtocol`
- Updated `KernelHandle` struct: 3 new fields (`exec`, `browser`, `a2a`)
- Updated `KernelHandle::new()`: now accepts 11 facade arguments
- Updated deprecated `from_subsystems()`: provides stubs for new fields (uses `config.exec.clone()` for ExecApi, `BrowserApi::default()`, new `A2AProtocol` instance)
- Doc updated from "7 domain Facades" to "10 domain Facades"

### `src/kernel.rs`
- Added `a2a_protocol: Arc<A2AProtocol>` field to `Kernel` struct
- Updated `Kernel::handle()`: passes `ExecApi`, `BrowserApi` (via `build_browser_api()`), and `A2aApi` to `KernelHandle::new()`
- Added `build_browser_api()` method with cfg-gated implementations:
  - `#[cfg(feature = "browser")]`: creates real `OxibrowserBackend` if config enables it
  - `#[cfg(not(feature = "browser"))]`: returns `BrowserApi::default()` (zero-sized)
- Stored `a2a_protocol` in `Kernel` return struct (was previously only local in `build()`)

## Compilation

All new/modified files compile cleanly. Pre-existing errors in `oxi-agent` (path dependency) are unrelated:
- `tool_result_message` move errors
- `max_iterations` type mismatch (u32 vs usize)
- `dir_path` borrow errors
- `read_text` PathBuf vs &Path mismatch

No errors originate from `oxios-kernel` or `oxios` crates.
