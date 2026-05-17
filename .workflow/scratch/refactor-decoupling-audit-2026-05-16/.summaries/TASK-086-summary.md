# TASK-086 Summary

Completed at: 2026-05-17T16:30:58+08:00

## Scope

Introduced a metrics and usage accounting boundary distinct from observability logs/traces, provider protocol parsers, platform adapters, storage table helpers, and dashboard stat routes.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/utils/metrics.py`
- `E:/Playground/Astrbot/astrbot/dashboard/routes/stat.py`
- `E:/Playground/Astrbot/astrbot/core/agent/runners/tool_loop_agent_runner.py`
- `E:/Playground/Astrbot/astrbot/core/astr_agent_run_util.py`
- `E:/Playground/Astrbot/astrbot/core/db/po.py`

## Changes

- Added `astrbot-metrics` as a workspace crate.
- Added `MetricEvent` and `MetricTtsStats` for platform message, LLM response, TTS playback, and custom metric events.
- Added `UsageRecord`, `TokenPrice`, `UsageCharge`, and `UsageAccountant` to bridge provider token usage into accounting without provider/storage coupling.
- Added `MetricSink`, `NoopMetricSink`, `InMemoryMetricSink`, `FanoutMetricSink`, and `LocalPlatformStatsSink`.
- Added `InstallationIdentity`, `MetricRedactionPolicy`, `MetricUploadPayload`, `RemoteMetricUploader`, and `RemoteMetricSink` to model optional remote upload without leaking conversation/session text or ids.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-metrics`
- `cargo test -p astrbot-storage`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-087`.
