//! Per-plugin store and host context.
//!
//! Each plugin instance gets its own [`Store<WasmStoreContext>`] that:
//! - carries the plugin id for diagnostics,
//! - owns a [`PluginResourceLimiter`] which the wasm runtime queries on every
//!   memory/table growth,
//! - exposes the configured fuel and epoch deadline.

use wasmtime::Store;

use crate::wasm::engine::WasmEngine;
use crate::wasm::resource_limits::{PluginResourceLimiter, PluginResourceLimits};

/// Host-side state attached to a plugin's wasmtime store.
pub struct WasmStoreContext {
    plugin_id: String,
    limiter: PluginResourceLimiter,
}

impl WasmStoreContext {
    pub fn new(plugin_id: impl Into<String>, limits: PluginResourceLimits) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            limiter: PluginResourceLimiter::new(limits),
        }
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn limits(&self) -> &PluginResourceLimits {
        self.limiter.limits()
    }

    pub fn current_memory_bytes(&self) -> usize {
        self.limiter.current_memory_bytes()
    }

    /// Accessor used by [`new_plugin_store`] to wire the limiter into wasmtime.
    /// Kept `pub(crate)` because callers should not poke at the limiter
    /// directly — the runtime owns mutation.
    pub(crate) fn limiter_mut(&mut self) -> &mut PluginResourceLimiter {
        &mut self.limiter
    }
}

/// Build a wasmtime [`Store`] tied to this engine and its policies.
///
/// - `fuel` is consumed by every wasm instruction when the engine has fuel
///   metering enabled. Pass `u64::MAX / 2` to effectively disable it in tests
///   that do not exercise fuel.
/// - `epoch_deadline` (in ticks past the engine's current epoch) governs how
///   many [`WasmEngine::increment_epoch`] calls must occur before the store
///   traps on its next safepoint.
pub fn new_plugin_store(
    engine: &WasmEngine,
    context: WasmStoreContext,
    fuel: u64,
    epoch_deadline: Option<u64>,
) -> Store<WasmStoreContext> {
    let mut store = Store::new(engine.engine(), context);
    store.limiter(|ctx| ctx.limiter_mut());
    if engine.config().consume_fuel {
        store
            .set_fuel(fuel)
            .expect("fuel metering enabled at engine level");
    }
    if engine.config().epoch_interruption {
        store.set_epoch_deadline(epoch_deadline.unwrap_or(u64::MAX));
    }
    store
}
