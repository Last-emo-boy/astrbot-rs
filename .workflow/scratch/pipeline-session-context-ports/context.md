# M5-T1 Session Context Ports

## AstrBot Reference

AstrBot builds agent/provider requests using conversation manager state and passes provider `contexts` into `ProviderRequest`/`Provider.text_chat(...)`. The Rust port should keep that concept without coupling `astrbot-pipeline` to a concrete conversation manager.

## Rust Boundary

- `SessionContextPort` is a `PipelineContext` trait port returning typed `ProviderContextMessage` values for a `MessageEvent`.
- `EmptySessionContextPort` is the default, preserving existing behavior.
- `run_provider_fallback` enriches the event-level `ProviderRequest` with session context before converting to `ChatRequest`.
- Session context is prepended to existing request contexts so stored history stays before request-local context.

## Deferred

- Session-specific provider preference storage.
- Quote/reply context extraction.
- Persistent conversation storage and history writeback.
- Context compression/truncation policy.
