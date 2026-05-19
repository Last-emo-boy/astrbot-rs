use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    DependencyConflictKind, DependencyConflictReport, DependencyInstallOutcome,
    DependencyInstallRequest, FixturePluginMarketPackageFetcher, NoopDependencyInstaller,
    PluginCompatibility, PluginDependencyPlanInstaller, PluginDocument, PluginInstallSource,
    PluginMarketAction, PluginMarketCache, PluginMarketDownloadedPackage, PluginMarketEntry,
    PluginMarketExecutionContext, PluginMarketExecutionOptions, PluginMarketExecutionStatus,
    PluginMarketExecutor, PluginMarketOperationPlan, PluginMarketPackageFetcher,
    PluginMarketRuntimeReloader, PluginPackageDescriptor, PluginRegistrySource,
    PluginUninstallPlan, derive_repository_archive_url,
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

#[test]
fn plugin_market_cache_parses_source_registry_entries() {
    let cache = PluginMarketCache::from_json_str(
        r#"{
            "timestamp": "2026-05-19T00:00:00Z",
            "md5": "abc",
            "data": {
                "FancyPlugin": {
                    "desc": "Fancy desc",
                    "version": "1.2.3",
                    "author": "Alice",
                    "repo": "https://github.com/example/fancy-plugin",
                    "astrbot_version": ">=0.1.0",
                    "compatible": true,
                    "download_url": "https://example.com/fancy.zip",
                    "md5": "deadbeef"
                }
            }
        }"#,
    )
    .expect("source cache parses");

    let entry = cache.entry("fancyplugin").expect("entry");
    assert_eq!(entry.plugin_id, "fancyplugin");
    assert_eq!(entry.name, "FancyPlugin");
    assert_eq!(entry.version, "1.2.3");
    assert_eq!(
        entry
            .package
            .as_ref()
            .and_then(|pkg| pkg.checksum_md5.as_deref()),
        Some("deadbeef")
    );

    let plan = PluginMarketOperationPlan::from_market_entry(entry).expect("plan");
    assert_eq!(plan.action, PluginMarketAction::Install);
}

#[test]
fn repository_sources_derive_source_compatible_archive_urls() {
    assert_eq!(
        derive_repository_archive_url("https://github.com/AstrBotDevs/demo-plugin").unwrap(),
        "https://github.com/AstrBotDevs/demo-plugin/archive/refs/heads/master.zip"
    );
    assert_eq!(
        derive_repository_archive_url("https://github.com/AstrBotDevs/demo-plugin/tree/dev")
            .unwrap(),
        "https://github.com/AstrBotDevs/demo-plugin/archive/refs/heads/dev.zip"
    );
}

#[tokio::test]
async fn plugin_market_executor_installs_archive_with_checksum_unpack_metadata_requirements_and_reload()
 {
    let root = unique_test_dir("market-install");
    let archive_path = root.join("fixtures").join("tools.zip");
    write_plugin_zip(
        &archive_path,
        "tools-main",
        "tools",
        "0.2.0",
        Some("watchfiles>=0.21\n"),
    );
    let checksum = format!(
        "{:x}",
        md5::compute(fs::read(&archive_path).expect("archive bytes"))
    );

    let package = PluginPackageDescriptor::new(PluginInstallSource::archive(
        "https://example.com/tools.zip",
    ))
    .with_checksum_md5(checksum);
    let plan = PluginMarketOperationPlan::install(crate::PluginInstallPlan::new("tools", package));
    let fetcher = FixturePluginMarketPackageFetcher::new();
    fetcher.push_package(
        PluginMarketDownloadedPackage::new(
            PluginInstallSource::archive("https://example.com/tools.zip"),
            archive_path.clone(),
        )
        .cache_hit(),
    );
    let installer = RecordingPlanInstaller::default();
    let reloader = RecordingReloader::default();
    let executor = PluginMarketExecutor::with_parts(
        execution_context(&root).with_site_packages_root(root.join("site-packages")),
        fetcher,
        installer.clone(),
        reloader.clone(),
    );

    let result = executor
        .execute(plan, PluginMarketExecutionOptions::default())
        .await;

    assert_eq!(result.status, PluginMarketExecutionStatus::Completed);
    assert!(result.is_success());
    assert_eq!(
        result.metadata.as_ref().map(|meta| meta.name.as_str()),
        Some("tools")
    );
    assert_eq!(
        fs::read_to_string(root.join("plugins/tools/metadata.yaml")).expect("metadata"),
        "name: tools\nversion: 0.2.0\ndesc: Test plugin\n"
    );
    assert_eq!(
        result
            .dependency_outcome
            .as_ref()
            .map(|outcome| outcome.status.as_str()),
        Some("completed")
    );
    let requests = installer.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].plan.dependencies()[0].name, "watchfiles");
    assert_eq!(
        requests[0].environment.package_preference(),
        crate::PackagePreferencePolicy::PreferInstalledSitePackages
    );
    assert_eq!(
        reloader.reloads(),
        vec![("tools".to_string(), root.join("plugins/tools"))]
    );
}

#[tokio::test]
async fn plugin_market_executor_blocks_incompatible_install_until_ignored() {
    let root = unique_test_dir("market-compat");
    let archive_path = root.join("fixtures").join("tools.zip");
    write_plugin_zip(&archive_path, "tools-main", "tools", "0.2.0", None);
    let package = PluginPackageDescriptor::new(PluginInstallSource::archive(
        "https://example.com/tools.zip",
    ));
    let plan = PluginMarketOperationPlan::install(
        crate::PluginInstallPlan::new("tools", package.clone()).with_compatibility(
            PluginCompatibility::incompatible("<0.1.0", "requires old core"),
        ),
    );
    let fetcher = FixturePluginMarketPackageFetcher::new();
    let executor = PluginMarketExecutor::with_parts(
        execution_context(&root),
        fetcher.clone(),
        NoopDependencyInstaller,
        NoopReloader,
    );

    let blocked = executor
        .execute(plan.clone(), PluginMarketExecutionOptions::default())
        .await;
    assert_eq!(blocked.status, PluginMarketExecutionStatus::Failed);
    assert!(
        blocked
            .failure
            .unwrap()
            .message
            .contains("requires old core")
    );
    assert!(!root.join("plugins/tools").exists());

    fetcher.push_package(
        PluginMarketDownloadedPackage::new(
            PluginInstallSource::archive("https://example.com/tools.zip"),
            archive_path,
        )
        .cache_hit(),
    );
    let installed = executor
        .execute(
            plan.ignoring_compatibility(),
            PluginMarketExecutionOptions::default(),
        )
        .await;
    assert_eq!(installed.status, PluginMarketExecutionStatus::Completed);
    assert!(root.join("plugins/tools/metadata.yaml").exists());
}

#[tokio::test]
async fn plugin_market_executor_redacts_dependency_conflicts_and_rolls_back_update() {
    let root = unique_test_dir("market-conflict");
    fs::create_dir_all(root.join("plugins/tools")).expect("plugin dir");
    fs::write(
        root.join("plugins/tools/metadata.yaml"),
        "name: tools\nversion: 0.1.0\n",
    )
    .expect("old metadata");

    let archive_path = root.join("fixtures").join("tools.zip");
    write_plugin_zip(
        &archive_path,
        "tools-main",
        "tools",
        "0.2.0",
        Some("httpx==0.20\n"),
    );
    let package = PluginPackageDescriptor::new(PluginInstallSource::archive(
        "https://example.com/tools.zip",
    ));
    let plan = PluginMarketOperationPlan::update(crate::PluginUpdatePlan::new("tools", package));
    let fetcher = FixturePluginMarketPackageFetcher::new();
    fetcher.push_package(
        PluginMarketDownloadedPackage::new(
            PluginInstallSource::archive("https://example.com/tools.zip"),
            archive_path,
        )
        .cache_hit(),
    );
    let installer = FailingPlanInstaller::from_output([
        "The user requested httpx==0.20",
        "astrbot-core depends on httpx==0.27 (constraint)",
        "Using index https://user:secret@example.com/simple?token=abc",
        "--password=hunter2",
    ]);
    let executor = PluginMarketExecutor::with_parts(
        execution_context(&root),
        fetcher,
        installer,
        NoopReloader,
    );

    let result = executor
        .execute(plan, PluginMarketExecutionOptions::default())
        .await;

    assert_eq!(result.status, PluginMarketExecutionStatus::Failed);
    assert_eq!(
        fs::read_to_string(root.join("plugins/tools/metadata.yaml")).expect("restored metadata"),
        "name: tools\nversion: 0.1.0\n"
    );
    let outcome = result.dependency_outcome.expect("dependency outcome");
    assert_eq!(outcome.status, "failed");
    assert_eq!(
        outcome.conflicts[0].kind,
        DependencyConflictKind::CoreVersionConflict
    );
    let details = outcome.conflicts[0].details().join("\n");
    assert!(details.contains("https://<redacted>@example.com/simple?token=****"));
    assert!(details.contains("--password=****"));
    assert!(!details.contains("hunter2"));
}

#[tokio::test]
async fn plugin_market_executor_installs_uploaded_archive_and_uninstalls_optional_artifacts() {
    let root = unique_test_dir("market-upload-uninstall");
    let upload_path = root.join("uploads").join("upload-tools.zip");
    write_plugin_zip(
        &upload_path,
        "upload-tools-main",
        "upload_tools",
        "0.1.0",
        None,
    );
    fs::create_dir_all(root.join("plugin-config")).expect("config dir");
    fs::create_dir_all(root.join("plugin-data/upload_tools")).expect("data dir");
    fs::write(root.join("plugin-config/upload_tools.json"), "{}").expect("config");

    let package =
        PluginPackageDescriptor::new(PluginInstallSource::uploaded_archive("upload-tools.zip"));
    let plan =
        PluginMarketOperationPlan::install(crate::PluginInstallPlan::new("upload_tools", package));
    let executor = PluginMarketExecutor::with_parts(
        execution_context(&root),
        ReqwestBypassedFetcher,
        NoopDependencyInstaller,
        NoopReloader,
    );

    let install = executor
        .execute(plan, PluginMarketExecutionOptions::default())
        .await;
    assert_eq!(install.status, PluginMarketExecutionStatus::Completed);
    assert!(root.join("plugins/upload_tools/metadata.yaml").exists());

    let uninstall = executor
        .execute(
            PluginMarketOperationPlan::uninstall(
                PluginUninstallPlan::new("upload_tools")
                    .delete_config()
                    .delete_data(),
            ),
            PluginMarketExecutionOptions::default(),
        )
        .await;
    assert_eq!(uninstall.status, PluginMarketExecutionStatus::Completed);
    assert!(uninstall.removed_config);
    assert!(uninstall.removed_data);
    assert!(!root.join("plugins/upload_tools").exists());
    assert!(!root.join("plugin-config/upload_tools.json").exists());
    assert!(!root.join("plugin-data/upload_tools").exists());
}

#[tokio::test]
async fn plugin_market_executor_records_hot_reload_failure() {
    let root = unique_test_dir("market-reload-fail");
    let archive_path = root.join("fixtures").join("tools.zip");
    write_plugin_zip(&archive_path, "tools-main", "tools", "0.2.0", None);
    let package = PluginPackageDescriptor::new(PluginInstallSource::archive(
        "https://example.com/tools.zip",
    ));
    let plan = PluginMarketOperationPlan::install(crate::PluginInstallPlan::new("tools", package));
    let fetcher = FixturePluginMarketPackageFetcher::new();
    fetcher.push_package(
        PluginMarketDownloadedPackage::new(
            PluginInstallSource::archive("https://example.com/tools.zip"),
            archive_path,
        )
        .cache_hit(),
    );
    let executor = PluginMarketExecutor::with_parts(
        execution_context(&root),
        fetcher,
        NoopDependencyInstaller,
        FailingReload,
    );

    let result = executor
        .execute(plan, PluginMarketExecutionOptions::default())
        .await;

    assert_eq!(result.status, PluginMarketExecutionStatus::Failed);
    let failure = result.failure.expect("failure record");
    assert!(failure.message.contains("plugin hot reload failed"));
    assert!(failure.rollback_hint.is_some());
}

fn execution_context(root: &Path) -> PluginMarketExecutionContext {
    PluginMarketExecutionContext::new(root)
        .with_plugin_store_dir(root.join("plugins"))
        .with_package_cache_dir(root.join("cache"))
        .with_upload_dir(root.join("uploads"))
        .with_config_dir(root.join("plugin-config"))
        .with_data_dir(root.join("plugin-data"))
        .with_dependency_root_dir(root.join("plugin-deps"))
}

fn write_plugin_zip(
    archive_path: &Path,
    root_dir: &str,
    plugin_name: &str,
    version: &str,
    requirements: Option<&str>,
) {
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).expect("archive parent");
    }
    let file = File::create(archive_path).expect("archive file");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(format!("{root_dir}/metadata.yaml"), options)
        .expect("metadata entry");
    write!(
        zip,
        "name: {plugin_name}\nversion: {version}\ndesc: Test plugin\n"
    )
    .expect("metadata content");
    zip.start_file(format!("{root_dir}/README.md"), options)
        .expect("readme entry");
    write!(zip, "# {plugin_name}").expect("readme content");
    if let Some(requirements) = requirements {
        zip.start_file(format!("{root_dir}/requirements.txt"), options)
            .expect("requirements entry");
        write!(zip, "{requirements}").expect("requirements content");
    }
    zip.finish().expect("finish zip");
}

fn unique_test_dir(prefix: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("astrbot-plugin-{prefix}-{nonce}"));
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove old test dir");
    }
    root
}

#[derive(Clone, Default)]
struct RecordingPlanInstaller {
    requests: Arc<Mutex<Vec<DependencyInstallRequest>>>,
}

impl RecordingPlanInstaller {
    fn requests(&self) -> Vec<DependencyInstallRequest> {
        self.requests
            .lock()
            .expect("installer requests lock")
            .clone()
    }
}

#[async_trait]
impl PluginDependencyPlanInstaller for RecordingPlanInstaller {
    async fn install_dependencies(
        &self,
        request: DependencyInstallRequest,
    ) -> Result<DependencyInstallOutcome> {
        self.requests
            .lock()
            .expect("installer requests lock")
            .push(request.clone());
        Ok(DependencyInstallOutcome::completed(&request.plan))
    }
}

struct FailingPlanInstaller {
    output: Vec<String>,
}

impl FailingPlanInstaller {
    fn from_output(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            output: lines.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl PluginDependencyPlanInstaller for FailingPlanInstaller {
    async fn install_dependencies(
        &self,
        request: DependencyInstallRequest,
    ) -> Result<DependencyInstallOutcome> {
        Ok(DependencyInstallOutcome::failed(
            request.plan.plugin_id().to_string(),
            vec![
                DependencyConflictReport::from_installer_output(
                    request.plan.plugin_id().to_string(),
                    self.output.iter(),
                )
                .expect("conflict"),
            ],
        ))
    }
}

#[derive(Clone, Default)]
struct RecordingReloader {
    reloads: Arc<Mutex<Vec<(String, PathBuf)>>>,
}

impl RecordingReloader {
    fn reloads(&self) -> Vec<(String, PathBuf)> {
        self.reloads.lock().expect("reload lock").clone()
    }
}

#[async_trait]
impl PluginMarketRuntimeReloader for RecordingReloader {
    async fn reload_plugin(&self, plugin_id: &str, plugin_dir: &Path) -> Result<()> {
        self.reloads
            .lock()
            .expect("reload lock")
            .push((plugin_id.to_string(), plugin_dir.to_path_buf()));
        Ok(())
    }

    async fn unload_plugin(&self, _plugin_id: &str) -> Result<()> {
        Ok(())
    }
}

struct NoopReloader;

#[async_trait]
impl PluginMarketRuntimeReloader for NoopReloader {
    async fn reload_plugin(&self, _plugin_id: &str, _plugin_dir: &Path) -> Result<()> {
        Ok(())
    }

    async fn unload_plugin(&self, _plugin_id: &str) -> Result<()> {
        Ok(())
    }
}

struct FailingReload;

#[async_trait]
impl PluginMarketRuntimeReloader for FailingReload {
    async fn reload_plugin(&self, _plugin_id: &str, _plugin_dir: &Path) -> Result<()> {
        Err(AstrbotError::Pipeline("reload exploded".to_string()))
    }

    async fn unload_plugin(&self, _plugin_id: &str) -> Result<()> {
        Ok(())
    }
}

struct ReqwestBypassedFetcher;

#[async_trait]
impl PluginMarketPackageFetcher for ReqwestBypassedFetcher {
    async fn fetch_package(
        &self,
        package: &PluginPackageDescriptor,
        context: &PluginMarketExecutionContext,
    ) -> Result<PluginMarketDownloadedPackage> {
        match &package.source {
            PluginInstallSource::UploadedArchive { filename } => {
                Ok(PluginMarketDownloadedPackage::new(
                    package.source.clone(),
                    context.upload_dir.join(filename),
                )
                .cache_hit())
            }
            _ => Err(AstrbotError::Pipeline("unexpected source".to_string())),
        }
    }
}
