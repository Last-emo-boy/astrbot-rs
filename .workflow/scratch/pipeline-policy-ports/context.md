# Pipeline Policy Ports

## AstrBot Reference

AstrBot runs these policy stages after `WakingCheckStage`:

- `WhitelistCheckStage`: optional ID whitelist; bypasses WebChat; may ignore admins by message type; stops event when unified session/group ID is not allowed.
- `SessionStatusCheckStage`: asks `SessionServiceManager` whether the unified session is enabled; stops disabled sessions and ensures a conversation exists.
- `RateLimitStage`: fixed-window per-session limiter; strategy is `stall` or `discard`.

## Rust Boundary

M2-T2 should migrate the policy boundary without importing AstrBot's concrete managers:

- `WhitelistPolicyConfig` remains pure typed config.
- `SessionStatusPort` is a trait object on `PipelineContext`, with a default allow-all implementation.
- `RateLimitConfig` is typed config; first implementation owns in-memory fixed-window counters inside the stage.

## Deferred

- Persistent session status storage.
- Conversation bootstrap side effects from `SessionStatusCheckStage`.
- Admin role and platform-specific unified origin mapping beyond current session/sender fields.
- Dashboard/runtime APIs to mutate these policies.

## Result

- Added `WhitelistPolicyConfig`, `SessionStatusPort`, `RateLimitConfig`, and `RateLimitStrategy` to `PipelineContext`.
- Added `WhitelistCheckStage`, `SessionStatusCheckStage`, and `RateLimitStage`.
- Default built-in order is now `wake -> whitelist -> session_status -> rate_limit -> plugin -> provider -> respond`.
- Runtime config now wires typed whitelist/session/rate-limit policy into `PipelineContext`.
- `cargo test -p astrbot-pipeline` and `cargo test -p astrbot-runtime` passed after implementation.
