# TASK-078 Summary

Completed at: 2026-05-17T14:15:49+08:00

## Scope

Defined provider response metadata boundaries for token usage, reasoning, raw response identity, finish/stop data, tool calls, and stream events.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/entities.py`
- `E:/Playground/Astrbot/astrbot/core/provider/sources/openai_source.py`
- `E:/Playground/Astrbot/astrbot/core/provider/sources/gemini_source.py`
- `E:/Playground/Astrbot/astrbot/core/provider/sources/anthropic_source.py`
- `E:/Playground/Astrbot/astrbot/core/agent/response.py`

## Changes

- Added `astrbot-provider/src/response/` facade modules for `ProviderResponseMetadata`, `ProviderTokenUsage`, `ProviderReasoningMetadata`, `ProviderToolCall`, raw response, and stream events.
- Extended `ChatResponse` with a metadata facade while preserving `chain` as the existing visible response surface.
- Updated OpenAI-compatible, Gemini, and Anthropic response parsers to normalize usage, reasoning, finish/stop reasons, raw payload identity, and tool calls outside concrete adapters.
- Updated `ChatAgentRunner` to forward provider reasoning through `AgentDoneEvent` without depending on concrete provider protocol DTOs.
- Added tests covering metadata extraction for OpenAI-compatible, Gemini, and Anthropic responses plus agent reasoning forwarding.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-provider`
- `cargo test -p astrbot-agent`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-079`.
