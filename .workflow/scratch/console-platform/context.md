# Console Platform Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/platform/platform.py`
  - A platform adapter owns external input, converts it to a message event and commits it to the event queue.
  - Reply delivery belongs to the event-specific sink/send path.
- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py`
  - WebChat receives user input from a queue, converts it to a platform message event and commits it.
  - The adapter is registered by platform type and constructed by the platform manager.

## Rust Increment

- Added `CONSOLE_PLATFORM_TYPE`.
- Added `ConsolePlatform`:
  - reads stdin lines in `run()`
  - parses `sender: message` or defaults to `console-user`
  - emits `MessageEvent` through the shared event sender
- Added `ConsoleSink`:
  - prints outbound replies to stdout
  - records sent messages for tests and runtime introspection
- Added `MessageRecorder` trait so mock and console sinks can both expose sent-message history.
- Registered `console` in `PlatformRegistry::with_builtin_platforms()`.
- Added `PlatformConfig::console()` and `RuntimePlatformConfig::console()`.

## Result

- Platform tests cover console registration, manager construction, input parsing and output recording.
- Runtime test covers constructing a console platform from runtime config.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Promote console platform into the default generated config only if local interactive CLI becomes a formal product target.
- Start an HTTP/WebChat-style platform adapter once web/dashboard boundaries are ready.
