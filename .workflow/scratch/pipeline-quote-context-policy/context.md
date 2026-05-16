# M5-T3 Quote Context Policy

## AstrBot Reference

AstrBot decorates provider requests with quoted message text in `astr_main_agent.py` by adding a `<Quoted Message>...</Quoted Message>` text part to `ProviderRequest.extra_user_content_parts`. WebChat reply parts can carry `selected_text`; richer platforms may provide embedded reply chains or fetchable reply IDs.

## Rust Boundary

- `QuoteContextPolicy` is a pipeline trait so quote extraction can evolve without coupling `astrbot-pipeline` to WebChat history, platform adapters, or storage.
- `SelectedTextQuoteContextPolicy` is the default policy and only consumes non-empty `MessageComponent::Reply.selected_text`.
- `NoQuoteContextPolicy` allows runtime or tests to disable quote enrichment without changing provider fallback behavior.
- `run_provider_fallback` injects quote content before converting event-level `ProviderRequest` into provider crate `ChatRequest`.
- Reply-only messages still do not count as provider user content; quote context enriches an already valid request but does not trigger fallback by itself.

## Deferred

- Fetching quoted messages by reply ID.
- Embedded reply chain traversal beyond current `selected_text`.
- Quoted image/file extraction and image captioning.
- Platform-specific sender nickname formatting.
- Persistent conversation history and context compression policy.
