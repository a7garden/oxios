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
}

#[cfg(feature = "wasm-sandbox")]
impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 50 * 1024 * 1024,
            max_instructions: 10_000_000,
            max_module_size_bytes: 10 * 1024 * 1024,
        }
    }
}

/// Host state combining WASI preview1 context.
/// WASI types live in `wasmtime-wasi`; the `Linker`'s `T` must implement
/// a closure-based accessor rather than holding `WasiCtx` directly.
#[cfg(feature = "wasm-sandbox")]
struct WasiHostState {
    wasi: wasmtime_wasi::preview1::WasiP1Ctx,
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
    pub fn load_module_from_file(&self, name: &str, path: &Path) -> Result<(), WasmError> {
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

        // Create a new store with WASI state and fuel for instruction limiting.
        // store T is our WasiHostState (holds WasiP1Ctx).
        let wasi = wasmtime_wasi::WasiCtxBuilder::new().build_p1();
        let mut store = wasmtime::Store::new(&self.engine, WasiHostState { wasi });

        // Set fuel limit (convert instructions to fuel units, 1 fuel = 1 instruction)
        store
            .set_fuel(self.config.max_instructions)
            .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

        // Instantiate the module (synchronous API).
        let instance = self
            .linker
            .instantiate(&mut store, &module)
            .map_err(|e| WasmError::InstantiationFailed(e.to_string()))?;

        // Get the function
        let func = instance
            .get_typed_func::<(i32, i32), (i32, i32)>(&mut store, func_name)
            .map_err(|_| {
                WasmError::FunctionNotFound(module_name.to_string(), func_name.to_string())
            })?;

        // Serialize input to JSON bytes
        let input_bytes = serde_json::to_vec(&input_json)
            .map_err(|e| WasmError::ExecutionFailed(format!("Failed to serialize input: {}", e)))?;

        // Write input to memory (simplified - assumes module exports memory)
        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
            WasmError::ExecutionFailed("Module does not export 'memory'".to_string())
        })?;

        let input_ptr = 0i32;
        memory
            .write(&mut store, input_ptr as usize, &input_bytes)
            .map_err(|e| {
                WasmError::ExecutionFailed(format!("Failed to write input to memory: {}", e))
            })?;

        // Execute the function (synchronous API).
        let result = func
            .call(&mut store, (input_ptr, input_bytes.len() as i32))
            .map_err(|e| {
                // Check for fuel exhaustion (get_fuel returns Result).
                let fuel_err = store
                    .get_fuel()
                    .map(|remaining| remaining == 0)
                    .unwrap_or(false);

                if fuel_err {
                    WasmError::OutOfResources {
                        kind: ResourceKind::Instructions,
                        limit: self.config.max_instructions,
                    }
                } else {
                    WasmError::ExecutionFailed(e.to_string())
                }
            })?;

        // Read output from memory.
        // `Memory::read` fills a caller-provided buffer instead of returning a
        // new Vec.
        let output_len = result.1 as usize;
        let mut output_bytes = vec![0u8; output_len];
        memory
            .read(&store, result.0 as usize, &mut output_bytes)
            .map_err(|e| {
                WasmError::ExecutionFailed(format!("Failed to read output from memory: {}", e))
            })?;

        // Deserialize output
        let output: serde_json::Value = serde_json::from_slice(&output_bytes).map_err(|e| {
            WasmError::ExecutionFailed(format!("Failed to deserialize output: {}", e))
        })?;

        Ok(output)
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
