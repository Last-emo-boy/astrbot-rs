use crate::{
    HotReloadDecision, PLUGIN_SDK_VERSION, PluginCapability, PluginFileChange,
    PluginFileChangeKind, PluginLifecycleAction, PluginLifecycleState, PluginLoadSource,
    PluginLoader, PluginManifest, PluginPermission, plan_hot_reload,
};

#[test]
fn plugin_loader_discovers_and_activates_manifest_metadata() {
    let manifest = PluginManifest::new("My Fancy Plugin", "0.1.0")
        .with_permission(PluginPermission::RegisterWebApi)
        .with_capability(PluginCapability::WebApi);
    let mut loader = PluginLoader::new();

    let metadata = loader
        .discover_manifest(PluginLoadSource::native_rust("My Fancy Plugin"), manifest)
        .expect("manifest should be discovered")
        .with_supported_platform("webchat")
        .with_runtime_version(PLUGIN_SDK_VERSION);

    assert_eq!(metadata.plugin_id(), "my_fancy_plugin");
    assert_eq!(metadata.supported_platforms(), &["webchat".to_string()]);
    assert_eq!(metadata.runtime_version(), Some(PLUGIN_SDK_VERSION));

    let loaded = loader
        .mark_loaded("my_fancy_plugin")
        .expect("plugin should transition to loaded");
    assert_eq!(loaded.previous, PluginLifecycleState::Discovered);
    assert_eq!(loaded.next, PluginLifecycleState::Loaded);

    let activated = loader
        .activate("my_fancy_plugin")
        .expect("plugin should transition to active");
    assert_eq!(activated.action, PluginLifecycleAction::Activate);
    assert_eq!(activated.next, PluginLifecycleState::Active);

    let context = loader
        .context_for("my_fancy_plugin")
        .expect("plugin context should be available");
    assert!(context.allows_permission(PluginPermission::RegisterWebApi));
}

#[test]
fn plugin_loader_disables_and_unloads_without_dynamic_imports() {
    let manifest = PluginManifest::new("stateful", "0.1.0");
    let mut loader = PluginLoader::new();
    loader
        .discover_manifest(PluginLoadSource::python_compat("stateful"), manifest)
        .expect("manifest should be discovered");
    loader
        .mark_loaded("stateful")
        .expect("plugin should transition to loaded");
    loader
        .activate("stateful")
        .expect("plugin should transition to active");

    let disabled = loader
        .disable("stateful")
        .expect("plugin should transition to disabled");
    assert_eq!(disabled.previous, PluginLifecycleState::Active);
    assert_eq!(disabled.next, PluginLifecycleState::Disabled);

    let unloaded = loader
        .unload("stateful")
        .expect("plugin should transition to unloaded");
    assert_eq!(unloaded.next, PluginLifecycleState::Unloaded);
}

#[test]
fn hot_reload_plans_source_changes() {
    let source_change = PluginFileChange::new(
        "plugin",
        "plugins/plugin/src/lib.rs",
        PluginFileChangeKind::Modified,
    );
    let asset_change = PluginFileChange::new(
        "plugin",
        "plugins/plugin/logo.png",
        PluginFileChangeKind::Modified,
    );
    let removal = PluginFileChange::new("plugin", "plugins/plugin", PluginFileChangeKind::Removed);

    assert_eq!(plan_hot_reload(&source_change), HotReloadDecision::Reload);
    assert_eq!(plan_hot_reload(&asset_change), HotReloadDecision::Ignore);
    assert_eq!(plan_hot_reload(&removal), HotReloadDecision::Unload);
}
