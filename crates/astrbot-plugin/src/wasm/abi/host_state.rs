//! Host-side state attached to a plugin's wasmtime store.
//!
//! While [`crate::wasm::WasmStoreContext`] tracks engine-level concerns
//! (memory limits, plugin id), the **ABI host state** sits on top and tracks
//! the things the host functions need to mutate during a single
//! `astrbot_dispatch` call: collected log lines, collected outbound messages,
//! and the active capability set.

use std::collections::HashSet;

use crate::wasm::WasmStoreContext;
use crate::wasm::abi::messages::{LogLevel, LogRecord, OutboundMessage};

/// State carried alongside the wasmtime store while a guest call runs.
pub struct AbiHostState {
    base: WasmStoreContext,
    capabilities: HashSet<String>,
    outbound_messages: Vec<OutboundMessage>,
    log_records: Vec<LogRecord>,
}

impl AbiHostState {
    pub fn new(base: WasmStoreContext) -> Self {
        Self {
            base,
            capabilities: HashSet::new(),
            outbound_messages: Vec::new(),
            log_records: Vec::new(),
        }
    }

    pub fn with_capabilities<I, S>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for capability in capabilities {
            self.capabilities.insert(capability.into());
        }
        self
    }

    pub fn base(&self) -> &WasmStoreContext {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut WasmStoreContext {
        &mut self.base
    }

    pub fn plugin_id(&self) -> &str {
        self.base.plugin_id()
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn record_log(&mut self, level: LogLevel, message: String) {
        self.log_records.push(LogRecord { level, message });
    }

    pub fn push_outbound(&mut self, message: OutboundMessage) {
        self.outbound_messages.push(message);
    }

    pub fn drain_outbound(&mut self) -> Vec<OutboundMessage> {
        std::mem::take(&mut self.outbound_messages)
    }

    pub fn drain_logs(&mut self) -> Vec<LogRecord> {
        std::mem::take(&mut self.log_records)
    }

    /// Borrow the resource limiter wasmtime queries on memory/table growth.
    /// Mirrors [`WasmStoreContext::limiter_mut`] so callers can install this
    /// state into a `Store::limiter`. Not yet used internally — kept for
    /// integrators that wire `AbiHostState` into a fueled store.
    #[allow(dead_code)]
    pub(crate) fn limiter_mut(&mut self) -> &mut crate::wasm::PluginResourceLimiter {
        self.base.limiter_mut()
    }
}

/// Standard capability identifiers. Host functions consult these names when
/// gating sensitive operations.
pub mod capability {
    pub const LOG: &str = "log";
    pub const MESSAGING: &str = "messaging";
    pub const HTTP_FETCH: &str = "http_fetch";
    pub const KV: &str = "kv";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::PluginResourceLimits;

    #[test]
    fn capabilities_are_tracked() {
        let base = WasmStoreContext::new("test", PluginResourceLimits::default());
        let state = AbiHostState::new(base).with_capabilities([capability::LOG, "custom"]);
        assert!(state.has_capability(capability::LOG));
        assert!(state.has_capability("custom"));
        assert!(!state.has_capability(capability::HTTP_FETCH));
    }

    #[test]
    fn drain_returns_buffered_items() {
        let base = WasmStoreContext::new("test", PluginResourceLimits::default());
        let mut state = AbiHostState::new(base);
        state.push_outbound(OutboundMessage {
            session_id: "s1".into(),
            text: "hi".into(),
        });
        state.record_log(LogLevel::Info, "boot".into());
        let messages = state.drain_outbound();
        let logs = state.drain_logs();
        assert_eq!(messages.len(), 1);
        assert_eq!(logs.len(), 1);
        // After drain, follow-up drains return empty.
        assert!(state.drain_outbound().is_empty());
        assert!(state.drain_logs().is_empty());
    }
}
