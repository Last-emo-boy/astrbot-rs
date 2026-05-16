# M5-T2 Provider Preference Storage

## AstrBot Reference

AstrBot resolves the active provider through `ProviderManager.get_using_provider(umo)`, using session-scoped preferences from shared preferences before falling back to configured defaults. Dashboard/session management can update those preferences independently of the pipeline.

## Rust Boundary

- `ProviderPreferencePort` returns the preferred chat provider ID for a `MessageEvent`.
- `InMemoryProviderPreferencePort` stores session-scoped preferences for tests and future runtime wiring.
- `run_provider_fallback` applies session preference only when the event-level `ProviderRequest` does not already specify a provider.
- `ProviderManager` implements `ChatProvider`, routing `ChatRequest.provider_id` to the selected provider and otherwise using the default provider.
- Runtime now gives the default pipeline the provider manager facade instead of a single default provider instance.

## Deferred

- Dashboard/API commands for setting preferences.
- Persistent shared-preference storage.
- STT/TTS provider preference ports.
- Validation hooks that proactively clear stale provider IDs.
