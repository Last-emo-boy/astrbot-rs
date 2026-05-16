# M6-T1 Runtime Default Pipeline Assembly

## AstrBot Reference

AstrBot keeps lifecycle assembly in `AstrBotCoreLifecycle`: provider manager, platform manager, plugin context, pipeline scheduler, and shared preference/session state are assembled centrally instead of being built by CLI or transport code.

## Rust Boundary

- `AstrbotRuntime::initialize` still delegates stage construction to `DefaultPipelineBuilder`, preserving the pipeline-owned default order.
- Runtime now owns an `InMemoryProviderPreferencePort` as a concrete assembly detail and passes it to `PipelineContext` through the `ProviderPreferencePort` trait.
- `AstrbotRuntime::provider_preference()` exposes the preference store for future dashboard/session APIs without making pipeline depend on runtime storage.
- The focused test uses two mock providers and proves session preference reaches `ProviderManager` routing through the default runtime pipeline.

## Deferred

- Persistent provider preference storage.
- Dashboard/API commands for setting preferences.
- Session context storage and compression policy.
- Reload preservation/discard policy for runtime-owned preference state.
