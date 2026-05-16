use crate::{
    PluginCompatibility, PluginDocument, PluginInstallSource, PluginMarketAction,
    PluginMarketCache, PluginMarketEntry, PluginMarketOperationPlan, PluginPackageDescriptor,
    PluginRegistrySource, PluginUninstallPlan,
};

#[test]
fn plugin_registry_source_models_default_and_custom_cache_md5_boundaries() {
    let default = PluginRegistrySource::default_collection("data/plugins.json");
    assert_eq!(default.urls.len(), 2);
    assert_eq!(
        default.md5_url.as_deref(),
        Some("https://api.soulter.top/astrbot/plugins-md5")
    );

    let custom = PluginRegistrySource::custom(
        "https://example.com/market.json",
        "data/plugins_custom.json",
    );
    assert_eq!(
        custom.md5_url.as_deref(),
        Some("https://example.com/market-md5.json")
    );

    let cache = PluginMarketCache::new("2026-05-17T00:00:00Z", Vec::new()).with_md5("abc");
    assert!(cache.is_valid_for_remote_md5(Some("abc")));
    assert!(!cache.is_valid_for_remote_md5(Some("def")));
}

#[test]
fn market_entry_carries_package_compatibility_and_documents() {
    let entry = PluginMarketEntry::new("Fancy Plugin", "Fancy Plugin", "1.2.3")
        .with_repo_url("https://github.com/example/fancy")
        .with_package(
            PluginPackageDescriptor::new(PluginInstallSource::archive(
                "https://example.com/fancy.zip",
            ))
            .with_checksum_md5("abc123")
            .with_cache_key("fancy.zip"),
        )
        .with_compatibility(PluginCompatibility::compatible(">=0.1.0"))
        .with_readme(PluginDocument::markdown("# Fancy"))
        .with_changelog(PluginDocument::missing());

    assert_eq!(entry.plugin_id, "fancy_plugin");
    assert_eq!(
        entry
            .package
            .as_ref()
            .and_then(|pkg| pkg.checksum_md5.as_deref()),
        Some("abc123")
    );
    assert!(entry.compatibility.compatible);
    assert_eq!(
        entry.readme.as_ref().and_then(|doc| doc.content.as_deref()),
        Some("# Fancy")
    );
    assert!(
        entry
            .changelog
            .as_ref()
            .is_some_and(|doc| doc.content.is_none())
    );
}

#[test]
fn plugin_market_operation_plan_separates_side_effect_planning_from_loader() {
    let entry = PluginMarketEntry::new("tools", "Tools", "0.2.0")
        .with_repo_url("https://github.com/example/tools")
        .with_compatibility(PluginCompatibility::incompatible(
            "<0.1.0",
            "requires an older AstrBot",
        ));

    let install =
        PluginMarketOperationPlan::from_market_entry(&entry).expect("entry has repo source");
    assert_eq!(install.action, PluginMarketAction::Install);
    assert!(install.requires_download);
    assert!(install.requires_unpack);
    assert!(install.requires_loader_reload);
    assert!(install.is_blocked_by_compatibility());

    let uninstall =
        PluginMarketOperationPlan::uninstall(PluginUninstallPlan::new("tools").delete_config());
    assert_eq!(uninstall.action, PluginMarketAction::Uninstall);
    assert!(!uninstall.requires_download);
    assert!(uninstall.delete_config);
    assert!(!uninstall.delete_data);
}
