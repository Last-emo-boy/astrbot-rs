//! WASM plugin loader: lifecycle, instantiation, hot reload.
//!
//! The loader owns the compiled module, the per-instance store, and the
//! linker. It exposes a small, sync API:
//!
//! 1. `load_bytes` / `load_file` — compile and instantiate.
//! 2. `init` — call `astrbot_init`, capture the registered commands, move to
//!    `ready`.
//! 3. `reload` — atomically swap module bytes; the old instance is dropped
//!    after the new one initialises.
//! 4. `unload` — drop the store and forget the plugin.
//!
//! The loader is **not** the file watcher. A watcher daemon will call
//! `reload` whenever the artefact on disk changes; that wiring lands in a
//! follow-up.

use std::collections::HashMap;
use std::path::Path;

use wasmtime::{Instance, Store, TypedFunc};

use crate::wasm::abi::host_fns::make_default_linker;
use crate::wasm::abi::host_state::AbiHostState;
use crate::wasm::abi::messages::{CommandRegistration, PluginInitRequest, PluginInitResponse};
use crate::wasm::abi::{ABI_VERSION_MAJOR, ABI_VERSION_MINOR, unpack_ptr_len};
use crate::wasm::engine::WasmEngine;
use crate::wasm::manifest::WasmPluginManifest;
use crate::wasm::store::WasmStoreContext;

/// Configuration knobs per plugin instance. Defaults are conservative enough
/// for hello-world plugins; the host operator tunes them through
/// `plugin.toml` or the management API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasmInstanceConfig {
    /// Initial fuel to allocate. Each guest instruction consumes ≥ 1.
    pub fuel: u64,
    /// Epoch deadline relative to the engine's current tick. Useful when a
    /// watchdog thread pumps the engine clock to enforce wall-clock budgets.
    pub epoch_deadline: u64,
}

impl Default for WasmInstanceConfig {
    fn default() -> Self {
        Self {
            fuel: 10_000_000,
            epoch_deadline: u64::MAX,
        }
    }
}

/// Lifecycle states observed during loading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmLifecycle {
    /// Module compiled, instance built but `astrbot_init` not yet called.
    Instantiated,
    /// `astrbot_init` returned successfully.
    Ready,
    /// `astrbot_init` returned an error or the plugin trapped during init.
    Failed,
    /// `unload` was called.
    Unloaded,
}

#[derive(Debug, thiserror::Error)]
pub enum WasmLoaderError {
    #[error("wasm engine error: {0}")]
    Engine(String),
    #[error("module is missing required export: {0}")]
    MissingExport(String),
    #[error("plugin init returned an error: {0}")]
    InitFailed(String),
    #[error("plugin trapped: {0}")]
    Trap(String),
    #[error("invalid memory region returned by guest: ptr={ptr}, len={len}")]
    InvalidGuestPointer { ptr: u32, len: u32 },
    #[error("guest returned invalid utf-8")]
    InvalidUtf8,
    #[error("guest returned invalid json: {0}")]
    InvalidJson(String),
}

impl From<WasmLoaderError> for astrbot_core::AstrbotError {
    fn from(value: WasmLoaderError) -> Self {
        astrbot_core::AstrbotError::Pipeline(value.to_string())
    }
}

/// State of a single loaded plugin instance.
pub struct WasmPluginInstance {
    plugin_id: String,
    lifecycle: WasmLifecycle,
    store: Store<AbiHostState>,
    instance: Instance,
    commands: Vec<CommandRegistration>,
}

impl WasmPluginInstance {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn lifecycle(&self) -> WasmLifecycle {
        self.lifecycle
    }

    pub fn commands(&self) -> &[CommandRegistration] {
        &self.commands
    }

    /// Borrow the underlying store. Tests and integrators may use this to
    /// inspect drained logs / outbound messages after a guest call.
    pub fn store_mut(&mut self) -> &mut Store<AbiHostState> {
        &mut self.store
    }

    /// Borrow the underlying instance for direct export calls.
    pub fn instance(&self) -> &Instance {
        &self.instance
    }
}

/// Loader for WASM plugins. Acts as a thin registry mapping `plugin_id` →
/// live instance.
pub struct WasmPluginLoader {
    engine: WasmEngine,
    instances: HashMap<String, WasmPluginInstance>,
}

impl WasmPluginLoader {
    pub fn new(engine: WasmEngine) -> Self {
        Self {
            engine,
            instances: HashMap::new(),
        }
    }

    pub fn engine(&self) -> &WasmEngine {
        &self.engine
    }

    /// Compile and instantiate a plugin from raw bytes. Returns the
    /// post-init [`PluginInitResponse`].
    pub fn load_bytes(
        &mut self,
        manifest: &WasmPluginManifest,
        bytes: &[u8],
        config: WasmInstanceConfig,
    ) -> Result<PluginInitResponse, WasmLoaderError> {
        manifest
            .validate_abi()
            .map_err(|err| WasmLoaderError::Engine(err.to_string()))?;

        let module = self
            .engine
            .compile_module(bytes)
            .map_err(|err| WasmLoaderError::Engine(err.to_string()))?;

        let linker = make_default_linker(self.engine.engine())
            .map_err(|err| WasmLoaderError::Engine(err.to_string()))?;

        let base = WasmStoreContext::new(manifest.id.clone(), crate::wasm::PluginResourceLimits::default());
        let state = AbiHostState::new(base).with_capabilities(manifest.capabilities.as_str_vec());
        let mut store = make_store(&self.engine, state, config);

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| WasmLoaderError::Trap(err.to_string()))?;

        require_export(&instance, &mut store, "memory")?;
        require_export(&instance, &mut store, "astrbot_alloc")?;

        let mut plugin = WasmPluginInstance {
            plugin_id: manifest.id.clone(),
            lifecycle: WasmLifecycle::Instantiated,
            store,
            instance,
            commands: Vec::new(),
        };

        let response = call_init(&mut plugin, manifest)?;
        if let Some(err) = response.error.clone() {
            plugin.lifecycle = WasmLifecycle::Failed;
            self.instances.insert(manifest.id.clone(), plugin);
            return Err(WasmLoaderError::InitFailed(err));
        }
        plugin.commands = response.commands.clone();
        plugin.lifecycle = WasmLifecycle::Ready;
        self.instances.insert(manifest.id.clone(), plugin);
        Ok(response)
    }

    /// Read bytes from `path` and forward to [`load_bytes`].
    pub fn load_file(
        &mut self,
        manifest: &WasmPluginManifest,
        path: impl AsRef<Path>,
        config: WasmInstanceConfig,
    ) -> Result<PluginInitResponse, WasmLoaderError> {
        let bytes = std::fs::read(path.as_ref())
            .map_err(|err| WasmLoaderError::Engine(format!("failed to read wasm: {err}")))?;
        self.load_bytes(manifest, &bytes, config)
    }

    /// Replace the current artefact for `plugin_id` with new bytes. The old
    /// instance is dropped after the new one initialises successfully.
    pub fn reload(
        &mut self,
        manifest: &WasmPluginManifest,
        bytes: &[u8],
        config: WasmInstanceConfig,
    ) -> Result<PluginInitResponse, WasmLoaderError> {
        // Load into a fresh entry first; only on success do we drop the old
        // instance. If load_bytes fails the previous instance stays alive.
        let old = self.instances.remove(&manifest.id);
        match self.load_bytes(manifest, bytes, config) {
            Ok(response) => {
                drop(old);
                Ok(response)
            }
            Err(err) => {
                if let Some(prev) = old {
                    self.instances.insert(manifest.id.clone(), prev);
                }
                Err(err)
            }
        }
    }

    /// Look up an active instance.
    pub fn get(&self, plugin_id: &str) -> Option<&WasmPluginInstance> {
        self.instances.get(plugin_id)
    }

    pub fn get_mut(&mut self, plugin_id: &str) -> Option<&mut WasmPluginInstance> {
        self.instances.get_mut(plugin_id)
    }

    /// Drop the instance for `plugin_id`. Returns whether something was
    /// removed.
    pub fn unload(&mut self, plugin_id: &str) -> bool {
        if let Some(mut plugin) = self.instances.remove(plugin_id) {
            plugin.lifecycle = WasmLifecycle::Unloaded;
            true
        } else {
            false
        }
    }

    pub fn loaded_ids(&self) -> impl Iterator<Item = &str> {
        self.instances.keys().map(|s| s.as_str())
    }
}

fn make_store(
    engine: &WasmEngine,
    state: AbiHostState,
    config: WasmInstanceConfig,
) -> Store<AbiHostState> {
    let mut store = Store::new(engine.engine(), state);
    if engine.config().consume_fuel {
        store.set_fuel(config.fuel).expect("fuel metering enabled");
    }
    if engine.config().epoch_interruption {
        store.set_epoch_deadline(config.epoch_deadline);
    }
    store
}

fn require_export(
    instance: &Instance,
    store: &mut Store<AbiHostState>,
    name: &str,
) -> Result<(), WasmLoaderError> {
    if instance.get_export(store, name).is_none() {
        return Err(WasmLoaderError::MissingExport(name.to_string()));
    }
    Ok(())
}

fn call_init(
    plugin: &mut WasmPluginInstance,
    manifest: &WasmPluginManifest,
) -> Result<PluginInitResponse, WasmLoaderError> {
    let request = PluginInitRequest {
        plugin_id: manifest.id.clone(),
        host_abi_major: ABI_VERSION_MAJOR,
        host_abi_minor: ABI_VERSION_MINOR,
        config: serde_json::Map::new(),
        capabilities: manifest.capabilities.as_str_vec(),
    };
    let payload = serde_json::to_vec(&request)
        .map_err(|err| WasmLoaderError::Engine(format!("init serialise: {err}")))?;

    let memory = plugin
        .instance
        .get_memory(&mut plugin.store, "memory")
        .ok_or_else(|| WasmLoaderError::MissingExport("memory".to_string()))?;
    let alloc: TypedFunc<i32, i32> = plugin
        .instance
        .get_typed_func(&mut plugin.store, "astrbot_alloc")
        .map_err(|err| WasmLoaderError::Engine(err.to_string()))?;
    let init: TypedFunc<(i32, i32), i64> = plugin
        .instance
        .get_typed_func(&mut plugin.store, "astrbot_init")
        .map_err(|_| WasmLoaderError::MissingExport("astrbot_init".to_string()))?;

    let ptr = alloc
        .call(&mut plugin.store, payload.len() as i32)
        .map_err(|err| WasmLoaderError::Trap(err.to_string()))?;
    if ptr <= 0 {
        return Err(WasmLoaderError::InitFailed(format!(
            "astrbot_alloc returned non-positive pointer {ptr}"
        )));
    }
    memory
        .write(&mut plugin.store, ptr as usize, &payload)
        .map_err(|err| WasmLoaderError::Engine(err.to_string()))?;

    let packed = init
        .call(&mut plugin.store, (ptr, payload.len() as i32))
        .map_err(|err| WasmLoaderError::Trap(err.to_string()))?;
    let response_bytes = read_guest_buffer(plugin, packed as u64)?;
    let response: PluginInitResponse = serde_json::from_slice(&response_bytes)
        .map_err(|err| WasmLoaderError::InvalidJson(err.to_string()))?;
    Ok(response)
}

fn read_guest_buffer(
    plugin: &mut WasmPluginInstance,
    packed: u64,
) -> Result<Vec<u8>, WasmLoaderError> {
    let (ptr, len) = unpack_ptr_len(packed);
    let memory = plugin
        .instance
        .get_memory(&mut plugin.store, "memory")
        .ok_or_else(|| WasmLoaderError::MissingExport("memory".to_string()))?;
    let data = memory.data(&plugin.store);
    let ptr_usize = ptr as usize;
    let len_usize = len as usize;
    let end = ptr_usize
        .checked_add(len_usize)
        .ok_or(WasmLoaderError::InvalidGuestPointer { ptr, len })?;
    if end > data.len() {
        return Err(WasmLoaderError::InvalidGuestPointer { ptr, len });
    }
    Ok(data[ptr_usize..end].to_vec())
}

// Re-export pack helper for guests that want to reach for it via the loader.
pub use crate::wasm::abi::pack_ptr_len as pack_guest_pointer;

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a trivial guest module that:
    /// - exports `memory`, `astrbot_alloc`, `astrbot_init`
    /// - returns the same canned JSON response from `astrbot_init`
    ///   regardless of input
    fn build_test_guest_wat() -> &'static str {
        // The canned response sits at offset 0 in linear memory. Bytes
        // (length = 75):
        // `{"plugin_abi_major":1,"plugin_abi_minor":0,"commands":[{"keyword":"echo"}]}`
        //
        // The bump allocator hands out memory starting at offset 256 so the
        // host's init payload doesn't collide with the canned response.
        r#"(module
            (memory (export "memory") 1)
            (data (i32.const 0) "{\"plugin_abi_major\":1,\"plugin_abi_minor\":0,\"commands\":[{\"keyword\":\"echo\"}]}")
            (global $next (mut i32) (i32.const 256))
            (func (export "astrbot_alloc") (param i32) (result i32)
                (local $out i32)
                global.get $next
                local.set $out
                global.get $next
                local.get 0
                i32.add
                global.set $next
                local.get $out)
            (func (export "astrbot_free") (param i32 i32))
            (func (export "astrbot_init") (param i32 i32) (result i64)
                ;; Always return (ptr=0, len=75) packed as upper32|lower32.
                i64.const 75))"#
    }

    fn engine() -> WasmEngine {
        WasmEngine::with_defaults().expect("engine builds")
    }

    fn manifest() -> WasmPluginManifest {
        let toml = format!(
            r#"id = "echo"
version = "0.1.0"
abi_major = {ABI_VERSION_MAJOR}
capabilities = ["log"]
"#
        );
        WasmPluginManifest::from_toml(&toml).unwrap()
    }

    fn wat_to_bytes(engine: &WasmEngine, source: &str) -> Vec<u8> {
        // Round-trip text -> module -> bytes so load_bytes hits the binary
        // path (compile_module).
        let module = engine.compile_wat(source).expect("compile wat");
        module.serialize().expect("serialise module")
    }

    #[test]
    fn round_trip_load_and_init() {
        let engine = engine();
        let manifest = manifest();
        let bytes = wat_to_bytes(&engine, build_test_guest_wat());

        let mut loader = WasmPluginLoader::new(engine);
        // The serialized module is precompiled; load_bytes treats it as raw
        // wasm. Compile via WAT path instead and feed the resulting module.
        // For test ergonomics we go through a direct call into the WAT path
        // and add a synthetic load.
        let module = loader
            .engine
            .compile_wat(build_test_guest_wat())
            .expect("wat module");
        let module_bytes = module.serialize().expect("serialise");
        // wasmtime can deserialize via Module::deserialize, but compile_module
        // expects raw wasm. We instead skip serialization for the test and
        // call the internal path: write a small inline helper that mirrors
        // load_bytes against a precompiled Module.
        let _ = (bytes, module_bytes);

        // Use the public API by compiling fresh raw bytes via wat::parse_str.
        let raw_wasm = wat::parse_str(build_test_guest_wat()).expect("wat to wasm");
        let response = loader
            .load_bytes(&manifest, &raw_wasm, WasmInstanceConfig::default())
            .expect("load");
        assert_eq!(response.plugin_abi_major, ABI_VERSION_MAJOR);
        assert_eq!(response.commands.len(), 1);
        assert_eq!(response.commands[0].keyword, "echo");

        let plugin = loader.get("echo").expect("registered");
        assert_eq!(plugin.lifecycle(), WasmLifecycle::Ready);
        assert_eq!(plugin.commands().len(), 1);
    }

    #[test]
    fn missing_required_export_rejected() {
        let engine = engine();
        let manifest = manifest();
        // Module missing astrbot_alloc.
        let wat = r#"(module
            (memory (export "memory") 1)
            (func (export "astrbot_init") (param i32 i32) (result i64) i64.const 0))"#;
        let raw_wasm = wat::parse_str(wat).expect("wat to wasm");

        let mut loader = WasmPluginLoader::new(engine);
        let err = loader
            .load_bytes(&manifest, &raw_wasm, WasmInstanceConfig::default())
            .unwrap_err();
        assert!(
            matches!(err, WasmLoaderError::MissingExport(ref name) if name == "astrbot_alloc")
        );
    }

    #[test]
    fn reload_swaps_instance() {
        let engine = engine();
        let manifest = manifest();
        let raw_wasm = wat::parse_str(build_test_guest_wat()).expect("wat to wasm");

        let mut loader = WasmPluginLoader::new(engine);
        loader
            .load_bytes(&manifest, &raw_wasm, WasmInstanceConfig::default())
            .unwrap();
        assert!(loader.get("echo").is_some());

        // Reload with the same bytes; old instance should be replaced.
        loader
            .reload(&manifest, &raw_wasm, WasmInstanceConfig::default())
            .unwrap();
        assert_eq!(loader.loaded_ids().count(), 1);

        assert!(loader.unload("echo"));
        assert!(!loader.unload("echo"));
    }
}
