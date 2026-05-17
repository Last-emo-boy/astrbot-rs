# TASK-084 Summary

Completed at: 2026-05-17T15:54:47+08:00

## Scope

Introduced a shared network boundary for generic downloads, TLS/proxy policy, progress reporting, file/temp destinations, and cache key planning.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/utils/io.py`
- `E:/Playground/Astrbot/astrbot/core/utils/http_ssl.py`
- `E:/Playground/Astrbot/astrbot/core/utils/t2i/network_strategy.py`

## Changes

- Added `astrbot-net` with `download`, `tls`, `progress`, and `cache` modules.
- Added typed `DownloadRequest`, `DownloadDestination`, `DownloadResponse`, `DownloadService`, and `ReqwestDownloadService`.
- Added `HttpClientPolicy` and `TlsVerificationPolicy` for timeout, proxy/trust-env, verified TLS, and insecure fallback decisions.
- Added `DownloadProgressSnapshot` and progress sink events for future dashboard update/plugin/KB flows.
- Added cache key/policy DTOs with path traversal-safe cache paths.
- Moved `astrbot-media` remote media transfer onto `astrbot-net` while preserving media-specific resolver and MIME logic.
- Moved provider HTTP client construction through `HttpClientPolicy` so provider adapters share timeout/proxy/TLS policy construction.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-net`
- `cargo test -p astrbot-media`
- `cargo test -p astrbot-provider`
- `cargo test -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-085`.
