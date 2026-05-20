//! Host function registry for the WASM plugin ABI.
//!
//! These are the imports a plugin can declare from the `"astrbot"` namespace.
//! Each function is intentionally small — it reads a JSON blob out of guest
//! memory, validates it, and routes the call into [`AbiHostState`]. Output
//! that needs to travel back to the guest is allocated by the guest (via
//! `astrbot_alloc`) and the resulting `(ptr, len)` pair is packed into the
//! `u64` return value (see [`crate::wasm::abi::pack_ptr_len`]).
//!
//! V1 surface (registered when `register_host_fns` is called):
//!
//! | Import                      | Capability        | Behaviour                            |
//! |-----------------------------|-------------------|--------------------------------------|
//! | `astrbot.host_log`          | `log`             | Append a log record.                 |
//! | `astrbot.host_send_message` | `messaging`       | Buffer one outbound message.         |
//! | `astrbot.host_abi_version`  | (none)            | Returns packed `(major, minor)`.     |
//!
//! All other V1 surfaces (KV, HTTP fetch, tool dispatch) will land in a
//! follow-up — the linker exposed here is enough to round-trip a hello-world
//! plugin end-to-end.

use wasmtime::{Caller, Engine, Extern, Linker, Memory};

use crate::wasm::abi::host_state::{AbiHostState, capability};
use crate::wasm::abi::messages::{LogLevel, OutboundMessage};
use crate::wasm::abi::{ABI_VERSION_MAJOR, ABI_VERSION_MINOR};

/// Register every host function on the given [`Linker`].
///
/// The linker is typed against [`AbiHostState`], so plugins must run inside a
/// `Store<AbiHostState>`. Callers can layer their own additional imports on
/// the same linker before instantiating a module.
pub fn register_host_fns(linker: &mut Linker<AbiHostState>) -> wasmtime::Result<()> {
    register_log(linker)?;
    register_send_message(linker)?;
    register_abi_version(linker)?;
    Ok(())
}

/// Convenience helper for tests and integrators: build a default linker for
/// the given engine in one call.
pub fn make_default_linker(engine: &Engine) -> wasmtime::Result<Linker<AbiHostState>> {
    let mut linker = Linker::new(engine);
    register_host_fns(&mut linker)?;
    Ok(linker)
}

fn register_log(linker: &mut Linker<AbiHostState>) -> wasmtime::Result<()> {
    linker.func_wrap(
        "astrbot",
        "host_log",
        |mut caller: Caller<'_, AbiHostState>, level: i32, ptr: i32, len: i32| -> i32 {
            if !caller.data().has_capability(capability::LOG) {
                return ABI_RESULT_DENIED;
            }
            let payload = match read_guest_string(&mut caller, ptr, len) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let level = log_level_from_i32(level).unwrap_or(LogLevel::Info);
            caller.data_mut().record_log(level, payload);
            ABI_RESULT_OK
        },
    )?;
    Ok(())
}

fn register_send_message(linker: &mut Linker<AbiHostState>) -> wasmtime::Result<()> {
    linker.func_wrap(
        "astrbot",
        "host_send_message",
        |mut caller: Caller<'_, AbiHostState>, ptr: i32, len: i32| -> i32 {
            if !caller.data().has_capability(capability::MESSAGING) {
                return ABI_RESULT_DENIED;
            }
            let payload = match read_guest_string(&mut caller, ptr, len) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let message: OutboundMessage = match serde_json::from_str(&payload) {
                Ok(value) => value,
                Err(_) => return ABI_RESULT_INVALID_JSON,
            };
            caller.data_mut().push_outbound(message);
            ABI_RESULT_OK
        },
    )?;
    Ok(())
}

fn register_abi_version(linker: &mut Linker<AbiHostState>) -> wasmtime::Result<()> {
    linker.func_wrap("astrbot", "host_abi_version", || -> i64 {
        // Pack (major, minor) so guests can extract both without a second call.
        ((ABI_VERSION_MAJOR as i64) << 32) | ((ABI_VERSION_MINOR as i64) & 0xFFFF_FFFF)
    })?;
    Ok(())
}

/// Result codes returned by every void-shaped host call. Negative codes are
/// errors, zero is success.
pub const ABI_RESULT_OK: i32 = 0;
pub const ABI_RESULT_DENIED: i32 = -1;
pub const ABI_RESULT_INVALID_MEMORY: i32 = -2;
pub const ABI_RESULT_INVALID_UTF8: i32 = -3;
pub const ABI_RESULT_INVALID_JSON: i32 = -4;

fn log_level_from_i32(level: i32) -> Option<LogLevel> {
    match level {
        0 => Some(LogLevel::Trace),
        1 => Some(LogLevel::Debug),
        2 => Some(LogLevel::Info),
        3 => Some(LogLevel::Warn),
        4 => Some(LogLevel::Error),
        _ => None,
    }
}

fn read_guest_string(
    caller: &mut Caller<'_, AbiHostState>,
    ptr: i32,
    len: i32,
) -> Result<String, i32> {
    let memory = match caller.get_export("memory") {
        Some(Extern::Memory(memory)) => memory,
        _ => return Err(ABI_RESULT_INVALID_MEMORY),
    };
    let data = read_memory_bytes(caller, &memory, ptr, len)?;
    String::from_utf8(data).map_err(|_| ABI_RESULT_INVALID_UTF8)
}

fn read_memory_bytes(
    caller: &mut Caller<'_, AbiHostState>,
    memory: &Memory,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>, i32> {
    if ptr < 0 || len < 0 {
        return Err(ABI_RESULT_INVALID_MEMORY);
    }
    let ptr = ptr as usize;
    let len = len as usize;
    let data = memory.data(&caller);
    let end = ptr.checked_add(len).ok_or(ABI_RESULT_INVALID_MEMORY)?;
    if end > data.len() {
        return Err(ABI_RESULT_INVALID_MEMORY);
    }
    Ok(data[ptr..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::{PluginResourceLimits, WasmEngine, WasmStoreContext};

    fn make_engine() -> WasmEngine {
        WasmEngine::with_defaults().expect("engine builds")
    }

    fn make_state() -> AbiHostState {
        AbiHostState::new(WasmStoreContext::new(
            "test.abi",
            PluginResourceLimits::default(),
        ))
    }

    #[test]
    fn default_linker_registers_known_imports() {
        let engine = make_engine();
        // Just building the linker validates that every host fn passed
        // wasmtime's signature checks. A follow-up instantiation test below
        // confirms that the imports are wired up correctly at runtime.
        let _linker = make_default_linker(engine.engine()).expect("linker");
    }

    #[test]
    fn abi_version_packs_major_and_minor() {
        let engine = make_engine();
        let module = engine
            .compile_wat(
                r#"(module
                    (import "astrbot" "host_abi_version" (func $v (result i64)))
                    (func (export "ask") (result i64) call $v))"#,
            )
            .expect("module compiles");

        let linker = make_default_linker(engine.engine()).expect("linker");
        let mut store = wasmtime::Store::new(engine.engine(), make_state());
        store.set_fuel(1_000_000).expect("fuel");
        store.set_epoch_deadline(u64::MAX);

        let instance = linker
            .instantiate(&mut store, &module)
            .expect("instantiate");
        let ask = instance
            .get_typed_func::<(), i64>(&mut store, "ask")
            .expect("ask export");
        let packed = ask.call(&mut store, ()).expect("call");
        let major = (packed >> 32) as i32;
        let minor = (packed & 0xFFFF_FFFF) as i32;
        assert_eq!(major, ABI_VERSION_MAJOR);
        assert_eq!(minor, ABI_VERSION_MINOR);
    }

    #[test]
    fn host_log_denied_without_capability() {
        let engine = make_engine();
        let module = engine
            .compile_wat(
                r#"(module
                    (import "astrbot" "host_log" (func $log (param i32 i32 i32) (result i32)))
                    (memory (export "memory") 1)
                    (func (export "try_log") (result i32)
                        i32.const 2
                        i32.const 0
                        i32.const 0
                        call $log))"#,
            )
            .expect("module compiles");

        let linker = make_default_linker(engine.engine()).expect("linker");
        // No capabilities granted.
        let state = AbiHostState::new(WasmStoreContext::new(
            "test.abi",
            PluginResourceLimits::default(),
        ));
        let mut store = wasmtime::Store::new(engine.engine(), state);
        store.set_fuel(1_000_000).expect("fuel");
        store.set_epoch_deadline(u64::MAX);

        let instance = linker
            .instantiate(&mut store, &module)
            .expect("instantiate");
        let try_log = instance
            .get_typed_func::<(), i32>(&mut store, "try_log")
            .expect("export");
        assert_eq!(try_log.call(&mut store, ()).unwrap(), ABI_RESULT_DENIED);
    }

    #[test]
    fn host_log_buffers_record_when_capability_granted() {
        let engine = make_engine();
        // Plant the literal "hi" at offset 0 in memory using a data segment,
        // then call host_log(level=2, ptr=0, len=2).
        let module = engine
            .compile_wat(
                r#"(module
                    (import "astrbot" "host_log" (func $log (param i32 i32 i32) (result i32)))
                    (memory (export "memory") 1)
                    (data (i32.const 0) "hi")
                    (func (export "trigger") (result i32)
                        i32.const 2
                        i32.const 0
                        i32.const 2
                        call $log))"#,
            )
            .expect("module compiles");

        let linker = make_default_linker(engine.engine()).expect("linker");
        let state = AbiHostState::new(WasmStoreContext::new(
            "test.abi",
            PluginResourceLimits::default(),
        ))
        .with_capabilities([capability::LOG]);
        let mut store = wasmtime::Store::new(engine.engine(), state);
        store.set_fuel(1_000_000).expect("fuel");
        store.set_epoch_deadline(u64::MAX);

        let instance = linker
            .instantiate(&mut store, &module)
            .expect("instantiate");
        let trigger = instance
            .get_typed_func::<(), i32>(&mut store, "trigger")
            .expect("export");
        assert_eq!(trigger.call(&mut store, ()).unwrap(), ABI_RESULT_OK);
        let logs = store.data_mut().drain_logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].level, LogLevel::Info);
        assert_eq!(logs[0].message, "hi");
    }
}
