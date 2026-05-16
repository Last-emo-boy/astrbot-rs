# Quality Rules

<spec-entry category="quality" keywords="unit-tests,edge-cases,public-api,coverage" date="2026-05-15" source="AGENTS.md">

### Public Interfaces Need Focused Tests

所有 public trait、registry、Pipeline Stage、Provider adapter 和 Platform adapter 都需要覆盖正常路径、错误路径和边界条件。外部 Provider、网络、文件系统和平台 API 必须可 mock。

</spec-entry>

<spec-entry category="rule" keywords="error-handling,redaction,graceful-degradation" date="2026-05-15" source="AGENTS.md">

### Errors Must Be Structured And Redacted

错误类型应携带清晰上下文并避免泄露 token、key、cookie、用户隐私或平台原始敏感 payload。Provider 和插件错误不应导致核心运行时崩溃，除非启动期必要依赖不可用。

</spec-entry>
