# Review Standards

<spec-entry category="review" keywords="coupling,api-boundaries,regression-risk" date="2026-05-15" source="AGENTS.md">

### Reviews Prioritize Coupling And Boundary Leaks

代码审查应优先检查跨 crate 依赖方向、trait 是否泄露具体实现、全局状态是否可替换、外部服务是否被 mock、错误是否可诊断，以及 Pipeline/Provider/Platform 改动是否引入行为回归。

</spec-entry>
