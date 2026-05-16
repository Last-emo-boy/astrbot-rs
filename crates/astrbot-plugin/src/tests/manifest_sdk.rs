use crate::{
    PluginCapability, PluginManifest, PluginPermission, PluginPlatformExtension,
    PluginPlatformExtensionKind, PluginTestHarness, PluginWebApiMethod, PluginWebApiRoute,
    ToolCapability,
};

#[test]
fn manifest_drives_plugin_context_sandbox_permissions() {
    let manifest = PluginManifest::new("tools", "0.1.0")
        .with_capability(PluginCapability::SandboxTool)
        .with_permission(PluginPermission::SendMessage)
        .with_tool_capability(ToolCapability::Browser);

    let harness = PluginTestHarness::from_manifest(&manifest);
    let ctx = harness.context();

    assert_eq!(ctx.plugin_name(), "tools");
    assert!(ctx.allows_permission(PluginPermission::SendMessage));
    assert!(ctx.allows_tool_capability(ToolCapability::Browser));
    assert!(!ctx.allows_tool_capability(ToolCapability::Shell));
}

#[test]
fn plugin_extension_descriptors_are_typed() {
    let platform_extension = PluginPlatformExtension::new(
        "plugin",
        "webchat-extra",
        PluginPlatformExtensionKind::MessageBridge,
        "webchat",
    )
    .with_description("extra webchat bridge");
    assert_eq!(platform_extension.platform_type, "webchat");
    assert_eq!(
        platform_extension.kind,
        PluginPlatformExtensionKind::MessageBridge
    );

    let route = PluginWebApiRoute::new("plugin", "api/plugins/plugin")
        .with_method(PluginWebApiMethod::Post)
        .with_description("plugin management route");
    assert_eq!(route.route, "/api/plugins/plugin");
    assert!(route.methods.contains(&PluginWebApiMethod::Get));
    assert!(route.methods.contains(&PluginWebApiMethod::Post));
}
