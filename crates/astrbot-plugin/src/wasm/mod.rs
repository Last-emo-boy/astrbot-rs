//! WASM plugin sandbox primitives.
//!
//! This module wraps the wasmtime runtime with policies tuned for AstrBot
//! plugins: epoch-based interruption, deterministic fuel metering, and
//! per-instance resource ceilings. The goal is to load untrusted plugin
//! bytecode without giving up host stability.

pub mod abi;
pub mod capability;
mod engine;
mod loader;
mod manifest;
mod resource_limits;
mod store;

pub use capability::{Capability, CapabilitySet, UnknownCapability};
pub use engine::{WasmEngine, WasmEngineConfig, WasmEngineError};
pub use loader::{
    WasmInstanceConfig, WasmLifecycle, WasmLoaderError, WasmPluginInstance, WasmPluginLoader,
};
pub use manifest::{ManifestError, WasmPluginManifest};
pub use resource_limits::{PluginResourceLimiter, PluginResourceLimits};
pub use store::{WasmStoreContext, new_plugin_store};
