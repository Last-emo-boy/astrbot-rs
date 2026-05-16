# M6-T3 Manager Termination Hooks

## AstrBot Reference

AstrBot `stop()` cancels running tasks, shuts down cron, terminates plugins, then terminates provider/platform managers. This keeps lifecycle cleanup in the core runtime instead of scattering resource release across CLI or adapters.

## Rust Boundary

- `ChatProvider`, `PlatformAdapter`, and `PluginHandler` now expose default no-op `terminate()` hooks.
- `ProviderManager::terminate`, `PlatformManager::terminate`, and `PluginRegistry::terminate` call those hooks for configured instances.
- `RuntimeHandle::stop` now stops platform and event bus tasks before manager termination.
- Hooks are currently no-op for built-ins, but real providers/platforms/plugins can override them without changing runtime stop/restart orchestration.

## Deferred

- Rich terminate error aggregation instead of fail-fast.
- Provider/platform hot reload that terminates only changed instances.
- Plugin lifecycle loading/unloading beyond static command handlers.
