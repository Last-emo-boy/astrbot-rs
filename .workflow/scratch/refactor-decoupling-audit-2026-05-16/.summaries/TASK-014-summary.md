# TASK-014 Provider HTTP Helper Boundary

`TASK-014` is complete. Provider adapters now share a focused HTTP helper tree for base URL joining, client/header construction, bearer/API-key auth helpers, custom header insertion, and common JSON error extraction.

Implementation modules:

- `http.rs`
- `http/auth.rs`
- `http/client.rs`
- `http/error.rs`
- `http/url.rs`

Adapters updated:

- Chat: `openai_compatible.rs`, `anthropic.rs`, `gemini.rs`
- Embedding: `openai_embedding.rs`
- STT: `openai_stt.rs`, `xinference_stt.rs`
- TTS: `openai_tts.rs`
- Rerank: `vllm_rerank.rs`, `xinference_rerank.rs`

AstrBot comparison:

- Mirrors the repeated API-base, bearer/header, and structured error patterns visible across `E:/Playground/Astrbot/astrbot/core/provider/sources/*`.
- Keeps provider-specific payload construction and response parsing inside concrete adapters, so capability/family boundaries remain explicit.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
