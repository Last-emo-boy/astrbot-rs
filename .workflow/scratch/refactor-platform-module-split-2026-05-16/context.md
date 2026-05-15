# Platform Module Split Refactor

Reference:
- `E:/Playground/Astrbot/astrbot/core/platform/register.py`
- `E:/Playground/Astrbot/astrbot/core/platform/manager.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources`

AstrBot keeps platform adapter discovery and instantiation behind `platform_cls_map` and `PlatformManager.load_platform()`, while concrete implementations live under `platform/sources/*`. The Rust platform crate now follows the same idea with static Rust modules:

- `core.rs`: shared traits, config, constants, recording sink, and ID validation.
- `built.rs`: typed result of a platform factory.
- `registry.rs`: platform type to factory mapping plus built-in registrations.
- `manager.rs`: configured platform ID to running adapter instances and task/termination orchestration.
- `adapters/`: concrete adapters (`mock`, `console`, `webchat`, `onebot`) that should grow into future WeChat, QQ, OneBot transport, and other platform implementations.
- `lib.rs`: public API re-export surface only.

This is a structural refactor only. It intentionally does not add new provider or platform parity.
