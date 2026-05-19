# Reflection Log

## Round 0 — Plan Authoring (2026-05-20)

- 目标：把 Python Astrbot → Rust 重写的剩余差距落成可执行任务。
- 输入：两份 Explore 调研（Python 原版功能盘点 + Rust 当前实现现状）。
- 用户决策（一次性锁定）：
  - WASM runtime = **wasmtime**
  - Sandbox 职责 = WASM 只管插件；Computer Sandbox 走 **OS 子进程**
  - Wave1 P0 平台 = **Telegram / 公众号 / 企业微信 / QQ 官方 Webhook / QQ 频道**（原版无个人微信，已转 P2 backlog）
  - 图表库 = **Apache ECharts**
  - 计划组织 = **单 Plan，P0/P1/P2 标签**

### 任务编排逻辑

- **P0 (14)** = WASM Sandbox 基础设施 (TASK-P0-001..004) + Wave1 五个最常用平台 (TASK-P0-005..009) + Dashboard 三件套（消息渲染/Config 树/Schema 表单）(TASK-P0-010..012) + MCP 双传输 (TASK-P0-013..014)。这一层完成意味着：插件生态、五大平台、消息核心 UI、MCP 完全可用。
- **P1 (14)** = builtin_stars 五批平移 (TASK-P1-001..005) + 记忆深度 (P1-006) + T2I 真渲染 (P1-007) + KB Parser/向量后端 (P1-008..009) + Dashboard 三页 (Knowledge/Persona/Observability) (P1-010..012) + Pipeline 安全策略深度 (P1-013) + 媒体转换 (P1-014)。完成后业务深度与原版基本对齐。
- **P2 (8)** = Computer Sandbox 两步 (P2-001..002) + 第三方 Agent 代理 (P2-003) + Wave1 剩余 8 个适配器 (P2-004) + E2E (P2-005) + Updater (P2-006) + Prometheus (P2-007) + Skills 填肉 (P2-008)。

### 已知遗留 / 待澄清

1. **个人微信适配器**：Python 原版没有；如确实需要走 Wechaty Puppet，单独 RFC 追加为 TASK-P2-009。
2. **T2I 渲染引擎选型**：TASK-P1-007 含 DECISION 子任务（chromiumoxide vs femtovg+skia），交付时再请用户判一次。
3. **Skill 与 Tool 边界**：原版 Skills/Tools/Stars 三层职责实际部分重叠，TASK-P2-008 实施前需再读 Python `astrbot/builtin_stars/` 与 `astrbot/core/agent/` 二次确认。
4. **Wave1 共享 webhook 验签框架**：在 TASK-P0-005(Telegram) 落地时一并抽取为复用层，避免 TASK-P0-006..008 与 P2-004 重复实现签名校验。
5. **WASM ABI 序列化格式**：TASK-P0-002 中 JSON vs postcard 留待 microbench 之后再定；不阻塞 ABI 设计。

### 验证基线

- 后端：`cargo check --workspace --all-targets`、`cargo test --workspace`
- 前端：`(cd dashboard-next && npx tsc --noEmit && npx vite build)` 主包 ≤ 250KB gzip
- DTO 漂移：`cargo test -p astrbot-web export_bindings && git diff --exit-code dashboard-next/src/api/dto/`
- Plugin Sandbox：加载/卸载循环 + 资源限制断言
- 平台冒烟：每平台至少一次 echo

### 下一步

- TASK-P0-001 优先开干（WASM engine 集成），它为 P0-002/003/004 + P1-001..005 提供地基。
- Wave1 P0 五个平台与 WASM 链路彼此独立，可并行；TASK-P0-005 (Telegram) 接入成本最低，建议作为 Wave1 首发。
- Dashboard P0 三件套 (010/011/012) 阻塞绝大部分 P1 dashboard 任务，建议尽早完成。
