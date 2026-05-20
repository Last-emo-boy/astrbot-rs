# M9 Completion Audit

Date: 2026-05-20T12:33:52+08:00

## Objective

Complete all Maestro-tracked M9 parity-closure tasks in `.workflow/scratch/parity-gap-roadmap-2026-05-20/plan.json` and verify the workspace before marking the active goal complete.

## Prompt-To-Artifact Checklist

| Requirement | Evidence |
| --- | --- |
| M9 task manifest has 36 total tasks and no pending/blocked tasks. | `plan.json`, `index.json`, and `.workflow/state.json` updated to completed, 36/36 done, 0 pending, 0 blocked. |
| `TASK-P1-001` builtin_stars admin / alter_cmd / setunset / sid. | `crates/astrbot-plugin-sdk/src/builtin_commands.rs`; `examples/builtin-stars/admin`, `alter_cmd`, `setunset`, `sid`; `cargo test -p astrbot-plugin-sdk` passed; all builtin-star examples passed `cargo check --target wasm32-wasip1`. |
| `TASK-P1-002` builtin_stars conversation / persona / llm. | `examples/builtin-stars/conversation`, `persona`, `llm`; SDK tests `conversation_routes_common_commands` and `persona_and_llm_use_admin_flow`; wasm example checks passed. |
| `TASK-P1-003` builtin_stars web_searcher. | `examples/builtin-stars/web_searcher`; SDK test `websearch_formats_engine_results_with_links_and_timestamps`; wasm example check passed. |
| `TASK-P1-004` builtin_stars plugin / provider / help / tts / t2i. | `examples/builtin-stars/plugin`, `provider`, `help`, `tts`, `t2i`; SDK tests `plugin_aliases_cover_management_actions`, `help_lists_full_builtin_surface`, `tts_and_t2i_cover_one_shot_and_template_commands`; wasm checks passed. |
| `TASK-P1-005` builtin_stars session_controller. | `examples/builtin-stars/session_controller`; SDK test `session_controller_validates_sleep_wake_and_rate`; wasm check passed. |
| `TASK-P1-006` Long-term Memory depth. | `crates/astrbot-memory/src/long_term.rs`, `active_reply.rs`; `cargo test -p astrbot-memory` passed, including 100-record compression, key fact retention, image caption recording, and active reply policy tests. |
| `TASK-P1-007` concrete T2I rendering. | `crates/astrbot-render/src/local.rs`, `template.rs`; `crates/astrbot-agent/src/t2i.rs`; `cargo test -p astrbot-render` passed, including PNG/JPEG artifact generation for builtin templates and Agent tool output. Python reference has no fixed image baseline for the three new Rust templates, so the runnable gate is deterministic image decode/artifact verification rather than a fabricated pixel-diff baseline. |
| `TASK-P1-010` Dashboard Knowledge upload/chunk/reindex/highlight/SSE. | `dashboard-next/src/pages/knowledge/index.tsx`; `dashboard-next/src/features/uploads/chunkedUpload.ts`; `crates/astrbot-web/src/management/knowledge_base.rs`; SSE route `/api/management/kb/upload/progress/{task_id}/stream`; `cargo test -p astrbot-web management_knowledge_base_routes_delegate_to_typed_services`, `npm run typecheck`, and `npm run build` passed. |
| `TASK-P1-011` Dashboard Persona folder tree and drag-drop. | `dashboard-next/src/pages/persona/index.tsx`; `dashboard-next/src/features/folder-tree/FolderTree.tsx`; `@thisbeyond/solid-dnd` dependency; breadcrumb, context menu, depth guard, localStorage selection persistence; `npm run typecheck` and `npm run build` passed. |
| `TASK-P2-002` Computer Sandbox browser tool. | `crates/astrbot-computer/src/tools/browser.rs`; runtime exports in `crates/astrbot-computer/src/lib.rs`; `cargo test -p astrbot-computer` passed, including screenshot/text extraction, single/batch invocation, and crash recovery tests. |
| Workspace remains green after the final fix. | `cargo test --workspace --all-targets` passed. |
| Frontend remains green after Dashboard changes. | `dashboard-next`: `npm run typecheck` and `npm run build` passed; Vite reported only existing large chunk warnings. |

## Commands Run

- `cargo test -p astrbot-cli cli_webchat_server_submits_events_to_runtime -- --nocapture`
- `cargo test --workspace --all-targets`
- `npm run typecheck` in `dashboard-next`
- `npm run build` in `dashboard-next`
- `cargo check --target wasm32-wasip1 --manifest-path examples/builtin-stars/*/Cargo.toml`
- `cargo test -p astrbot-plugin-sdk`
- `cargo test -p astrbot-memory`
- `cargo test -p astrbot-render`
- `cargo test -p astrbot-computer`
- `cargo test -p astrbot-web management_knowledge_base_routes_delegate_to_typed_services`

## Final Verdict

Completion audit passed. The M9 plan is closed at 36/36 tasks completed, no blocked tasks, and all available automated verification gates are green.
