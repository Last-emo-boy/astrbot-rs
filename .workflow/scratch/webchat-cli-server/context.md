# WebChat CLI Server Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/initial_loader.py`
  - Starts core lifecycle and dashboard server as sibling long-running tasks.
- `E:/Playground/Astrbot/astrbot/dashboard/server.py`
  - Reads dashboard host/port from config/env and uses a shutdown trigger.
- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py`
  - Keeps WebChat as a platform boundary: queue/listener input becomes platform events, output goes through session send.

## Current Rust State

- `astrbot-platform` already exposes `WebChatPlatform::submit_text()` and keeps the reply sink on the platform object.
- `astrbot-web` already exposes a thin Axum router/server that only calls the WebChat platform boundary.
- `astrbot-runtime` owns EventBus and platform adapter tasks through `RuntimeHandle`.
- `astrbot-cli run` currently starts only runtime and waits for `Ctrl+C`.

## Decision

This increment should keep `astrbot-web` independent from runtime internals. The CLI launcher may start the WebChat transport server as a sibling service, using the `WebChatPlatform` assembled by runtime. This follows AstrBot's launcher pattern without letting HTTP know about pipeline/provider internals.

## Verification

- Config defaults and normalization cover the new webchat server config.
- CLI helper can bind a configured WebChat HTTP server against a runtime-created WebChat platform.
- Workspace fmt, tests, clippy and CLI smoke stay green.

## Execution Notes

- Added `RuntimeWebChatServerConfig` to `RuntimeConfig`, disabled by default with `platform_id=webchat`, `host=127.0.0.1`, `port=6185`.
- `astrbot-cli run` now prepares the HTTP listener before starting runtime, then starts `astrbot-web` with graceful shutdown after runtime starts.
- Added CLI tests for binding the configured server and sending an HTTP WebChat message through the running runtime pipeline.
- Verified with `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo run -p astrbot-cli`.
