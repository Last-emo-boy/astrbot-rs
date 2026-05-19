//! AstrBot plugin WASM ABI definitions.
//!
//! The ABI is the contract between the host and a WASM plugin: which symbols
//! the guest must export, what host functions it can import, and how
//! structured data flows across the boundary.
//!
//! ## Memory protocol
//!
//! All structured payloads cross the boundary as **length-prefixed UTF-8
//! JSON** in the guest's linear memory.
//!
//! * Guest → host: the guest writes the payload into its memory and returns a
//!   packed `u64` where the upper 32 bits hold the pointer and the lower 32
//!   bits hold the byte length: `(ptr as u64) << 32 | len as u64`.
//! * Host → guest: the host calls `astrbot_alloc(len) -> ptr` on the guest to
//!   reserve a buffer, copies the bytes into the guest's memory, then invokes
//!   the relevant export with `(ptr, len)`. The guest is responsible for
//!   eventually freeing via `astrbot_free(ptr, len)`.
//!
//! ## Required guest exports
//!
//! * `astrbot_abi_version() -> i32` — returns [`ABI_VERSION_MAJOR`].
//! * `memory` — exported linear memory.
//! * `astrbot_alloc(len: i32) -> i32` — bump/heap allocator owned by the
//!   guest.
//! * `astrbot_free(ptr: i32, len: i32)` — paired with `astrbot_alloc`.
//! * `astrbot_init(ptr: i32, len: i32) -> u64` — called once after
//!   instantiation with a JSON-encoded [`messages::PluginInitRequest`]; must
//!   return a JSON-encoded [`messages::PluginInitResponse`].
//! * `astrbot_dispatch(ptr: i32, len: i32) -> u64` — invoked for every
//!   inbound event with a JSON-encoded [`messages::PluginEvent`]; must return
//!   a JSON-encoded [`messages::PluginResponse`].
//!
//! ## Host functions
//!
//! See [`host_fns::register_host_fns`] for the full surface. V1 covers
//! logging and message-send; capability-gated functions (HTTP fetch, tool
//! invocation, KV store) will be layered on top later.

pub mod host_fns;
pub mod host_state;
pub mod messages;

/// Major ABI version. Incompatible changes bump this number.
pub const ABI_VERSION_MAJOR: i32 = 1;

/// Minor ABI version. New host functions / message fields bump this number
/// while preserving backward compatibility for already-shipped plugins.
pub const ABI_VERSION_MINOR: i32 = 0;

/// Compact `"major.minor"` string useful for logging and manifests.
pub const ABI_VERSION: &str = "1.0";

/// Pack a `(ptr, len)` pair into the `u64` shape the ABI uses for return
/// values. Symmetrical with [`unpack_ptr_len`].
#[inline]
pub fn pack_ptr_len(ptr: u32, len: u32) -> u64 {
    ((ptr as u64) << 32) | (len as u64)
}

/// Reverse of [`pack_ptr_len`].
#[inline]
pub fn unpack_ptr_len(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, (value & 0xFFFF_FFFF) as u32)
}

/// Errors emitted while crossing the ABI boundary.
#[derive(Debug, thiserror::Error)]
pub enum AbiError {
    #[error("plugin declares ABI {declared}, host implements {host}")]
    VersionMismatch { declared: i32, host: i32 },
    #[error("missing required export: {0}")]
    MissingExport(String),
    #[error("invalid memory region (ptr={ptr:#x}, len={len})")]
    InvalidMemoryRegion { ptr: u32, len: u32 },
    #[error("invalid utf-8 payload: {0}")]
    InvalidUtf8(String),
    #[error("invalid json payload: {0}")]
    InvalidJson(String),
}

impl From<AbiError> for astrbot_core::AstrbotError {
    fn from(value: AbiError) -> Self {
        astrbot_core::AstrbotError::Pipeline(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_roundtrip() {
        let ptr = 0x1234_5678_u32;
        let len = 0xABCD_u32;
        let packed = pack_ptr_len(ptr, len);
        assert_eq!(unpack_ptr_len(packed), (ptr, len));
    }

    #[test]
    fn version_constants_consistent() {
        assert_eq!(ABI_VERSION_MAJOR, 1);
        assert_eq!(ABI_VERSION_MINOR, 0);
        assert_eq!(ABI_VERSION, "1.0");
    }
}
