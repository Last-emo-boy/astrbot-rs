use crate::{
    BackgroundTaskPolicy, HandoffToolTarget, PluginPermission, PluginToolDeclaration,
    PluginToolKind, ToolExecutionResult, ToolExecutionStatus,
};
use astrbot_tool::ToolSource;

#[test]
fn tool_declarations_model_handoff_and_background_boundaries() {
    let handoff = PluginToolDeclaration::handoff(
        "delegate",
        HandoffToolTarget::new("researcher")
            .with_provider_id("provider-a")
            .allow_background(),
    );
    let background = PluginToolDeclaration::background(
        "long-job",
        BackgroundTaskPolicy::new()
            .with_max_seconds(300)
            .with_note("summarize later"),
    )
    .requires_permission(PluginPermission::SpawnBackgroundTask);

    match handoff.kind {
        PluginToolKind::Handoff(target) => {
            assert_eq!(target.agent_name, "researcher");
            assert_eq!(target.provider_id.as_deref(), Some("provider-a"));
            assert!(target.background_allowed);
        }
        _ => panic!("expected handoff tool kind"),
    }

    match background.kind {
        PluginToolKind::Background(policy) => {
            assert!(policy.wake_on_complete);
            assert_eq!(policy.max_seconds, Some(300));
            assert_eq!(policy.note.as_deref(), Some("summarize later"));
        }
        _ => panic!("expected background tool kind"),
    }
}

#[test]
fn plugin_tool_declarations_project_tool_source_metadata() {
    let local =
        PluginToolDeclaration::local("local").source_metadata("plugin.weather", "Weather Plugin");
    let mcp = PluginToolDeclaration::mcp("search", "docs-server")
        .source_metadata("plugin.docs", "Docs Plugin");
    let handoff = PluginToolDeclaration::handoff("delegate", HandoffToolTarget::new("writer"))
        .source_metadata("plugin.agents", "Agent Plugin");
    let background = PluginToolDeclaration::background("long-job", BackgroundTaskPolicy::new())
        .source_metadata("plugin.jobs", "Jobs Plugin");

    assert_eq!(local.kind, ToolSource::Plugin);
    assert_eq!(local.plugin_id.as_deref(), Some("plugin.weather"));
    assert_eq!(local.origin_name(), "Weather Plugin");

    assert_eq!(mcp.kind, ToolSource::Mcp);
    assert_eq!(mcp.mcp_server_name.as_deref(), Some("docs-server"));

    assert_eq!(handoff.kind, ToolSource::Subagent);
    assert_eq!(handoff.subagent_id.as_deref(), Some("writer"));

    assert_eq!(background.kind, ToolSource::Background);
    assert_eq!(background.plugin_id.as_deref(), Some("plugin.jobs"));
}

#[test]
fn background_tool_execution_result_models_python_accepted_result() {
    let result =
        ToolExecutionResult::accepted_background("Background task submitted. task_id=task-1", true);

    assert_eq!(result.status, ToolExecutionStatus::AcceptedBackground);
    assert_eq!(
        result.content.as_deref(),
        Some("Background task submitted. task_id=task-1")
    );
    assert!(result.wake_main_agent);
}
