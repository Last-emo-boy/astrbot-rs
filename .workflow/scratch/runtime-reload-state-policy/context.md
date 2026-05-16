# M6-T2 Runtime Reload State Policy

## AstrBot Reference

AstrBot terminates provider/platform managers during restart, but session-scoped provider choice is stored through shared preferences in `ProviderManager.set_provider(...)`. That means provider instances are rebuilt while user/session preference can survive.

## Rust Boundary

- `RuntimeStatePolicyConfig` makes restart state behavior explicit.
- Default policy preserves runtime-owned provider preference across `RuntimeHandle::restart`.
- Setting `preserve_provider_preference_on_restart = false` discards the in-memory preference snapshot and falls back to configured default provider routing.
- The state transfer happens in runtime only: pipeline still depends only on `ProviderPreferencePort`.

## Deferred

- Persistent provider preference storage across process reboot.
- Stale provider ID cleanup when a preserved preference points to a removed provider.
- Reload policies for future session context, plugin state, WebChat queues, and streaming buffers.
