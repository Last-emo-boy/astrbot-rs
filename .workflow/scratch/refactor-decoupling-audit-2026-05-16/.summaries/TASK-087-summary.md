# TASK-087 Summary

Completed at: 2026-05-17T16:45:12+08:00

## Scope

Introduced route-independent tool output reference extraction, agent response reference metadata, conversation reference persistence, and WebChat reference serialization boundaries.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/dashboard/routes/chat.py`
- `E:/Playground/Astrbot/astrbot/dashboard/routes/live_chat.py`
- `E:/Playground/Astrbot/astrbot/dashboard/routes/open_api.py`
- `E:/Playground/Astrbot/astrbot/builtin_stars/web_searcher/main.py`

## Changes

- Added `astrbot-tool::ToolReferenceExtractor`, `ToolCallReferencePayload`, `ToolReferenceItem`, and `ToolReferenceSet` for supported `web_search_tavily`/`web_search_bocha` result parsing and `<ref>...</ref>` matching.
- Added `astrbot-agent::AgentReferenceDecorator` and `AgentResponseReferences` so refs can live on agent response metadata instead of dashboard route state.
- Added `ConversationReferenceRepository`, `ConversationReferenceRecord`, and `InMemoryConversationReferenceRepository`.
- Added `WebChatReferenceResponse` and `WebChatReferenceItem` as serialization DTOs that do not perform extraction.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-tool`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-088`.
