use crate::{
    BackgroundTaskPolicy, HandoffToolTarget, PluginPermission, PluginToolDeclaration,
    PluginToolKind,
};

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
