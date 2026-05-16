# First Message Loop Context

## Loaded Specs

- EventBus stays thin and only dispatches events.
- Message flow stays typed: message event, message chain, provider request and pipeline result are Rust data structures.
- Platform and Provider implementations sit behind stable traits.
- First E2E test uses Mock Platform and Mock Provider.

## Reference Patterns From AstrBot

- `E:/Playground/Astrbot/astrbot/core/event_bus.py`: queue consumer dispatches an event to a scheduler and avoids business logic.
- `E:/Playground/Astrbot/astrbot/core/pipeline/scheduler.py`: ordered stages process a unified message event.
- `E:/Playground/Astrbot/astrbot/core/platform/platform.py`: platform adapters own inbound transport and submit events.
- `E:/Playground/Astrbot/astrbot/core/provider/provider.py`: Provider is a capability trait, not a concrete runtime dependency.
- `E:/Playground/Astrbot/astrbot/core/message/message_event_result.py`: message chain and event result are distinct from platform transport.

## Decisions

- Start with a deterministic sequential Stage chain; onion-style pre/post middleware can be added after the typed loop is validated.
- Put the thin EventBus in `astrbot-core`, depending only on an `EventExecutor` trait.
- Keep the first Provider stage simple: if no plugin/result exists, call `ChatProvider` with plain text and produce an LLM result.
- Mock Platform emits events through an mpsc channel and records outgoing messages through a `MessageSink`.

## Result

- Created Rust workspace crates: `astrbot-core`, `astrbot-provider`, `astrbot-platform`, `astrbot-pipeline`, `astrbot-cli`.
- Implemented typed `MessageChain`, `MessageEvent`, `MessageEventResult`, `MessageSink`, thin `EventBus`, `ChatProvider`, `PlatformAdapter`, `PipelineStage`, `PipelineScheduler`, `ProviderStage`, and `RespondStage`.
- Added integration test `mock_platform_message_reaches_provider_and_responds`.
- Verification passed: `cargo fmt --all --check`, `cargo test --workspace`, `cargo run -p astrbot-cli`, `cargo clippy --workspace -- -D warnings`.
