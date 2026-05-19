//! Wasmtime engine wrapper with deterministic execution policies.
//!
//! [`WasmEngine`] is the cross-store factory: it owns the wasmtime [`Engine`]
//! configured for AOT compilation, epoch interruption, and fuel metering. The
//! same engine is shared across every plugin store so module compilation is
//! amortised.

use std::fmt;
use std::sync::Arc;

use wasmtime::{Config, Engine, Module, Strategy};

/// Errors emitted while configuring or compiling on the plugin engine.
#[derive(Debug)]
pub enum WasmEngineError {
    Config(String),
    Compile(String),
}

impl fmt::Display for WasmEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WasmEngineError::Config(msg) => write!(f, "wasm engine config error: {msg}"),
            WasmEngineError::Compile(msg) => write!(f, "wasm module compile error: {msg}"),
        }
    }
}

impl std::error::Error for WasmEngineError {}

impl From<WasmEngineError> for astrbot_core::AstrbotError {
    fn from(value: WasmEngineError) -> Self {
        astrbot_core::AstrbotError::Pipeline(value.to_string())
    }
}

/// Static engine policies. These apply to every store spawned from the engine.
///
/// Per-store knobs (initial fuel, epoch deadline value) live on
/// [`crate::wasm::WasmStoreContext`] because they are mutable at call time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasmEngineConfig {
    /// Enable epoch-based interruption. The host can call
    /// [`WasmEngine::increment_epoch`] (typically from a watchdog thread) to
    /// force the guest to trap at the next safepoint.
    pub epoch_interruption: bool,
    /// Enable fuel metering. Stores must call `set_fuel` before invoking guest
    /// code; running out of fuel triggers a trap.
    pub consume_fuel: bool,
    /// Allow async wasm calls. Off by default to keep the unit test surface
    /// synchronous; the loader will flip this on once we wire Tokio.
    pub async_support: bool,
    /// Compilation strategy. Cranelift is the only supported option on stable
    /// wasmtime today.
    pub strategy: Strategy,
    /// Whether to allow the guest to use the Wasm component model. The plugin
    /// SDK will eventually depend on this; we keep it on by default.
    pub wasm_component_model: bool,
}

impl Default for WasmEngineConfig {
    fn default() -> Self {
        Self {
            epoch_interruption: true,
            consume_fuel: true,
            async_support: false,
            strategy: Strategy::Cranelift,
            wasm_component_model: true,
        }
    }
}

/// Shared wasmtime engine instance.
#[derive(Clone)]
pub struct WasmEngine {
    inner: Arc<Engine>,
    config: WasmEngineConfig,
}

impl WasmEngine {
    /// Build an engine from the given policy.
    pub fn new(config: WasmEngineConfig) -> Result<Self, WasmEngineError> {
        let mut cfg = Config::new();
        cfg.strategy(config.strategy);
        cfg.epoch_interruption(config.epoch_interruption);
        cfg.consume_fuel(config.consume_fuel);
        // `async_support` was deprecated in wasmtime 44 — async wrappers are
        // available regardless. We keep the toggle in our config so callers can
        // signal intent, but no longer pass it down to wasmtime.
        cfg.wasm_component_model(config.wasm_component_model);
        // Multi-value and bulk-memory have been stable for years; turn on for
        // ergonomic codegen on the SDK side. Reference types are always on in
        // recent wasmtime releases.
        cfg.wasm_multi_value(true);
        cfg.wasm_bulk_memory(true);

        let engine = Engine::new(&cfg).map_err(|err| WasmEngineError::Config(err.to_string()))?;
        Ok(Self {
            inner: Arc::new(engine),
            config,
        })
    }

    /// Construct an engine using [`WasmEngineConfig::default`].
    pub fn with_defaults() -> Result<Self, WasmEngineError> {
        Self::new(WasmEngineConfig::default())
    }

    /// Borrow the underlying engine. Stores are created from this handle.
    pub fn engine(&self) -> &Engine {
        &self.inner
    }

    /// The policy used to build this engine.
    pub fn config(&self) -> &WasmEngineConfig {
        &self.config
    }

    /// Compile a module from raw bytes (`.wasm`).
    pub fn compile_module(&self, bytes: &[u8]) -> Result<Module, WasmEngineError> {
        Module::from_binary(&self.inner, bytes)
            .map_err(|err| WasmEngineError::Compile(err.to_string()))
    }

    /// Compile a module from text (`.wat`). Test-only path; production loads
    /// signed `.wasm` artefacts.
    pub fn compile_wat(&self, source: &str) -> Result<Module, WasmEngineError> {
        Module::new(&self.inner, source).map_err(|err| WasmEngineError::Compile(err.to_string()))
    }

    /// Move the engine clock forward by one tick. Any store whose deadline has
    /// elapsed will trap on its next safepoint.
    pub fn increment_epoch(&self) {
        self.inner.increment_epoch();
    }
}

impl fmt::Debug for WasmEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmEngine")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::{PluginResourceLimits, WasmStoreContext, new_plugin_store};
    use std::thread;
    use std::time::Duration;
    use wasmtime::Trap;

    fn engine() -> WasmEngine {
        WasmEngine::with_defaults().expect("engine builds")
    }

    #[test]
    fn engine_builds_with_defaults() {
        let engine = engine();
        assert!(engine.config().epoch_interruption);
        assert!(engine.config().consume_fuel);
    }

    #[test]
    fn fuel_exhaustion_traps_infinite_loop() {
        let engine = engine();
        let module = engine
            .compile_wat(
                r#"(module
                    (func $spin (loop $forever br $forever))
                    (export "spin" (func $spin)))"#,
            )
            .expect("module compiles");

        let ctx = WasmStoreContext::new("test.fuel", PluginResourceLimits::default());
        let mut store = new_plugin_store(&engine, ctx, /* fuel */ 10_000, /* epoch */ None);

        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let spin = instance
            .get_typed_func::<(), ()>(&mut store, "spin")
            .expect("export");
        let err = spin.call(&mut store, ()).expect_err("must trap");
        let trap = err.downcast_ref::<Trap>().copied();
        assert_eq!(
            trap,
            Some(Trap::OutOfFuel),
            "expected OutOfFuel trap, got {err:?}"
        );
    }

    #[test]
    fn epoch_interruption_halts_infinite_loop() {
        let engine = engine();
        let module = engine
            .compile_wat(
                r#"(module
                    (func $spin (loop $forever br $forever))
                    (export "spin" (func $spin)))"#,
            )
            .expect("module compiles");

        let ctx = WasmStoreContext::new("test.epoch", PluginResourceLimits::default());
        let mut store = new_plugin_store(
            &engine,
            ctx,
            /* fuel */ u64::MAX / 2,
            /* epoch deadline */ Some(1),
        );

        let watchdog_engine = engine.clone();
        let watchdog = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            watchdog_engine.increment_epoch();
        });

        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let spin = instance
            .get_typed_func::<(), ()>(&mut store, "spin")
            .expect("export");
        let err = spin.call(&mut store, ()).expect_err("must trap");
        let trap = err.downcast_ref::<Trap>().copied();
        assert_eq!(
            trap,
            Some(Trap::Interrupt),
            "expected Interrupt trap, got {err:?}"
        );
        watchdog.join().expect("watchdog joins");
    }

    #[test]
    fn memory_growth_denied_above_limit() {
        let engine = engine();
        // Grow memory by 100 pages (6.4 MiB) and trap unconditionally so we can
        // observe the post-growth state.
        let module = engine
            .compile_wat(
                r#"(module
                    (memory (export "mem") 1)
                    (func (export "grow") (param i32) (result i32)
                        local.get 0
                        memory.grow))"#,
            )
            .expect("module compiles");

        let limits = PluginResourceLimits::with_memory(64 * 1024); // 1 page
        let ctx = WasmStoreContext::new("test.mem", limits);
        let mut store = new_plugin_store(&engine, ctx, /* fuel */ 1_000_000, /* epoch */ None);

        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let grow = instance
            .get_typed_func::<i32, i32>(&mut store, "grow")
            .expect("export");

        // Initial memory is already 1 page (= the configured ceiling).
        // Attempting to grow by 1 more page must be rejected.
        let result = grow.call(&mut store, 1).expect("grow returns sentinel");
        assert_eq!(result, -1, "memory.grow should return -1 when denied");
    }
}
