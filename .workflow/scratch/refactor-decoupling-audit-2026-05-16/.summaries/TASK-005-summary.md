# TASK-005 Summary - Core Message Domain Split

Status: completed

## Result

`crates/astrbot-core/src/message.rs` was replaced by a `message/` module tree with `mod.rs` as the public facade.

Focused modules:

- `component.rs`: `MessageComponent` and component helpers.
- `chain.rs`: `MessageChain` construction, extraction, and validation helpers.
- `session.rs`: `MessageSessionKind`, `MessageSession`, and `MessageSender`.
- `sink.rs`: `MessageSink`.
- `result.rs`: `EventResultType`, `ResultContentType`, `MessageStream`, and `MessageEventResult`.
- `provider_request.rs`: provider request/context/tool DTOs and event-default helpers.
- `event.rs`: `MessageEvent` state and event-level accessors.
- `tests.rs`: existing message domain tests.

Public exports from `astrbot_core::message` and `astrbot_core` remain compatible.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-core`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

All passed.

## Next

Continue `M7-R4` with `TASK-006`: split `crates/astrbot-pipeline/src/context.rs` into context, policy, ports, session, content safety, provider preference, quote context, provider fallback, and result decoration modules.
