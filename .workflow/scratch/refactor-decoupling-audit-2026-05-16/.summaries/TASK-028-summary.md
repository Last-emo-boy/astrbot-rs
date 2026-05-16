# TASK-028 Summary - Agent Context Boundary

## Outcome

Introduced an agent-owned context window boundary in `astrbot-agent`, keeping token counting, context budget, truncation, and compression decisions out of pipeline stages and provider adapters.

New modules:

- `context/window.rs`: typed `AgentContextWindow` wrapper around provider context messages.
- `context/token.rs`: `AgentTokenCounter`, `ApproximateTokenCounter`, and `ContextTokenBudget`.
- `context/truncation.rs`: recent-message and token-budget truncation policy that preserves leading system context.
- `context/compression.rs`: `AgentContextCompressor` trait plus `NoopContextCompressor` placeholder.
- `context/manager.rs`: `ContextWindowManager` orchestration for count, compress, and truncate.
- `context/decorator.rs`: `ContextWindowRequestDecorator` for integration with existing request decorator chains.

## Integration

- `astrbot-agent` now re-exports the context boundary from its crate facade.
- Existing `ChatAgentRunner` and pipeline provider fallback behavior are unchanged.
- Context processing can be composed into `CompositeProviderRequestDecorator` without growing `ProcessStage`, `ProviderRequest`, or concrete provider adapters.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/agent/context/manager.py`
- `E:/Playground/Astrbot/astrbot/core/agent/context/compressor.py`
- `E:/Playground/Astrbot/astrbot/core/agent/context/truncator.py`
- `E:/Playground/Astrbot/astrbot/core/agent/context/token_counter.py`

Rust keeps AstrBot's explicit context manager/token counter/truncator/compressor separation, but expresses it through traits and typed policies. LLM summary compression remains a later integration behind `AgentContextCompressor`.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-pipeline`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
