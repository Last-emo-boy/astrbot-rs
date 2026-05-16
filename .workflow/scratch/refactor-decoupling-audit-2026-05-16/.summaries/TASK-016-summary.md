# TASK-016 Summary - Provider Test Support Boundary

Status: completed

Provider integration tests now use shared support modules for local HTTP fixtures and request capture:

- `tests/support/http_server.rs` owns `serve_once`, `serve_sequence`, `TestResponse`, request body capture, content-length handling, and response writing.
- `tests/support/captured_request.rs` owns common header assertions.
- Adapter tests for Anthropic, Gemini, embeddings, rerank, STT, TTS, OpenAI-compatible chat, Xinference, and provider registry now import shared support instead of defining local socket helpers.

AstrBot comparison:

- AstrBot provider sources repeat provider-specific request/response patterns across adapters; the Rust port keeps protocol behavior inside each adapter while centralizing only test harness mechanics.
- This keeps parity tests focused on provider behavior and prevents each new provider from copying local `TcpListener` fixtures.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
