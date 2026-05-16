use astrbot_core::Result;
use async_trait::async_trait;

use crate::{
    PluginContext, PluginPermission, PluginToolDeclaration, SandboxProfile, SandboxedToolExecutor,
    ToolCapability, ToolCapabilityDecision, ToolExecutionRequest, ToolExecutionResult,
    ToolExecutionStatus, ToolExecutor,
};

struct EchoToolExecutor;

#[async_trait]
impl ToolExecutor for EchoToolExecutor {
    async fn execute(&self, request: ToolExecutionRequest) -> Result<ToolExecutionResult> {
        Ok(ToolExecutionResult::completed(
            request.argument("text").unwrap_or(""),
        ))
    }
}

#[test]
fn tool_capability_decision_reports_missing_sandbox_requirements() {
    let declaration = PluginToolDeclaration::local("browser")
        .requires_permission(PluginPermission::UseNetwork)
        .requires_capability(ToolCapability::Browser);
    let decision = ToolCapabilityDecision::check(&declaration, &SandboxProfile::restricted());

    assert!(!decision.allowed);
    assert_eq!(
        decision.missing_permissions,
        vec![PluginPermission::UseNetwork]
    );
    assert_eq!(decision.missing_capabilities, vec![ToolCapability::Browser]);
    assert!(
        decision
            .rejection_message("browser")
            .expect("rejection should explain missing requirements")
            .contains("missing tool capabilities")
    );
}

#[tokio::test]
async fn sandboxed_tool_executor_runs_allowed_tools() {
    let declaration =
        PluginToolDeclaration::local("browser").requires_capability(ToolCapability::Browser);
    let context = PluginContext::new("tools").with_sandbox_profile(
        SandboxProfile::restricted().with_tool_capability(ToolCapability::Browser),
    );
    let request = ToolExecutionRequest::new(declaration, context).with_argument("text", "ok");

    let result = SandboxedToolExecutor::new(EchoToolExecutor)
        .execute(request)
        .await
        .expect("allowed tool should execute");

    assert_eq!(result.status, ToolExecutionStatus::Completed);
    assert_eq!(result.content.as_deref(), Some("ok"));
}

#[tokio::test]
async fn sandboxed_tool_executor_rejects_missing_capability() {
    let declaration =
        PluginToolDeclaration::local("browser").requires_capability(ToolCapability::Browser);
    let context = PluginContext::new("tools");
    let request = ToolExecutionRequest::new(declaration, context);

    let err = SandboxedToolExecutor::new(EchoToolExecutor)
        .execute(request)
        .await
        .expect_err("missing browser capability should reject execution");

    assert!(err.to_string().contains("rejected by sandbox"));
}
