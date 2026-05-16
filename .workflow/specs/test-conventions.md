# Test Conventions

<spec-entry category="test" keywords="mock-platform,mock-provider,pipeline-tests,e2e" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/pipeline/process_stage/stage.py">

### First E2E Test Uses Mock Platform And Mock Provider

首个端到端测试应验证：Mock Platform 提交统一消息事件，EventBus 分发到 Pipeline，Pipeline 调用 Mock Provider，Respond Stage 通过事件回写消息链。这个测试是重构过程中判断核心闭环是否仍然成立的最低门槛。

</spec-entry>
