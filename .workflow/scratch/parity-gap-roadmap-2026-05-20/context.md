# Parity Gap Roadmap — astrbot-rs 对齐 Python Astrbot

## 1. 背景

`E:/Playground/astrbot-rs/` 是对 `E:/Playground/Astrbot/`（Python + Vue3）的 Rust 重写。
M1–M6（管道内核 + 提供器 + 配置 + 运行时装配）已基本对齐；M7（平台 / 提供器 / 仪表板宽度）进行中；
M8（Dashboard 重写为 Solid + Vite，骨架已落地）刚结束。本计划锁定**剩余功能差距**，按 P0/P1/P2 推进。

## 2. 原版盘点（survey 摘要）

- 16 个平台适配器（aiocqhttp/qq*/wecom*/lark/dingtalk/discord/slack/kook/misskey/satori/telegram/webchat/weixin_official_account/line）
- 20+ LLM Provider（chat/STT/TTS/embedding/rerank/image-gen）
- 49 个 Vue3 dashboard 页面 (~55K LOC)
- 25+ pipeline 阶段（含 content_safety、whitelist、rate_limit、waking_check、session_status、result_decorate 等多策略子目录）
- Computer Sandbox：browser / shell / python / fs + bay_manager(Docker) / shipyard_neo / boxlite / local
- builtin_stars：admin / alter_cmd / conversation / help / llm / persona / plugin / provider / setunset / sid / t2i / tts / web_searcher / session_controller
- 三层能力：Skills（多步工作流）/ Tools（Agent 工具）/ Stars（轻量命令插件）
- MCP：stdio + http 双传输，工具/资源/提示桥接
- 向量库：FAISS 本地；长期记忆自研（群消息 + 图像描述 + LLM 压缩）
- T2I：模板 + Pillow/Playwright 真正出图
- 第三方 Agent 代理：Coze / DashScope / Dify / Deerflow

## 3. Rust 复刻度（综合评估）

| 域 | 复刻度 | 现状 |
|---|---|---|
| 管道内核 | 95% | M1–M6 完成 |
| 配置 / 持久化 | 90% | SQLite + 备份/迁移完整 |
| KB / RAG | 90% | Hybrid + Rerank；缺 PDF/Markdown Parser、FAISS 后端 |
| LLM Provider | 85% | 主流商家齐全；媒体转换/OpenAPI 流式 70% |
| 运维 / 可观测 | 70% | 日志/Tracing/Audit 框架；缺 Prometheus exporter |
| 插件 / 工具 | 60% | Registry + 市场骨架；**动态加载 0%** |
| MCP | 40% | 配置 + 桥接；**运输层 0%** |
| 平台适配 | 30% | OneBot 全功能；17 个 Wave1 仅占位 |
| Dashboard 前端 | 20% | 26 页骨架 + Card+Table+JSON-Modal；缺富消息、Schema 表单、图表 |
| Computer / Sandbox | 10% | crate 存在；**核心能力 0%** |
| 第三方 Agent | 0% | 未启动 |

综合 **~50–55%**。

## 4. 关键架构决策（用户已确认 2026-05-20）

1. **Plugin Sandbox**: 选用 **wasmtime**（Bytecode Alliance，主流，WASI 1/2、Component Model、AOT 编译）
2. **Sandbox 职责边界**: WASM **只管插件**；Computer Sandbox 走 **OS 子进程 + 权限限制**（不混在 WASM 里）
3. **Wave1 P0 平台**: **Telegram + 公众号 + 企业微信 + QQ 官方 Webhook + QQ 频道（WSS）**（"个人微信"原版不存在，移到 P2 backlog）
4. **Dashboard 图表库**: **Apache ECharts**（Observability 仪表盘需要）
5. **迭代组织**: 单一 plan，按 P0/P1/P2 标签

## 5. 验证基线

- 后端：`cargo check --workspace --all-targets`、`cargo test --workspace`
- 前端：`(cd dashboard-next && npx tsc --noEmit && npx vite build)`
- DTO 漂移：`cargo test -p astrbot-web export_bindings && git diff --exit-code dashboard-next/src/api/dto/`
- 平台冒烟：`cargo test -p astrbot-platform <adapter>`
- Plugin Sandbox：WASM 加载 → invoke → 卸载循环 + 资源限制断言

## 6. 不在范围

- iOS/Android 客户端（原版也没有）
- 与第三方训练平台（HuggingFace 等）的训练侧集成
- Mem0 SaaS 集成（如要做，单独 RFC）

## 7. 引用

- `E:/Playground/Astrbot/astrbot/core/platform/sources/`（16 适配器）
- `E:/Playground/Astrbot/astrbot/core/provider/sources/`（20+ Provider）
- `E:/Playground/Astrbot/astrbot/builtin_stars/builtin_commands/`（13 内置星标）
- `E:/Playground/Astrbot/astrbot/core/agent/mcp_client.py`、`mcp_stdio_client.py`
- `E:/Playground/Astrbot/astrbot/core/computer/`（沙箱执行）
- `E:/Playground/astrbot-rs/crates/astrbot-plugin/`（待加 WASM loader）
- `E:/Playground/astrbot-rs/crates/astrbot-platform/src/adapters/wave1/`（17 占位）
- `E:/Playground/astrbot-rs/dashboard-next/src/pages/`（26 页骨架）
- `E:/Playground/astrbot-rs/.workflow/roadmap.md`（M7-R/M8 backlog）
