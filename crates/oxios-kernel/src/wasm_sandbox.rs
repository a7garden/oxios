//! WebAssembly sandbox for executing untrusted tool code.
//!
//! This module provides a WASM-based sandbox using wasmtime for safely
//! executing tool code with resource limits.
//!
//! Entire module is behind the `wasm-sandbox` feature gate.

#[cfg(feature = "wasm-sandbox")]
use std::path::Path;

#[cfg(feature = "wasm-sandbox")]
use std::collections::HashMap;

#[cfg(feature = "wasm-sandbox")]
use parking_lot::RwLock;

#[cfg(feature = "wasm-sandbox")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "wasm-sandbox")]
use thiserror::Error;

/// Resource exhaustion kind for WASM execution.
#[cfg(feature = "wasm-sandbox")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    /// Memory limit exceeded.
    Memory,
    /// Instruction count limit exceeded.
    Instructions,
    /// Module size limit exceeded.
    ModuleSize,
}

/// Error types for WASM sandbox operations.
#[cfg(feature = "wasm-sandbox")]
#[derive(Debug, Clone, Error)]
pub enum WasmError {
    /// The requested module is not loaded.
    #[error("WASM module '{0}' not found")]
    ModuleNotFound(String),

    /// The requested function is not exported by the module.
    #[error("Function '{1}' not found in module '{0}'")]
    FunctionNotFound(String, String),

    /// Execution of the function failed.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Resource limit exceeded during execution.
    #[error("Out of resources: {kind:?} limit {limit} exceeded")]
    OutOfResources {
        /// The kind of resource that was exceeded.
        kind: ResourceKind,
        /// The configured limit.
        limit: u64,
    },

    /// Module instantiation failed.
    #[error("Module instantiation failed: {0}")]
    InstantiationFailed(String),

    /// Module binary exceeds size limit.
    #[error("Module too large: {size} bytes exceeds limit of {limit} bytes")]
    ModuleTooLarge { size: u64, limit: u64 },

    /// WASM sandbox feature is disabled.
    #[error("WASM sandbox feature is disabled")]
    FeatureDisabled,
}

/// Configuration for WASM sandbox limits.
#[cfg(feature = "wasm-sandbox")]
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Maximum memory in bytes (default: 50MB).
    pub max_memory_bytes: u64,
    /// Maximum instruction count (default: 10 million).
    pub max_instructions: u64,
    /// Maximum module size in bytes (default: 10MB).
    pub max_module_size_bytes: u64,
    /// Wall-clock timeout for a single tool execution (default: 30s).
    pub max_exec_seconds: u64,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 50 * 1024 * 1024,
            max_instructions: 10_000_000,
            max_module_size_bytes: 10 * 1024 * 1024,
            max_exec_seconds: 30,
        }
    }
}

/// Host-side resource limiter enforcing `WasmConfig::max_memory_bytes`.
///
/// Installed on every `Store` via `Store::limiter` so that a guest
/// `memory.grow` beyond the configured ceiling fails instead of letting the
/// module grow toward its declared maximum (up to 4GB on wasm32).
#[cfg(feature = "wasm-sandbox")]
struct StoreLimiter {
    max_memory_bytes: u64,
}

#[cfg(feature = "wasm-sandbox")]
impl wasmtime::ResourceLimiter for StoreLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> std::result::Result<bool, anyhow::Error> {
        Ok((desired as u64) <= self.max_memory_bytes)
    }
}

/// Host state combining WASI preview1 context.
/// WASI types live in `wasmtime-wasi`; the `Linker`'s `T` must implement
/// a closure-based accessor rather than holding `WasiCtx` directly.
#[cfg(feature = "wasm-sandbox")]
struct WasiHostState {
    wasi: wasmtime_wasi::preview1::WasiP1Ctx,
    limiter: StoreLimiter,
}
/// WASM sandbox for executing untrusted tool code.
///
/// Provides isolation and resource limits for WASM modules.
#[cfg(feature = "wasm-sandbox")]
pub struct WasmSandbox {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<WasiHostState>,
    config: WasmConfig,
    modules: RwLock<HashMap<String, wasmtime::Module>>,
}

#[cfg(feature = "wasm-sandbox")]
impl WasmSandbox {
    /// Create a new WASM sandbox with the given configuration.
    pub fn new(config: WasmConfig) -> Result<Self, WasmError> {
        let mut engine_config = wasmtime::Config::new();
        engine_config.consume_fuel(true);

        let engine = wasmtime::Engine::new(&engine_config)
            .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

        let mut linker = wasmtime::Linker::new(&engine);

        // WASI is registered via the preview1 compatibility layer, not the
        // old `linker.define_wasi()` method. The closure extracts the
        // `WasiP1Ctx` from our host state during calls.
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |state: &mut WasiHostState| {
            &mut state.wasi
        })
        .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

        Ok(Self {
            engine,
            linker,
            config,
            modules: RwLock::new(HashMap::new()),
        })
    }

    /// Load a WASM module from bytes.
    pub fn load_module(&self, name: &str, wasm_bytes: &[u8]) -> Result<(), WasmError> {
        // Check module size limit
        let module_size = wasm_bytes.len() as u64;
        if module_size > self.config.max_module_size_bytes {
            return Err(WasmError::ModuleTooLarge {
                size: module_size,
                limit: self.config.max_module_size_bytes,
            });
        }

        let module = wasmtime::Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

        let mut modules = self.modules.write();
        modules.insert(name.to_string(), module);

        Ok(())
    }

    /// Load a WASM module from a file.
    ///
    /// `name` must be a bare identifier (used as the module-map key, so path
    /// separators or `..` would let a caller shadow another module) and the
    /// file `path` must not contain `..` traversal components.
    pub fn load_module_from_file(&self, name: &str, path: &Path) -> Result<(), WasmError> {
        if !is_safe_module_name(name) {
            return Err(WasmError::InstantiationFailed(format!(
                "invalid module name '{name}': must be a bare identifier (no path separators)"
            )));
        }
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(WasmError::InstantiationFailed(format!(
                "module path must not contain parent-directory ('..') components: {}",
                path.display()
            )));
        }
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| WasmError::InstantiationFailed(format!("Failed to read file: {}", e)))?;

        self.load_module(name, &wasm_bytes)
    }

    /// Execute a tool function in a loaded module.
    pub async fn execute_tool(
        &self,
        module_name: &str,
        func_name: &str,
        input_json: serde_json::Value,
    ) -> Result<serde_json::Value, WasmError> {
        // Get the module
        let module = {
            let modules = self.modules.read();
            modules
                .get(module_name)
                .cloned()
                .ok_or_else(|| WasmError::ModuleNotFound(module_name.to_string()))?
        };

        // WASM execution uses wasmtime's synchronous API, which blocks the
        // calling thread for the full instruction budget. Run it on a blocking
        // pool thread and apply a wall-clock timeout so a hostile or looping
        // module can't pin a tokio worker (and starve other tasks) forever.
        let engine = self.engine.clone();
        let linker = self.linker.clone();
        let config = self.config.clone();
        let module_name = module_name.to_string();
        let func_name = func_name.to_string();
        let max_exec_seconds = config.max_exec_seconds;
        let timeout = std::time::Duration::from_secs(max_exec_seconds.max(1));

        let join = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, WasmError> {
            let wasi = wasmtime_wasi::WasiCtxBuilder::new().build_p1();
            let host_state = WasiHostState {
                wasi,
                limiter: StoreLimiter {
                    max_memory_bytes: config.max_memory_bytes,
                },
            };
            let mut store = wasmtime::Store::new(&engine, host_state);
            // Enforce max_memory_bytes on guest `memory.grow`.
            store.limiter(|state: &mut WasiHostState| &mut state.limiter);

            store
                .set_fuel(config.max_instructions)
                .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

            let func = instance
                .get_typed_func::<(i32, i32), (i32, i32)>(&mut store, &func_name)
                .map_err(|_| WasmError::FunctionNotFound(module_name.clone(), func_name.clone()))?;

            let input_bytes = serde_json::to_vec(&input_json).map_err(|e| {
                WasmError::ExecutionFailed(format!("Failed to serialize input: {}", e))
            })?;

            let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
                WasmError::ExecutionFailed("Module does not export 'memory'".to_string())
            })?;

            let input_ptr = 0i32;
            memory
                .write(&mut store, input_ptr as usize, &input_bytes)
                .map_err(|e| {
                    WasmError::ExecutionFailed(format!("Failed to write input to memory: {}", e))
                })?;

            let result = func
                .call(&mut store, (input_ptr, input_bytes.len() as i32))
                .map_err(|e| {
                    let fuel_err = store
                        .get_fuel()
                        .map(|remaining| remaining == 0)
                        .unwrap_or(false);
                    if fuel_err {
                        WasmError::OutOfResources {
                            kind: ResourceKind::Instructions,
                            limit: config.max_instructions,
                        }
                    } else {
                        WasmError::ExecutionFailed(e.to_string())
                    }
                })?;

            // The WASM module controls the returned (ptr, len); treat them
            // as hostile and validate before allocating a host buffer.
            let output_ptr = result.0 as usize;
            if result.1 < 0 {
                return Err(WasmError::ExecutionFailed(format!(
                    "WASM returned negative output length: {}",
                    result.1
                )));
            }
            let output_len = result.1 as usize;
            if output_len as u64 > config.max_memory_bytes {
                return Err(WasmError::OutOfResources {
                    kind: ResourceKind::Memory,
                    limit: config.max_memory_bytes,
                });
            }
            let mem_size = memory.data_size(&store);
            let end = output_ptr.checked_add(output_len).ok_or_else(|| {
                WasmError::ExecutionFailed("WASM output pointer+length overflow".into())
            })?;
            if output_ptr >= mem_size || end > mem_size {
                return Err(WasmError::ExecutionFailed(format!(
                    "WASM output {output_ptr}..{end} outside guest memory size {mem_size}"
                )));
            }
            let mut output_bytes = vec![0u8; output_len];
            memory
                .read(&store, output_ptr, &mut output_bytes)
                .map_err(|e| {
                    WasmError::ExecutionFailed(format!("Failed to read output from memory: {}", e))
                })?;

            let output: serde_json::Value = serde_json::from_slice(&output_bytes).map_err(|e| {
                WasmError::ExecutionFailed(format!("Failed to deserialize output: {}", e))
            })?;

            Ok(output)
        });

        match tokio::time::timeout(timeout, join).await {
            Ok(Ok(value)) => value,
            Ok(Err(join_err)) => Err(WasmError::ExecutionFailed(format!(
                "WASM execution task failed: {join_err}"
            ))),
            Err(_) => Err(WasmError::ExecutionFailed(format!(
                "WASM execution timed out after {max_exec_seconds}s"
            ))),
        }
    }

    /// List all loaded module names.
    pub fn list_modules(&self) -> Vec<String> {
        let modules = self.modules.read();
        modules.keys().cloned().collect()
    }

    /// Unload a module by name.
    /// Returns true if the module was found and removed.
    pub fn unload_module(&self, name: &str) -> bool {
        let mut modules = self.modules.write();
        modules.remove(name).is_some()
    }
}

/// Check that a module name is a bare identifier (no path separators or `..`).
///
/// The name is used as the key in the module map, so allowing `/`, `\`, or
/// `..` would let a caller shadow or collide with another loaded module.
#[cfg(feature = "wasm-sandbox")]
fn is_safe_module_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.contains('\0')
}

#[cfg(feature = "wasm-sandbox")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.max_memory_bytes, 50 * 1024 * 1024);
        assert_eq!(config.max_instructions, 10_000_000);
        assert_eq!(config.max_module_size_bytes, 10 * 1024 * 1024);
    }

    #[test]
    fn test_wasm_error_display() {
        let err = WasmError::ModuleNotFound("test".to_string());
        assert_eq!(format!("{}", err), "WASM module 'test' not found");

        let err = WasmError::FunctionNotFound("mod".to_string(), "func".to_string());
        assert_eq!(
            format!("{}", err),
            "Function 'func' not found in module 'mod'"
        );

        let err = WasmError::FeatureDisabled;
        assert_eq!(format!("{}", err), "WASM sandbox feature is disabled");
    }

    #[test]
    fn test_resource_kind_serde() {
        let memory = serde_json::to_string(&ResourceKind::Memory).unwrap();
        let instructions = serde_json::to_string(&ResourceKind::Instructions).unwrap();
        let module_size = serde_json::to_string(&ResourceKind::ModuleSize).unwrap();

        assert_eq!(memory, "\"Memory\"");
        assert_eq!(instructions, "\"Instructions\"");
        assert_eq!(module_size, "\"ModuleSize\"");
    }
}
