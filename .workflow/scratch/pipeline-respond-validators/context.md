# M4-T2 Respond Validators

## AstrBot Reference

`RespondStage` in AstrBot validates the result chain before sending: no result returns early, empty chains are skipped, empty `Plain` components are removed, and chains made only of `Reply`/`At` headers are not sent. Streaming result delivery and segmented reply remain separate behavior.

## Rust Boundary

- `astrbot-core::MessageComponent` now distinguishes content-bearing components from header components.
- `MessageChain::into_sendable()` removes empty plain/media components and returns `None` when no content-bearing component remains.
- `RespondStage` only takes the result, normalizes the chain, sends it if sendable, and preserves `Stop` control.

## Deferred

- Streaming result markers and streaming transport delivery.
- Segmented reply timing and record-only separate sends.
- Path mapping and platform-specific send transforms.
