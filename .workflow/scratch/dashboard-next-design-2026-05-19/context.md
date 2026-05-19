# Dashboard Next Design

## Intent

用一个新的 `dashboard-next/` 工程（Solid + Vite + TypeScript 严格模式）一次性重写 Rust 版 Astrbot 的 Dashboard 前端，并物理删除现有 `dashboard/`（vanilla JS，22046 LOC）。目标是与 AstrBot Vue 3 Dashboard 功能对齐，同时显著提升可维护性、类型安全与构建产物体积可控性。后端 `crates/astrbot-web/src/management/` 26 个模块的 HTTP/SSE/WS 契约保持不变，前端只通过自动生成的 TypeScript DTO 与之对接。

## Decision Summary

| 维度 | 决策 | 备选 | 理由 |
| --- | --- | --- | --- |
| Goal | 对齐 AstrBot UI 功能并提升可维护性 | 仅修 bug / 仅扩功能 | 现有 vanilla JS dashboard 没有类型系统、render 函数已达千行级，AstrBot Vue 版功能仍领先 |
| Stack | Solid + Vite + TypeScript strict | Vue 3 / SvelteKit / 保留 vanilla | Solid 编译期响应式，bundle 比 Vue 小，签名贴近 React，便于团队迁移；不引入 Next/Nuxt 这类强约束框架 |
| Headless UI | `@kobalte/core` | Radix Vue / 自写 | Solid 生态对应物，覆盖 Dialog/Combobox/Tabs/Popover/Menu/Tooltip 等 |
| Style | CSS variables + CSS Modules | Tailwind / UnoCSS | 旧版已有 6165 行 styles.css 设计 token，承袭其变量名，避免重写视觉系统；不引入原子化 CSS 框架 |
| 类型来源 | 从 Rust DTO 自动生成（`ts-rs`） | `typeshare` / 手写 | `ts-rs` 用 `#[derive(TS)]` + `#[ts(export)]` 落盘 `.ts`，对枚举 tag 与 serde rename 支持好；`typeshare` 偏 Annotation-driven 但对复杂枚举不如 `ts-rs` |
| Charts | `uplot`（轻量）/ `apexcharts`（功能丰富）二选一，Phase 2 决议 | echarts / chart.js | echarts 太重；先用 uPlot 做 token usage 时间线，复杂图按需替换 |
| Markdown | `markdown-it` + `highlight.js` + `katex` | marked / shiki | 保持与 AstrBot 一致，渲染结果可与原 Vue 版互相替换 |
| 代码编辑 | CodeMirror 6 | Monaco | Monaco 引入 webworker 与 ~1.5MB gzipped；CodeMirror 6 模块化、~150KB gzipped，足够 YAML/JSON 配置编辑 |
| 路由 | `@solidjs/router` HashHistory | History API | 保留 `#/...` 形式，沿用 `is_dashboard_index_route` SPA fallback 规则，避免动到后端路由表 |
| i18n | `@solid-primitives/i18n` | i18next | 轻量、与 Solid signal 直接整合；翻译表沿用旧 `src/i18n.js` 提取的 zh/en 词条 |
| 数据获取 | `createResource` + 轻量 SWR 封装 | TanStack Query Solid | 后端接口少且语义清晰，自写 fetcher + revalidate 足够；不引入额外 cache 层 |
| 实时 | 原生 `EventSource` / `WebSocket` 包装为 signal hook | socket.io | 后端用纯 SSE/WS；包装 `createEventStream()` / `createWebSocket()` 供页面消费 |
| 迁移路径 | 一次性重写整个 Dashboard | 渐进迁移 / 双轨并存 | 用户明确要求 "把之前的旧的版本完全删除"，不保留 dashboard-legacy keepalive |

## Evidence

| 项 | 旧版现状 | 来源 |
| --- | --- | --- |
| 旧 dashboard 总规模 | 22046 LOC（HTML 51 + JS 主入口 426 + CSS 6165 + src/* 15404） | E:/Playground/astrbot-rs/dashboard/ |
| 路由覆盖 | 24 个 hash 路由：overview, chat, chatbox, conversation, console, trace, config, providers, platforms, sessions, personas, cron, plugins, market, skills, subagent, tools, knowledge, projects, backup, update, settings, about, login | E:/Playground/astrbot-rs/dashboard/src/routes.js |
| render 层耦合 | render/data 1586、render/integrations 1894、render/operations 1128、render/knowledge 807、render/settings 780 等单文件均超 700 LOC | E:/Playground/astrbot-rs/dashboard/src/render/ |
| actions 层耦合 | actions/core 1568、actions/extensions 1676、actions/personas-cron 597、actions/sessions 393 | E:/Playground/astrbot-rs/dashboard/src/actions/ |
| 后端管理 API | 26 个 management 模块，共 23391 LOC，覆盖 api_key/auth/backup/chat_projects/commands/config/conversations/cron/dashboard/files/knowledge_base/mcp/observability/persona/platforms/plugin_market/plugins/providers/session_rules/skills/status/subagents/t2i_templates/tools/update | E:/Playground/astrbot-rs/crates/astrbot-web/src/management/ |
| AstrBot Vue 版规模 | 57748 LOC，134 个 `.vue` + 48 个 `.ts`，134 视图覆盖 chat / persona folder tree / knowledge-base / alkaid / observability 等 | E:/Playground/Astrbot/dashboard/ |
| Dashboard 资源装载 | `DashboardAssetSource` 枚举 Explicit/UserDist/BundledDist，`DASHBOARD_INDEX_ROUTES` 35 条 hash route | E:/Playground/astrbot-rs/crates/astrbot-runtime/src/dashboard_assets.rs |

## Architecture

### 目录布局

```
dashboard-next/
  package.json          # vite + solid + typescript + kobalte + markdown-it + ...
  tsconfig.json         # strict, noUncheckedIndexedAccess, target ES2022
  vite.config.ts        # vite-plugin-solid + vite-plugin-checker
  index.html
  src/
    main.tsx            # render(App, root)
    App.tsx             # Router + ErrorBoundary + Toast + ThemeProvider + i18nProvider
    routes.tsx          # 路由表 + lazy 边界
    api/
      client.ts         # fetch 封装、auth header、错误归一化
      dto/              # ts-rs 输出目录（生成产物，禁止手改）
        index.ts
        config.ts
        provider.ts
        ...
      hooks/            # createResource / createWebSocket / createEventStream
    components/         # 通用 UI：Button, Input, Toast, Modal, Tabs, DataTable, ...
    layouts/            # AppShell（Sidebar + Topbar + Workspace）/ AuthLayout
    pages/              # 与旧 24 + 4 新路由一一映射
      overview/
      chat/             # ChatPage + ChatBox + ConversationSidebar
      providers/
      platforms/
      ...
    features/           # 跨页面共享业务逻辑：knowledge-base ingestion, persona folder tree, ...
    i18n/
      zh.ts en.ts
    styles/
      tokens.css        # 由旧 styles.css 提炼的 CSS 变量
      reset.css
    utils/
      markdown.ts katex.ts highlight.ts
  tests/
    unit/   playwright/
```

### DTO 自动生成

- 在 `crates/astrbot-web` 给 `dto.rs` 及 `management/*` 的请求/响应结构体增加 `#[derive(ts_rs::TS)] #[ts(export, export_to = "../../dashboard-next/src/api/dto/")]`。
- 提供 `cargo test -p astrbot-web --features ts-rs-export -- --ignored ts_export` 触发导出；CI 增加 drift check：`git diff --exit-code dashboard-next/src/api/dto/`。
- 工作区根 `Cargo.toml` 增 `ts-rs = { version = "9", features = ["serde-compat", "format"] }` 到 workspace.dependencies。
- 涉及枚举（如 `WebChatMessagePart`、provider config）显式标注 `#[serde(tag = "type")]` 保证 TS 侧 discriminated union 行为正确。

### Runtime 资源装载

`crates/astrbot-runtime/src/dashboard_assets.rs`：

- 新增 `DashboardAssetSource::NextDist`，指向 `dashboard-next/dist/`，并在 `RuntimePathLayout` 新增 `dashboard_next_dist_dir`。
- `DASHBOARD_INDEX_ROUTES` 追加 `/mcp`、`/api-keys`、`/observability`、`/t2i-templates`，保持 hash route SPA fallback。
- `BundledDist` 在 Phase 9 切换为 `NextDist`；旧 `BundledDist` 路径在删除老 dashboard 后停止维护。

### 后端契约不变原则

- `crates/astrbot-web/src/management/` 26 个模块的 path/method/body schema 在重写期间冻结。
- 若 Vue 版有 Rust 后端尚未实现的端点（如部分 alkaid 长记忆 API），Phase 8 单列子任务并先以前端 Empty State 占位，不在 dashboard-next 重写过程中改动后端契约。

## Phases

| 阶段 | TASK | 范围 | 关键产物 | 退出条件 |
| --- | --- | --- | --- | --- |
| Phase 0 基建 | TASK-001 | dashboard-next 脚手架；ts-rs 工具链 + 3 个 DTO pilot；NextDist asset source | package.json/tsconfig/vite/main.tsx；3 个枚举 DTO 自动导出；`/welcome` 渲染 hello | `cargo test -p astrbot-web ts_export` 通过；`npm run build` 通过；runtime 能装载 NextDist |
| Phase 1 框架与认证 | TASK-002 | AppShell、Router、i18n、Theme tokens、Toast/Modal/Button/Input、Login | `/login`、`/welcome` 可登录；token 持久化 | 真实后端登录走通；i18n 切换正常 |
| Phase 2 概览与诊断 | TASK-003 | Overview、Trace、Console（SSE）、Settings、About | 5 个只读页面；Console 实时日志 | SSE 不阻塞主线程；trace 列表分页 |
| Phase 3 配置 / Provider / Platform | TASK-004 | Config 树编辑器、Providers、Platforms（schema-driven form） | 配置 CRUD；Provider/Platform 增删改 + 测试调用 | 旧 dashboard 同类操作行为对齐 |
| Phase 4 对话核心 | TASK-005 | Chat、ChatBox、Conversation、MessageList、MessagePartsRenderer、ChatInput、Markdown+KaTeX | 完整 webchat 体验，含 reply/image/file part | 与 AstrBot 主线对话体验一致 |
| Phase 5 扩展生态 | TASK-006 | Plugins（installed + market）、Skills、Tools、SubAgent | 插件安装/更新/启停；技能/工具列表；子代理面板 | 插件市场分页与状态机正确 |
| Phase 6 知识库 | TASK-007 | KnowledgeBase（KBList/KBDetail/DocumentDetail/Upload） | 知识库 CRUD + 文档上传 + 进度 | 上传任务可恢复，错误状态可见 |
| Phase 7 人格 / 定时 / 会话 / 项目 | TASK-008 | Persona（folder tree + drag-drop）、Cron、Sessions、Projects | 人格目录树拖拽；Cron 启停；Sessions 规则；ChatUI Projects | 人格拖拽迁移路径无丢失 |
| Phase 8 运维与新增 | TASK-009 | Backup、Update、MCP、ApiKeys、Observability、T2I Templates | 6 个运维/新增页面 | 新增 4 个路由在 SPA fallback 中可达 |
| Phase 9 收尾 | TASK-010 | Playwright e2e 迁移、bundle 分析、代码分包、删除旧 dashboard 目录、NextDist 设为默认 | 旧 `dashboard/` 不复存在；首屏 < 250KB gzipped | `cargo test --workspace` 通过；e2e green |

## Risks & Mitigations

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 一次性重写期间无可用 Dashboard | 开发者短期无界面调试 | 重写期间保留 `astrbot-web` 的 OpenAPI/CLI，所有操作具备 curl 等价路径；Phase 1 完成即恢复登录 + 概览 |
| DTO 自动生成与运行时序列化漂移 | 前后端语义不一致 | CI 走 `cargo test ts_export` + `git diff --exit-code` 双闸，drift 直接报错 |
| Solid 团队学习曲线 | 推进变慢 | Phase 1 落 4 个通用组件 + 1 个真实页面，作为团队模板；不引入复杂模式（render props/HOC） |
| 后端契约暴露隐藏破坏 | 重写时碰到 schema 异常 | 后端契约冻结期内任何破坏性变更只走 spec-entry 后再合 |
| Hash 路由 SPA fallback 漏路由 | 新页面 404 | Phase 0 同步更新 `DASHBOARD_INDEX_ROUTES`，并加单测 |

## Out of Scope

- 后端 `management/*` 任何业务行为变更（仅 DTO 加 derive 不属于行为变更）。
- 引入 SSR、PWA、移动端原生壳。
- 重新设计视觉系统：沿用旧 styles.css 的色彩/间距/圆角 token。
- 引入 GraphQL / tRPC 等新的传输层。
- 把 alkaid 长记忆等后端缺失能力补齐——只做 UI 占位。

## Open Questions

1. 图表库（`uplot` vs `apexcharts`）将在 Phase 2 Overview / Observability 实际使用时定稿。
2. lazy-load 边界粒度（按页面 / 按模块）将在 Phase 9 bundle 分析后决定。
3. ts-rs export 触发点（cargo test 还是独立 xtask binary）将在 Phase 0 验证后定稿。

## References

- 旧 dashboard：`E:/Playground/astrbot-rs/dashboard/`
- 后端管理模块：`E:/Playground/astrbot-rs/crates/astrbot-web/src/management/`
- runtime asset 装载：`E:/Playground/astrbot-rs/crates/astrbot-runtime/src/dashboard_assets.rs`
- AstrBot Vue 版：`E:/Playground/Astrbot/dashboard/`
- 旧 Roadmap 任务标记：`M7-T3-dashboard-api-and-ui`（本计划升级为 M8）
