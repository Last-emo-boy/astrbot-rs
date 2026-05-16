# Pipeline Wake Check Stage

## AstrBot Reference

AstrBot runs `WakingCheckStage` first. The stage:

- applies wake prefix checks before later policy/process stages;
- marks events as wake/at-or-command;
- strips the matched wake prefix from the text message;
- wakes on bot mention, at-all when not ignored, reply-to-bot, and direct chat when direct chat does not require a wake prefix;
- stops the event when no wake condition matches.

The Python stage also prepares plugin handler activation and permission replies. Rust keeps that part out of M2-T1 because plugin/process unification is planned for M3.

## Rust Boundary

M2-T1 should introduce only the reusable low-level boundary:

- `WakeCheckConfig` lives on `PipelineContext`.
- `WakeCheckStage` is a built-in stage ordered before `plugin`, `provider`, and `respond`.
- `MessageEvent` can record wake state without EventBus knowing wake logic.
- `MessageChain` gains minimal mention components so platform adapters can express AstrBot-style `At` / `AtAll` later.

## Verification Target

- Group messages without wake markers stop before provider.
- Group messages with wake prefix or bot mention continue.
- Direct messages remain accepted by default to preserve current WebChat/console/mock behavior.
- Default builder and runtime use the new stage order.

## Result

- Added `WakeCheckConfig` to `PipelineContext`.
- Added direct/group session metadata, wake markers, mention/mention-all components, and reply-to-sender metadata to `astrbot-core`.
- Added built-in `WakeCheckStage` before plugin/provider/respond.
- Runtime config now carries wake policy and passes it through `PipelineContext`.
- `cargo test -p astrbot-pipeline` and `cargo test -p astrbot-runtime` passed after implementation.
