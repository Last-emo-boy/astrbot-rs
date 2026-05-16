# Coding Conventions

<spec-entry category="pattern" keywords="trait-first,adapter-pattern,dependency-injection,rust" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/platform/platform.py">

### Implement Adapters Behind Stable Traits

平台、Provider、Pipeline Stage、Tool、Plugin Runtime 都应先定义稳定 trait，再实现具体适配器。具体实现通过 registry/factory 装配，测试中使用 fake/mock adapter 替代外部依赖。

</spec-entry>

<spec-entry category="pattern" keywords="typed-events,message-chain,pipeline-stage" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/pipeline/scheduler.py">

### Keep Message Flow Typed

统一消息事件、消息链、Pipeline 结果、Provider 请求和工具调用结果应使用强类型结构表达。避免以 `serde_json::Value` 或字符串 map 作为核心模块之间的主要通信格式，除非处在外部协议边界。

</spec-entry>

<spec-entry category="pattern" keywords="provider-boundary,response-normalization,openai-compatible" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/provider/sources/openai_source.py">

### Normalize Provider Protocol Quirks At Adapter Boundary

OpenAI-compatible 返回的 assistant content 可能是字符串、`{"text": ...}` 对象或 content-parts 数组。Rust 版 Provider adapter 应在边界内完成归一化，向 Pipeline 返回稳定的 `MessageChain`，避免把具体供应商协议形态泄漏给核心流程。

</spec-entry>
