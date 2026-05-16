# M4-T3 Streaming Response Boundary

## AstrBot Reference

AstrBot's `RespondStage` handles streaming as a separate send path:

- `STREAMING_RESULT` calls `event.send_streaming(result.async_stream, realtime_segmenting)`.
- `STREAMING_FINISH` marks `_streaming_finished` and returns without sending the final chain again.
- If `_streaming_finished` is already set, later duplicate results are skipped.

## Rust Boundary

- `MessageStream` is the event-level streaming payload boundary.
- `MessageSink::send_streaming` defaults to chunk fallback through `send`, while `RecordingSink` records streaming sends separately for tests.
- `MessageEvent` owns `streaming_finished`, so `RespondStage` can prevent duplicate sends without platform-specific extras.
- `RespondStage` now has independent branches for streaming result, streaming finish, and regular result-chain sending.

## Deferred

- True async stream transport from provider to platform.
- WebChat SSE/WebSocket back queue.
- Realtime segmenting fallback policy.
- Streaming history persistence and final response reconciliation.
