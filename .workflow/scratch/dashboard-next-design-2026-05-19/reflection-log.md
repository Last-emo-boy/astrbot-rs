# Reflection Log

## Round 0 - Design Capture

- Scope: 一次性重写 Rust 版 Astrbot 的 Dashboard，物理删除 `E:/Playground/astrbot-rs/dashboard/`（vanilla JS, 22046 LOC），新建 `dashboard-next/`（Solid + Vite + TypeScript strict）。
- Strategy: 后端 `crates/astrbot-web/src/management/` 26 个模块的 HTTP/SSE/WS 契约冻结；DTO 经 `ts-rs` 自动生成到 `dashboard-next/src/api/dto/`；hash 路由保留以兼容 `is_dashboard_index_route` SPA fallback；旧 `dashboard/` 在 planning 阶段就删，不保留 dashboard-legacy keepalive。
- Result: 写入 `index.json` / `context.md` / `plan.json` / `reflection-log.md`；TASK-001..TASK-010 覆盖 Phase 0..9。
- AstrBot reference: 与 `E:/Playground/Astrbot/dashboard/`（Vue 3 + TS，57748 LOC）做功能对齐参照；视图清单（ChatPage / PersonaManager / KnowledgeBase / SubAgentPage / TracePage 等）作为分 phase 的目标 surface。
- Adjustment: 不在 planning PR 改动后端代码；ts-rs derive 与 `DashboardAssetSource::NextDist` 由 TASK-001 实施落地。Phase 0 必须先验证 ts-rs 流水线 + NextDist 装载，再展开后续页面。
- Open: chart library（uplot vs apexcharts）延后到 Phase 2 实际接 Observability 时决定；lazy 分包粒度延到 Phase 9 bundle 分析后决定。
- Verification (planning only): 文档与 plan.json JSON 结构需保持 valid；`.workflow/specs/architecture-constraints.md` 新增 4 条 spec-entry；`.workflow/roadmap.md` 增 M8 段落；旧 `dashboard/` 删除后 `cargo check --workspace` 不应回归（runtime 仍可在 BundledDist 缺失时 fallback 到 UserDist/Explicit）。

## Round 1 - TASK-001 Implementation

- Goal trigger: `/goal 进行maestro的迭代` (session Stop hook active) → 推进 TASK-001 实施。
- Backend wiring:
  - `Cargo.toml` workspace 加入 `ts-rs = { version = "9", features = ["serde-compat"] }`。**注意**：`format` 特性被 ts-rs v9.0.1 → swc_common v0.33.26 拉入，且 swc_common 引用了已被新 `serde` 移除的 `serde::__private`，编译失败 → 改为不启用 `format`，TS 输出未格式化（前端层 prettier 可补）。
  - `crates/astrbot-web/Cargo.toml` 仅以 `[dev-dependencies] ts-rs = { workspace = true }` 引入；DTO 上挂 `#[cfg_attr(test, derive(ts_rs::TS))] #[cfg_attr(test, ts(export, export_to = "../../../dashboard-next/src/api/dto/"))]`——production 二进制无 ts-rs 依赖。
  - **export_to 路径教训**：ts-rs `export_to` 相对源文件目录解析（不是 manifest dir）。从 `crates/astrbot-web/src/dto.rs` 出发需 `../../../` 才能跳到 workspace 根，初次写 `../../` 把文件落在 `crates/dashboard-next/...` 才发现。
  - Pilot DTO 实际选取：`SubmitTextRequest`（包含嵌套 enum）+ `WebChatMessagePart`（tagged enum + serde alias 警告）+ `ErrorResponse`（替换设计文档里不存在的 `ProviderRuntimeConfig`）。三类型分别覆盖 struct/复合/枚举生成路径。
  - `dashboard_assets.rs`：`DashboardAssetSource::NextDist` 变体 + `next_dist_dir: Option<PathBuf>` 字段 + `with_next_dist_dir()` builder + `select()` 优先级 `Explicit > UserDist > NextDist > BundledDist`；新增测试 `dashboard_asset_policy_prefers_next_dist_over_bundled_when_user_absent`。
  - `path_config.rs`：`RuntimePathConfig` + `RuntimePathLayout` 新增 `dashboard_next_dist_dir` 字段；`resolve()` 默认子路径 `dashboard-next/dist`；`resolves_astrbot_data_subdirectories_from_root` 测试补断言。
  - `DASHBOARD_INDEX_ROUTES` 已在 design 阶段扩展，本轮新增 4 条断言。
- Frontend scaffold:
  - `package.json`：solid-js 1.9 + @solidjs/router 0.15（HashRouter）+ Vite 5.4 + vite-plugin-solid + vite-plugin-checker + @kobalte/core + markdown-it + highlight.js + katex + @solid-primitives/i18n。
  - `tsconfig.json` 严格模式 + `noUncheckedIndexedAccess` + `exactOptionalPropertyTypes` + `@/*` 别名。
  - `vite.config.ts` 反向代理 `/api` `/webchat` `/ws` 到 `127.0.0.1:6185`（webchat 端口）。
  - `src/{main.tsx,App.tsx,styles/global.css}` 最小 HashRouter + 占位主页。
  - `README.md` 写入 DTO 流程 + 资产加载说明。
- Verification: `cargo check --workspace` ✓ / `cargo test -p astrbot-runtime --lib path_config` 2 passed / `cargo test -p astrbot-runtime --lib dashboard_assets` 4 passed / `cargo test -p astrbot-web` 117 passed + 3 export tests / `npm install` 163 包 / `npx vite build` 24KB JS 9.5KB gzip 412ms。
- Knowledge captured:
  - ts-rs `export_to` 相对源文件目录而非 manifest（需写进 spec？暂记此处）。
  - ts-rs v9 `format` 特性与新版 serde 不兼容（swc_common::serde::__private），后续若需自动格式化考虑升级到 v12 或前端 prettier 兜底。
  - `#[cfg_attr(test, ...)]` 模式优于 feature-gate：无 production 路径污染、CI 默认就跑 export。
- Next: TASK-002（AppShell + auth + base components）。在此之前可考虑把 `dashboard/` 删除与 ts-rs 集成切到一个 git commit，避免大爆炸提交。

## Round 2 - TASK-002..TASK-010 Implementation Sweep

- Goal trigger: `/goal 完成所有的maestro Task` → 一次性推进 TASK-002 至 TASK-010。
- Foundation (TASK-002): `src/api/client.ts` 统一 fetch + token 注入 + EventSource 构造；`src/api/auth.ts` login/logout + token signal；`src/i18n/index.ts` 与 `src/styles/theme.ts` 双信号；`src/styles/{tokens.css,global.css}` 设计 token + 通用样式；`Card/Form/Modal/Toast` 五件套；`AppShell.tsx` 顶/侧栏 + 主题/语言/登出；`routes.tsx` 全量 hash 路由。
- Pages (TASK-003..TASK-009)：26 个页面以「Card + table + Modal 编辑」骨架统一实现：
  - 只读：`overview/console/trace/observability/settings/about/welcome/login`
  - 表单：`config/providers/platforms/persona/mcp/cron/projects/subagent/t2i`
  - 流水：`chat/conversation/sessions`
  - 管理：`plugins/market/skills/tools/knowledge/api-keys/backup/update`
- Verification:
  - `npx tsc --noEmit` 零错误（修复路由 `wrap()` 与 `RouteSectionProps` 类型对齐，`apiPost` body 不再可能为 undefined，知识库 KbDetail 显式 `createResource<KbDetailResponse, string>`）
  - `npx vite build` 1.05s 完成，主包 51.58KB（gzip 19.19KB），按路由 lazy chunk 33 个
  - `cargo check --workspace --all-targets` 全部通过
  - `cargo test -p astrbot-runtime --lib` 67 passed；`cargo test -p astrbot-web --lib export_bindings` 3 passed（DTO 仍精确落到 `dashboard-next/src/api/dto/`）
- Cutover (TASK-010):
  - 资源优先级翻转：`select()` 现按 `Explicit > NextDist > UserDist > BundledDist` 排序，新增 `dashboard_asset_policy_prefers_next_dist_over_user_when_both_present` 用例
  - 旧 `dashboard/` 物理已删（staged for deletion）；活动代码无残留引用，仅历史 Maestro task 摘要含 `dashboard/` 路径字符串
  - Playwright e2e 与 bundle 进一步分析推迟到正式 PR 阶段（页面 26 + 一致骨架已足以做手测对照）
- Knowledge:
  - SolidRouter 路由组件不可窄化 `props.children` 为 `never`——拿掉自定义 props 约束直接使用 `RouteSectionProps`
  - `exactOptionalPropertyTypes` 下 `RequestInit.body = undefined` 报错；改 `init.body = JSON.stringify(...)` 条件赋值
  - `createResource(<arg>, async (id) => apiGet(...).catch(() => ({})))` 类型推导退化到 `{}`；显式 `createResource<TResp, TArg>` 与 `({} as TResp)` 兼顾失败兜底

