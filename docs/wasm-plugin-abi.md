# AstrBot WASM Plugin ABI v1.0

> Status: stable for v1.x; minor bumps add fields without breaking existing
> plugins. Major bumps require recompilation.

The host loads plugin artefacts compiled to a wasm32 target (typically
`wasm32-wasip1`). Plugins talk to the host through a small set of imports
under the `astrbot` namespace and a fixed set of exports the loader expects.

## Versioning

Two constants are exported by `astrbot-plugin-sdk` and consumed by the host:

- `ABI_VERSION_MAJOR` (current: `1`)
- `ABI_VERSION_MINOR` (current: `0`)

Plugins declare their target ABI in `plugin.toml`:

```toml
abi_major = 1
abi_minor = 0
```

The host refuses to load any plugin whose `abi_major` does not match its own.

## Memory protocol

Structured payloads cross the boundary as **length-prefixed UTF-8 JSON** in
the guest's linear memory.

- Guest → host returns pack a `(ptr, len)` pair into a single `u64`:
  - upper 32 bits = pointer
  - lower 32 bits = byte length

  Helpers: `pack_ptr_len(ptr, len)` / `unpack_ptr_len(value)` exported by
  both the host and the SDK.

- Host → guest first calls `astrbot_alloc(len)` to reserve a buffer, copies
  bytes into the guest's memory, then invokes the relevant export with
  `(ptr, len)`. The guest is responsible for calling `astrbot_free(ptr, len)`
  when it is done with the buffer.

## Required guest exports

| Export | Signature | Purpose |
|--------|-----------|---------|
| `memory` | — | Linear memory; the host reads JSON from here. |
| `astrbot_alloc` | `(len: i32) -> i32` | Allocate a buffer of `len` bytes. Returns 0 on failure. |
| `astrbot_free` | `(ptr: i32, len: i32)` | Free a buffer previously returned by `astrbot_alloc`. |
| `astrbot_abi_version` | `() -> i32` | Returns `ABI_VERSION_MAJOR`. |
| `astrbot_init` | `(ptr: i32, len: i32) -> i64` | Called once after instantiation with a JSON-encoded `PluginInitRequest`. Returns a packed `(ptr, len)` to a JSON `PluginInitResponse`. |
| `astrbot_dispatch` | `(ptr: i32, len: i32) -> i64` | Called for every event with a `PluginEvent`. Returns a packed `(ptr, len)` to a `PluginResponse`. |

The `plugin_main!` macro in the SDK generates all six exports
automatically — most plugin authors never write them by hand.

## Host imports (v1)

All imports live under the `astrbot` module namespace.

| Import | Signature | Capability | Notes |
|--------|-----------|-----------|-------|
| `host_abi_version` | `() -> i64` | (none) | Returns `(major, minor)` packed: upper 32 = major, lower 32 = minor. |
| `host_log` | `(level: i32, ptr: i32, len: i32) -> i32` | `log` | Append a log line. Level: 0=trace, 1=debug, 2=info, 3=warn, 4=error. |
| `host_send_message` | `(ptr: i32, len: i32) -> i32` | `messaging` | Buffer an outbound message. Payload is a JSON-encoded `OutboundMessage`. |

Each capability-gated import returns `-1` (`ABI_RESULT_DENIED`) if the
plugin's manifest did not request the relevant capability. Other negative
return codes:

| Code | Meaning |
|------|---------|
|  `0` | success |
| `-1` | capability denied |
| `-2` | invalid memory region |
| `-3` | invalid UTF-8 payload |
| `-4` | invalid JSON payload |

## Capabilities

A capability is the unit of permission. The default stance is **deny** —
plugins that need anything outside pure computation must request it in
`plugin.toml`:

```toml
capabilities = ["log", "messaging"]
```

Known capability identifiers:

| Identifier | Grants |
|-----------|--------|
| `log` | `host_log` |
| `messaging` | `host_send_message` |
| `kv` | host KV store (planned) |
| `http_fetch` | outbound HTTP (planned) |
| `tools` | invoking host-registered tools (planned) |

## Message types

The wire types are stable JSON. Field names use snake_case. Unknown fields
are ignored on parse so new minor versions can add optional fields without
breaking older plugins.

### `PluginInitRequest`

```json
{
  "plugin_id": "echo",
  "host_abi_major": 1,
  "host_abi_minor": 0,
  "config": {},
  "capabilities": ["messaging"]
}
```

### `PluginInitResponse`

```json
{
  "plugin_abi_major": 1,
  "plugin_abi_minor": 0,
  "commands": [
    { "keyword": "echo", "description": null }
  ],
  "status": null,
  "error": null
}
```

### `PluginEvent`

Tagged JSON enum with three variants:

```json
{ "type": "ping" }
{ "type": "message", "session_id": "...", "sender_id": "...", "text": "..." }
{ "type": "command", "session_id": "...", "sender_id": "...", "keyword": "echo", "argument": "hi" }
```

### `PluginResponse`

```json
{ "type": "no_op" }
{ "type": "pong" }
{ "type": "replies", "messages": [{ "session_id": "...", "text": "..." }] }
{ "type": "error", "message": "..." }
```

## Reference implementation

See `examples/wasm-plugins/echo/` for the minimum viable Rust plugin (about
30 lines of source). Build with:

```bash
rustup target add wasm32-wasip1
cd examples/wasm-plugins/echo
cargo build --target wasm32-wasip1 --release
```

The resulting `target/wasm32-wasip1/release/echo_plugin.wasm` is what the
host's `WasmPluginLoader::load_file` consumes.
