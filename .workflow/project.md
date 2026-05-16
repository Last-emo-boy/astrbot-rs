# Project: astrbot-rs

## What This Is

astrbot-rs 是一次以 Rust 重构 AstrBot 的长期工程，目标不是逐行翻译 Python 实现，而是学习 AstrBot 的产品理念、事件模型和扩展生态后，重新设计一个更强类型、更清晰边界、更容易测试和部署的对话式智能基础设施。项目参考 `E:/playground/Astrbot`，优先保留一站式 Agentic 聊天助手、多平台接入、Provider 抽象、插件扩展、知识库、人格和 WebUI 等核心能力。

## Core Value

把 AstrBot 的“多平台消息入口 + 可扩展 Agent/插件能力 + 可运营配置面板”理念落到一个去耦合的 Rust 架构中：任何平台、Provider、插件和 Pipeline Stage 都应通过明确接口协作，而不是依赖全局状态或跨模块隐式耦合。

## Requirements

### Validated

(None yet - ship to validate)

### Active

- [ ] 建立 Rust workspace，先定义 `core`、`platform`、`provider`、`plugin`、`pipeline`、`dashboard` 等可独立演进的 crate 边界。
- [ ] 抽取 AstrBot 消息流理念：平台适配器提交统一事件，事件总线分发到 Pipeline，Pipeline 负责唤醒、权限、插件、Agent 请求和回复发送。
- [ ] 抽取 AstrBot 扩展理念：Provider、Platform、Plugin/Star、Tool、Knowledge Base 均以注册表和 trait/object-safe 接口管理。
- [ ] 优先做一个最小可运行闭环：CLI 启动、WebChat 或 Mock Platform 收消息、OpenAI-compatible Provider 响应、Pipeline 输出结果。
- [ ] 用 Maestro 维护路线、里程碑、规格和执行记录，避免一次性大爆炸迁移。

### Out of Scope

- 逐行移植 Python 代码 - Rust 版应围绕接口和边界重构，避免复制现有耦合。
- 首个里程碑覆盖所有平台和模型供应商 - 先实现协议与注册机制，再批量补适配器。
- 首个里程碑实现完整 Dashboard - 先保证后端 API 和运行时状态可观测。
- 兼容 Python 插件运行时 - 可作为后续桥接议题，当前优先设计 Rust 原生插件 API。

## Context

参考 AstrBot 是开源的一站式 Agentic 个人和群聊助手，支持 QQ、Telegram、企业微信、飞书、钉钉、Slack 等消息平台，支持多模态 LLM、Agent、MCP、Skills、知识库、人格设定、自动压缩对话、插件市场、Agent Sandbox、WebUI 和 ChatUI。

参考项目中的关键理念：

- `core_lifecycle.py` 统一装配配置、数据库、Persona、Provider、Platform、Knowledge Base、Plugin、Pipeline 和 EventBus。
- `event_bus.py` 只做事件队列消费、日志记录和 Scheduler 分发，业务行为放到 Pipeline。
- `pipeline/scheduler.py` 用有序 Stage 和异步生成器形成类似中间件的前后置处理模型。
- `platform/platform.py` 抽象平台适配器，平台只负责运行、提交事件和按会话发送消息。
- `provider/provider.py` 抽象 Chat、STT、TTS、Embedding、Rerank 等 Provider 类型。
- `star/*` 以插件元数据、Handler Registry、事件类型和过滤器组成插件扩展系统。

## Constraints

- **Architecture**: 去耦合优先 - 核心 crate 不应依赖具体平台、具体 Provider、Dashboard 或插件实现。
- **Migration**: 学习理念而非照搬实现 - Rust 设计可以改变目录、类型和生命周期模型。
- **Runtime**: 异步优先 - 消息入口、Provider 调用、Pipeline、Web API 均按 `tokio` 异步模型设计。
- **Safety**: 外部调用边界清晰 - Provider、插件、工具执行、文件/网络能力需要可测试、可审计、可降级。
- **Progress**: 使用 Maestro - 路线图、规格、里程碑和关键决策写入 `.workflow`。

## Tech Stack

- **Language**: Rust
- **Async Runtime**: tokio
- **Web/API**: axum or similar async HTTP framework
- **Serialization**: serde
- **Database**: SQLite first, behind repository traits
- **Dashboard**: later milestone, likely Vue or lightweight SPA served by Rust backend

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 从 Maestro 初始化项目管理，而不是直接恢复旧实现 | 当前分支按从零开始处理，先固定目标、边界和规格 | Accepted |
| Rust 版学习 AstrBot 理念，不逐行翻译 Python | 可避免把动态导入、全局注册表和跨模块状态原样带入 Rust | Accepted |
| 首个工程目标是最小消息闭环 | 能最快验证 Platform -> EventBus -> Pipeline -> Provider -> Respond 的核心架构 | Pending |
| 以 trait + registry + dependency injection 替代跨模块动态导入 | 保留扩展能力，同时让编译期边界和测试替身更自然 | Pending |

## Stakeholders

- 项目维护者/实现者
- AstrBot 用户和插件生态迁移使用者
- 未来平台适配器、Provider、插件开发者

---
*Last updated: 2026-05-15 after initialization*
