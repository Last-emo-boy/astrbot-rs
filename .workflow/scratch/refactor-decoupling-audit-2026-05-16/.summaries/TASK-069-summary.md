# TASK-069 Summary

Completed at: 2026-05-17T11:43:31+08:00

## Scope

Separated AstrBot-inspired skill package lifecycle concerns from agent prompt composition and Web management transport.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/skills/skill_manager.py`
- `E:/Playground/Astrbot/astrbot/dashboard/routes/skills.py`
- `E:/Playground/Astrbot/astrbot/core/astr_main_agent_resources.py`

## Changes

- Added explicit `astrbot-skill` boundaries for catalog descriptors, activation config, sandbox cache status, frontmatter parsing, package install/delete plans, zip entry validation, and prompt inventory rendering.
- Added prompt sanitizers mirroring AstrBot's SKILL.md path/name/description hardening, including sandbox path rendering.
- Added `SkillPromptInventoryRequestDecorator` so agent persona prompt composition can append active skill inventory without owning install/delete policy.
- Added `ManagementSkillState` and `/api/management/skills*` routes as thin DTO adapters for catalog, activation, install-plan, and delete-plan operations.
- Blocked sandbox-only skills from local activation/delete operations at the skill crate and Web boundary.

## Verification

- `cargo test -p astrbot-skill`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-plugin`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`

## Next

Next pending task is `TASK-070`.
