use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::dependency::{
    DependencyConflictReport, DependencyInstallOutcome, DependencyInstallRequest,
    DependencyInstallStatus, NoopDependencyInstaller, PackagePreferencePolicy, PluginDependency,
    PluginDependencyKind, PluginDependencyPlan, PluginDependencyPlanInstaller,
    PluginImportEnvironment,
};
use crate::manifest::PluginManifest;

use super::{
    PluginInstallSource, PluginMarketAction, PluginMarketCache, PluginMarketOperationPlan,
    PluginPackageDescriptor, PluginUninstallPlan,
};

#[derive(Clone, Debug)]
pub struct PluginMarketExecutionContext {
    pub plugin_store_dir: PathBuf,
    pub package_cache_dir: PathBuf,
    pub upload_dir: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub dependency_root_dir: PathBuf,
    pub site_packages_roots: Vec<PathBuf>,
}

impl PluginMarketExecutionContext {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let root_dir = root_dir.into();
        Self {
            plugin_store_dir: root_dir.join("plugins"),
            package_cache_dir: root_dir.join("plugin-package-cache"),
            upload_dir: root_dir.join("uploads"),
            config_dir: root_dir.join("plugin-config"),
            data_dir: root_dir.join("plugin-data"),
            dependency_root_dir: root_dir.join("plugin-deps"),
            site_packages_roots: Vec::new(),
        }
    }

    pub fn with_plugin_store_dir(mut self, plugin_store_dir: impl Into<PathBuf>) -> Self {
        self.plugin_store_dir = plugin_store_dir.into();
        self
    }

    pub fn with_package_cache_dir(mut self, package_cache_dir: impl Into<PathBuf>) -> Self {
        self.package_cache_dir = package_cache_dir.into();
        self
    }

    pub fn with_upload_dir(mut self, upload_dir: impl Into<PathBuf>) -> Self {
        self.upload_dir = upload_dir.into();
        self
    }

    pub fn with_config_dir(mut self, config_dir: impl Into<PathBuf>) -> Self {
        self.config_dir = config_dir.into();
        self
    }

    pub fn with_data_dir(mut self, data_dir: impl Into<PathBuf>) -> Self {
        self.data_dir = data_dir.into();
        self
    }

    pub fn with_dependency_root_dir(mut self, dependency_root_dir: impl Into<PathBuf>) -> Self {
        self.dependency_root_dir = dependency_root_dir.into();
        self
    }

    pub fn with_site_packages_root(mut self, site_packages_root: impl Into<PathBuf>) -> Self {
        let site_packages_root = site_packages_root.into();
        if !self
            .site_packages_roots
            .iter()
            .any(|known| known == &site_packages_root)
        {
            self.site_packages_roots.push(site_packages_root);
        }
        self
    }

    fn plugin_dir(&self, plugin_id: &str) -> PathBuf {
        self.plugin_store_dir.join(plugin_id)
    }

    fn config_path(&self, plugin_id: &str) -> PathBuf {
        self.config_dir.join(format!("{plugin_id}.json"))
    }

    fn data_path(&self, plugin_id: &str) -> PathBuf {
        self.data_dir.join(plugin_id)
    }

    fn dependency_root(&self, plugin_id: &str) -> PathBuf {
        self.dependency_root_dir.join(plugin_id)
    }
}

#[derive(Clone, Debug)]
pub struct PluginMarketExecutionOptions {
    pub ignore_compatibility: bool,
    pub prefer_installed_site_packages: bool,
    pub reload_after_operation: bool,
    pub clean_downloaded_archive: bool,
}

impl Default for PluginMarketExecutionOptions {
    fn default() -> Self {
        Self {
            ignore_compatibility: false,
            prefer_installed_site_packages: true,
            reload_after_operation: true,
            clean_downloaded_archive: true,
        }
    }
}

impl PluginMarketExecutionOptions {
    pub fn ignore_compatibility(mut self) -> Self {
        self.ignore_compatibility = true;
        self
    }

    pub fn prefer_isolated_dependencies(mut self) -> Self {
        self.prefer_installed_site_packages = false;
        self
    }

    pub fn skip_reload(mut self) -> Self {
        self.reload_after_operation = false;
        self
    }

    pub fn keep_downloaded_archive(mut self) -> Self {
        self.clean_downloaded_archive = false;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMarketDownloadedPackage {
    pub source: PluginInstallSource,
    pub archive_path: PathBuf,
    pub cache_hit: bool,
    pub checksum_md5: Option<String>,
}

impl PluginMarketDownloadedPackage {
    pub fn new(source: PluginInstallSource, archive_path: impl Into<PathBuf>) -> Self {
        Self {
            source,
            archive_path: archive_path.into(),
            cache_hit: false,
            checksum_md5: None,
        }
    }

    pub fn cache_hit(mut self) -> Self {
        self.cache_hit = true;
        self
    }

    pub fn with_checksum_md5(mut self, checksum_md5: impl Into<String>) -> Self {
        let checksum_md5 = checksum_md5.into();
        self.checksum_md5 = (!checksum_md5.trim().is_empty()).then_some(checksum_md5);
        self
    }
}

#[async_trait]
pub trait PluginMarketPackageFetcher: Send + Sync {
    async fn fetch_package(
        &self,
        package: &PluginPackageDescriptor,
        context: &PluginMarketExecutionContext,
    ) -> Result<PluginMarketDownloadedPackage>;
}

#[derive(Clone, Debug, Default)]
pub struct ReqwestPluginMarketPackageFetcher {
    client: reqwest::Client,
}

impl ReqwestPluginMarketPackageFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl PluginMarketPackageFetcher for ReqwestPluginMarketPackageFetcher {
    async fn fetch_package(
        &self,
        package: &PluginPackageDescriptor,
        context: &PluginMarketExecutionContext,
    ) -> Result<PluginMarketDownloadedPackage> {
        fs::create_dir_all(&context.package_cache_dir)
            .map_err(io_error("create plugin package cache directory"))?;
        match &package.source {
            PluginInstallSource::Repository { url } => {
                let archive_url = derive_repository_archive_url(url)?;
                download_package(
                    &self.client,
                    &archive_url,
                    package,
                    context,
                    package_filename(package, url, "repo.zip"),
                )
                .await
            }
            PluginInstallSource::Archive { url } => {
                download_package(
                    &self.client,
                    url,
                    package,
                    context,
                    package_filename(package, url, "archive.zip"),
                )
                .await
            }
            PluginInstallSource::UploadedArchive { filename } => {
                let filename = safe_direct_filename(filename)?;
                let archive_path = context.upload_dir.join(filename);
                if !archive_path.exists() {
                    return Err(AstrbotError::Pipeline(format!(
                        "uploaded plugin archive {} does not exist",
                        archive_path.display()
                    )));
                }
                Ok(
                    PluginMarketDownloadedPackage::new(package.source.clone(), archive_path)
                        .cache_hit(),
                )
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FixturePluginMarketPackageFetcher {
    packages: Arc<Mutex<VecDeque<PluginMarketDownloadedPackage>>>,
}

impl FixturePluginMarketPackageFetcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_package(&self, package: PluginMarketDownloadedPackage) {
        self.packages
            .lock()
            .expect("fixture plugin market package lock")
            .push_back(package);
    }
}

#[async_trait]
impl PluginMarketPackageFetcher for FixturePluginMarketPackageFetcher {
    async fn fetch_package(
        &self,
        _package: &PluginPackageDescriptor,
        _context: &PluginMarketExecutionContext,
    ) -> Result<PluginMarketDownloadedPackage> {
        self.packages
            .lock()
            .expect("fixture plugin market package lock")
            .pop_front()
            .ok_or_else(|| AstrbotError::Pipeline("fixture package queue is empty".to_string()))
    }
}

#[async_trait]
pub trait PluginMarketRuntimeReloader: Send + Sync {
    async fn reload_plugin(&self, plugin_id: &str, plugin_dir: &Path) -> Result<()>;
    async fn unload_plugin(&self, plugin_id: &str) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopPluginMarketRuntimeReloader;

#[async_trait]
impl PluginMarketRuntimeReloader for NoopPluginMarketRuntimeReloader {
    async fn reload_plugin(&self, _plugin_id: &str, _plugin_dir: &Path) -> Result<()> {
        Ok(())
    }

    async fn unload_plugin(&self, _plugin_id: &str) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginMarketExecutionStatus {
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketStepRecord {
    pub step: String,
    pub success: bool,
    pub message: String,
}

impl PluginMarketStepRecord {
    fn success(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            step: step.into(),
            success: true,
            message: message.into(),
        }
    }

    fn failure(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            step: step.into(),
            success: false,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketInstalledMetadata {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub readme: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketFailureRecord {
    pub plugin_id: String,
    pub action: PluginMarketAction,
    pub message: String,
    pub rollback_hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketExecutionResult {
    pub plugin_id: String,
    pub action: PluginMarketAction,
    pub status: PluginMarketExecutionStatus,
    pub plan: PluginMarketOperationPlan,
    pub installed_dir: Option<PathBuf>,
    pub downloaded_archive: Option<PathBuf>,
    pub metadata: Option<PluginMarketInstalledMetadata>,
    pub dependency_outcome: Option<DependencyInstallOutcomeSummary>,
    pub removed_config: bool,
    pub removed_data: bool,
    pub reloaded: bool,
    pub rollback_hint: Option<String>,
    pub failure: Option<PluginMarketFailureRecord>,
    pub steps: Vec<PluginMarketStepRecord>,
}

impl PluginMarketExecutionResult {
    fn completed(plan: PluginMarketOperationPlan, steps: Vec<PluginMarketStepRecord>) -> Self {
        Self {
            plugin_id: plan.plugin_id.clone(),
            action: plan.action,
            status: PluginMarketExecutionStatus::Completed,
            plan,
            installed_dir: None,
            downloaded_archive: None,
            metadata: None,
            dependency_outcome: None,
            removed_config: false,
            removed_data: false,
            reloaded: false,
            rollback_hint: None,
            failure: None,
            steps,
        }
    }

    fn failed(
        plan: PluginMarketOperationPlan,
        message: String,
        rollback_hint: Option<String>,
        mut steps: Vec<PluginMarketStepRecord>,
    ) -> Self {
        steps.push(PluginMarketStepRecord::failure(
            "operation",
            message.clone(),
        ));
        Self {
            plugin_id: plan.plugin_id.clone(),
            action: plan.action,
            status: PluginMarketExecutionStatus::Failed,
            installed_dir: None,
            downloaded_archive: None,
            metadata: None,
            dependency_outcome: None,
            removed_config: false,
            removed_data: false,
            reloaded: false,
            failure: Some(PluginMarketFailureRecord {
                plugin_id: plan.plugin_id.clone(),
                action: plan.action,
                message,
                rollback_hint: rollback_hint.clone(),
            }),
            rollback_hint,
            plan,
            steps,
        }
    }

    pub fn is_success(&self) -> bool {
        self.status == PluginMarketExecutionStatus::Completed
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyInstallOutcomeSummary {
    pub plugin_id: String,
    pub status: String,
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub conflicts: Vec<DependencyConflictReport>,
}

impl From<&DependencyInstallOutcome> for DependencyInstallOutcomeSummary {
    fn from(outcome: &DependencyInstallOutcome) -> Self {
        Self {
            plugin_id: outcome.plugin_id.clone(),
            status: match outcome.status {
                DependencyInstallStatus::Completed => "completed",
                DependencyInstallStatus::Skipped => "skipped",
                DependencyInstallStatus::Failed => "failed",
            }
            .to_string(),
            installed: dependency_names(outcome.installed()),
            skipped: dependency_names(outcome.skipped_dependencies()),
            conflicts: outcome.conflicts().to_vec(),
        }
    }
}

pub struct PluginMarketExecutor<
    F = ReqwestPluginMarketPackageFetcher,
    D = NoopDependencyInstaller,
    R = NoopPluginMarketRuntimeReloader,
> {
    context: PluginMarketExecutionContext,
    fetcher: F,
    dependency_installer: D,
    reloader: R,
}

impl PluginMarketExecutor {
    pub fn new(context: PluginMarketExecutionContext) -> Self {
        Self {
            context,
            fetcher: ReqwestPluginMarketPackageFetcher::new(),
            dependency_installer: NoopDependencyInstaller,
            reloader: NoopPluginMarketRuntimeReloader,
        }
    }
}

impl<F, D, R> PluginMarketExecutor<F, D, R>
where
    F: PluginMarketPackageFetcher,
    D: PluginDependencyPlanInstaller,
    R: PluginMarketRuntimeReloader,
{
    pub fn with_parts(
        context: PluginMarketExecutionContext,
        fetcher: F,
        dependency_installer: D,
        reloader: R,
    ) -> Self {
        Self {
            context,
            fetcher,
            dependency_installer,
            reloader,
        }
    }

    pub fn context(&self) -> &PluginMarketExecutionContext {
        &self.context
    }

    pub fn plan_install_from_cache(
        &self,
        cache: &PluginMarketCache,
        plugin_id: &str,
        options: &PluginMarketExecutionOptions,
    ) -> Result<PluginMarketOperationPlan> {
        let entry = cache.entry(plugin_id).ok_or_else(|| {
            AstrbotError::Pipeline(format!("plugin {plugin_id} is not in market cache"))
        })?;
        let mut plan = PluginMarketOperationPlan::from_market_entry(entry).ok_or_else(|| {
            AstrbotError::Pipeline(format!("plugin {plugin_id} has no install source"))
        })?;
        if options.ignore_compatibility {
            plan = plan.ignoring_compatibility();
        }
        Ok(plan)
    }

    pub fn plan_update_from_cache(
        &self,
        cache: &PluginMarketCache,
        plugin_id: &str,
        options: &PluginMarketExecutionOptions,
    ) -> Result<PluginMarketOperationPlan> {
        let entry = cache.entry(plugin_id).ok_or_else(|| {
            AstrbotError::Pipeline(format!("plugin {plugin_id} is not in market cache"))
        })?;
        let mut plan =
            PluginMarketOperationPlan::update_from_market_entry(entry).ok_or_else(|| {
                AstrbotError::Pipeline(format!("plugin {plugin_id} has no update source"))
            })?;
        if options.ignore_compatibility {
            plan = plan.ignoring_compatibility();
        }
        Ok(plan)
    }

    pub async fn install_from_cache(
        &self,
        cache: &PluginMarketCache,
        plugin_id: &str,
        options: PluginMarketExecutionOptions,
    ) -> PluginMarketExecutionResult {
        match self.plan_install_from_cache(cache, plugin_id, &options) {
            Ok(plan) => self.execute(plan, options).await,
            Err(err) => PluginMarketExecutionResult::failed(
                PluginMarketOperationPlan::uninstall(PluginUninstallPlan::new(plugin_id)),
                err.to_string(),
                None,
                Vec::new(),
            ),
        }
    }

    pub async fn update_from_cache(
        &self,
        cache: &PluginMarketCache,
        plugin_id: &str,
        options: PluginMarketExecutionOptions,
    ) -> PluginMarketExecutionResult {
        match self.plan_update_from_cache(cache, plugin_id, &options) {
            Ok(plan) => self.execute(plan, options).await,
            Err(err) => PluginMarketExecutionResult::failed(
                PluginMarketOperationPlan::uninstall(PluginUninstallPlan::new(plugin_id)),
                err.to_string(),
                None,
                Vec::new(),
            ),
        }
    }

    pub async fn execute(
        &self,
        plan: PluginMarketOperationPlan,
        options: PluginMarketExecutionOptions,
    ) -> PluginMarketExecutionResult {
        match plan.action {
            PluginMarketAction::Install => self.execute_install_or_update(plan, options).await,
            PluginMarketAction::Update => self.execute_install_or_update(plan, options).await,
            PluginMarketAction::Uninstall => self.execute_uninstall(plan, options).await,
        }
    }

    async fn execute_install_or_update(
        &self,
        plan: PluginMarketOperationPlan,
        options: PluginMarketExecutionOptions,
    ) -> PluginMarketExecutionResult {
        let mut steps = Vec::new();
        if plan.is_blocked_by_compatibility() && !options.ignore_compatibility {
            return PluginMarketExecutionResult::failed(
                plan.clone(),
                plan.compatibility.message.clone().unwrap_or_else(|| {
                    "plugin is incompatible with current AstrBot version".to_string()
                }),
                Some(
                    "rerun with ignore_compatibility only if the operator accepts the risk"
                        .to_string(),
                ),
                steps,
            );
        }
        steps.push(PluginMarketStepRecord::success(
            "compatibility",
            if plan.compatibility.compatible {
                "plugin compatibility accepted"
            } else {
                "plugin compatibility warning ignored"
            },
        ));

        let package = match plan.package.clone() {
            Some(package) => package,
            None => {
                return PluginMarketExecutionResult::failed(
                    plan,
                    "plugin market operation is missing package source".to_string(),
                    None,
                    steps,
                );
            }
        };

        let backup_dir = (plan.action == PluginMarketAction::Update).then(|| {
            self.context
                .plugin_store_dir
                .join(format!("{}.rollback", plan.plugin_id))
        });
        if let Some(backup_dir) = &backup_dir {
            if let Err(err) =
                prepare_update_backup(&self.context.plugin_dir(&plan.plugin_id), backup_dir)
            {
                let rollback_hint = Some(format!(
                    "existing plugin remains at {}",
                    self.context.plugin_dir(&plan.plugin_id).display()
                ));
                return PluginMarketExecutionResult::failed(
                    plan,
                    err.to_string(),
                    rollback_hint,
                    steps,
                );
            }
            steps.push(PluginMarketStepRecord::success(
                "backup",
                format!("rollback backup prepared at {}", backup_dir.display()),
            ));
        }

        let downloaded = match self.fetcher.fetch_package(&package, &self.context).await {
            Ok(downloaded) => downloaded,
            Err(err) => {
                return PluginMarketExecutionResult::failed(
                    plan,
                    err.to_string(),
                    backup_dir.map(|path| format!("restore from {}", path.display())),
                    steps,
                );
            }
        };
        steps.push(PluginMarketStepRecord::success(
            "download",
            format!(
                "package resolved at {}{}",
                downloaded.archive_path.display(),
                if downloaded.cache_hit {
                    " from cache"
                } else {
                    ""
                }
            ),
        ));

        if let Some(expected) = package.checksum_md5.as_deref() {
            match verify_md5(&downloaded.archive_path, expected) {
                Ok(actual) => steps.push(PluginMarketStepRecord::success(
                    "checksum",
                    format!("md5 checksum matched {actual}"),
                )),
                Err(err) => {
                    return PluginMarketExecutionResult::failed(
                        plan,
                        err.to_string(),
                        backup_dir.map(|path| format!("restore from {}", path.display())),
                        steps,
                    );
                }
            }
        }

        let plugin_dir = self.context.plugin_dir(&plan.plugin_id);
        if let Err(err) = replace_plugin_from_archive(&downloaded.archive_path, &plugin_dir) {
            restore_backup_if_needed(&plugin_dir, backup_dir.as_deref(), &mut steps);
            return PluginMarketExecutionResult::failed(
                plan,
                err.to_string(),
                backup_dir.map(|path| format!("restore from {}", path.display())),
                steps,
            );
        }
        steps.push(PluginMarketStepRecord::success(
            "unpack",
            format!("plugin unpacked into {}", plugin_dir.display()),
        ));

        let metadata = match read_installed_metadata(&plan.plugin_id, &plugin_dir) {
            Ok(metadata) => {
                steps.push(PluginMarketStepRecord::success(
                    "metadata",
                    format!(
                        "loaded plugin metadata {} {}",
                        metadata.name, metadata.version
                    ),
                ));
                Some(metadata)
            }
            Err(err) => {
                restore_backup_if_needed(&plugin_dir, backup_dir.as_deref(), &mut steps);
                return PluginMarketExecutionResult::failed(
                    plan,
                    err.to_string(),
                    backup_dir.map(|path| format!("restore from {}", path.display())),
                    steps,
                );
            }
        };

        let dependency_plan = match read_requirements_plan(&plan.plugin_id, &plugin_dir) {
            Ok(dependency_plan) => dependency_plan,
            Err(err) => {
                restore_backup_if_needed(&plugin_dir, backup_dir.as_deref(), &mut steps);
                return PluginMarketExecutionResult::failed(
                    plan,
                    err.to_string(),
                    backup_dir.map(|path| format!("restore from {}", path.display())),
                    steps,
                );
            }
        };
        let dependency_outcome = if dependency_plan.is_empty() {
            steps.push(PluginMarketStepRecord::success(
                "requirements",
                "plugin has no requirements.txt dependencies",
            ));
            None
        } else {
            let environment = self.import_environment(&plan.plugin_id, &plugin_dir, &options);
            match self
                .dependency_installer
                .install_dependencies(DependencyInstallRequest::new(dependency_plan, environment))
                .await
            {
                Ok(outcome) if outcome.is_success() => {
                    steps.push(PluginMarketStepRecord::success(
                        "requirements",
                        "plugin requirements precheck completed",
                    ));
                    Some(DependencyInstallOutcomeSummary::from(&outcome))
                }
                Ok(outcome) => {
                    let summary = DependencyInstallOutcomeSummary::from(&outcome);
                    restore_backup_if_needed(&plugin_dir, backup_dir.as_deref(), &mut steps);
                    let mut result = PluginMarketExecutionResult::failed(
                        plan,
                        "plugin dependency installation failed".to_string(),
                        backup_dir.map(|path| format!("restore from {}", path.display())),
                        steps,
                    );
                    result.dependency_outcome = Some(summary);
                    return result;
                }
                Err(err) => {
                    restore_backup_if_needed(&plugin_dir, backup_dir.as_deref(), &mut steps);
                    return PluginMarketExecutionResult::failed(
                        plan,
                        err.to_string(),
                        backup_dir.map(|path| format!("restore from {}", path.display())),
                        steps,
                    );
                }
            }
        };

        let mut result = PluginMarketExecutionResult::completed(plan.clone(), steps);
        result.installed_dir = Some(plugin_dir.clone());
        result.downloaded_archive = Some(downloaded.archive_path.clone());
        result.metadata = metadata;
        result.dependency_outcome = dependency_outcome;

        if options.reload_after_operation && plan.requires_loader_reload {
            match self
                .reloader
                .reload_plugin(&plan.plugin_id, &plugin_dir)
                .await
            {
                Ok(()) => {
                    result.reloaded = true;
                    result.steps.push(PluginMarketStepRecord::success(
                        "reload",
                        "plugin hot reload completed",
                    ));
                }
                Err(err) => {
                    restore_backup_if_needed(&plugin_dir, backup_dir.as_deref(), &mut result.steps);
                    return PluginMarketExecutionResult::failed(
                        plan,
                        format!("plugin hot reload failed: {err}"),
                        Some("plugin files were unpacked; restore rollback backup or retry reload after fixing the error".to_string()),
                        result.steps,
                    );
                }
            }
        }

        if let Some(backup_dir) = backup_dir
            && backup_dir.exists()
        {
            if let Err(err) = fs::remove_dir_all(&backup_dir) {
                result.steps.push(PluginMarketStepRecord::failure(
                    "rollback_cleanup",
                    format!(
                        "failed to remove rollback backup {}: {err}",
                        backup_dir.display()
                    ),
                ));
            }
        }
        if options.clean_downloaded_archive
            && !downloaded.cache_hit
            && let Err(err) = fs::remove_file(&downloaded.archive_path)
        {
            result.steps.push(PluginMarketStepRecord::failure(
                "archive_cleanup",
                format!(
                    "failed to remove downloaded archive {}: {err}",
                    downloaded.archive_path.display()
                ),
            ));
        }
        result
    }

    async fn execute_uninstall(
        &self,
        plan: PluginMarketOperationPlan,
        options: PluginMarketExecutionOptions,
    ) -> PluginMarketExecutionResult {
        let mut steps = Vec::new();
        if options.reload_after_operation && plan.requires_loader_reload {
            match self.reloader.unload_plugin(&plan.plugin_id).await {
                Ok(()) => steps.push(PluginMarketStepRecord::success(
                    "unload",
                    "plugin runtime unload completed",
                )),
                Err(err) => steps.push(PluginMarketStepRecord::failure(
                    "unload",
                    format!("plugin runtime unload failed: {err}"),
                )),
            }
        }

        let plugin_dir = self.context.plugin_dir(&plan.plugin_id);
        if let Err(err) = remove_path_if_exists(&plugin_dir) {
            return PluginMarketExecutionResult::failed(
                plan,
                err.to_string(),
                Some(format!(
                    "runtime may be unloaded; manually remove {} if needed",
                    plugin_dir.display()
                )),
                steps,
            );
        }
        steps.push(PluginMarketStepRecord::success(
            "remove_plugin",
            format!("removed plugin directory {}", plugin_dir.display()),
        ));

        let mut result = PluginMarketExecutionResult::completed(plan.clone(), steps);
        if plan.delete_config {
            let config_path = self.context.config_path(&plan.plugin_id);
            match remove_path_if_exists(&config_path) {
                Ok(()) => {
                    result.removed_config = true;
                    result.steps.push(PluginMarketStepRecord::success(
                        "remove_config",
                        format!("removed plugin config {}", config_path.display()),
                    ));
                }
                Err(err) => {
                    return PluginMarketExecutionResult::failed(
                        plan,
                        err.to_string(),
                        Some(format!(
                            "plugin files removed; manually inspect {}",
                            config_path.display()
                        )),
                        result.steps,
                    );
                }
            }
        }
        if plan.delete_data {
            let data_path = self.context.data_path(&plan.plugin_id);
            match remove_path_if_exists(&data_path) {
                Ok(()) => {
                    result.removed_data = true;
                    result.steps.push(PluginMarketStepRecord::success(
                        "remove_data",
                        format!("removed plugin data {}", data_path.display()),
                    ));
                }
                Err(err) => {
                    return PluginMarketExecutionResult::failed(
                        plan,
                        err.to_string(),
                        Some(format!(
                            "plugin files removed; manually inspect {}",
                            data_path.display()
                        )),
                        result.steps,
                    );
                }
            }
        }
        result
    }

    fn import_environment(
        &self,
        plugin_id: &str,
        plugin_dir: &Path,
        options: &PluginMarketExecutionOptions,
    ) -> PluginImportEnvironment {
        let mut environment = PluginImportEnvironment::python_compat(plugin_id)
            .with_plugin_root(plugin_dir.to_path_buf())
            .with_isolated_dependency_root(self.context.dependency_root(plugin_id));
        for site_packages_root in &self.context.site_packages_roots {
            environment = environment.with_site_packages_root(site_packages_root.clone());
        }
        if options.prefer_installed_site_packages && !self.context.site_packages_roots.is_empty() {
            environment = environment
                .with_package_preference(PackagePreferencePolicy::PreferInstalledSitePackages);
        }
        environment
    }
}

pub fn derive_repository_archive_url(repo_url: &str) -> Result<String> {
    let cleaned = repo_url
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let parts = cleaned.split('/').collect::<Vec<_>>();
    let github_index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("github.com"))
        .ok_or_else(|| {
            AstrbotError::Pipeline(format!(
                "repository source {repo_url} is not a supported GitHub URL"
            ))
        })?;
    let author = parts.get(github_index + 1).ok_or_else(|| {
        AstrbotError::Pipeline(format!("repository source {repo_url} is missing owner"))
    })?;
    let repo = parts.get(github_index + 2).ok_or_else(|| {
        AstrbotError::Pipeline(format!(
            "repository source {repo_url} is missing repository"
        ))
    })?;
    let branch = if parts.get(github_index + 3) == Some(&"tree") {
        parts.get(github_index + 4).copied()
    } else {
        None
    };
    Ok(match branch {
        Some(branch) if !branch.trim().is_empty() => {
            format!("https://github.com/{author}/{repo}/archive/refs/heads/{branch}.zip")
        }
        _ => format!("https://github.com/{author}/{repo}/archive/refs/heads/master.zip"),
    })
}

async fn download_package(
    client: &reqwest::Client,
    url: &str,
    package: &PluginPackageDescriptor,
    context: &PluginMarketExecutionContext,
    filename: String,
) -> Result<PluginMarketDownloadedPackage> {
    let archive_path = context
        .package_cache_dir
        .join(safe_direct_filename(&filename)?);
    if archive_path.exists() && package.checksum_md5.is_some() {
        return Ok(
            PluginMarketDownloadedPackage::new(package.source.clone(), archive_path).cache_hit(),
        );
    }
    let response =
        client.get(url).send().await.map_err(|err| {
            AstrbotError::Pipeline(format!("download plugin package {url}: {err}"))
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(AstrbotError::Pipeline(format!(
            "download plugin package {url} failed with HTTP {status}"
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| AstrbotError::Pipeline(format!("read plugin package {url}: {err}")))?;
    fs::write(&archive_path, &bytes).map_err(io_error("write plugin package archive"))?;
    Ok(PluginMarketDownloadedPackage::new(
        package.source.clone(),
        archive_path,
    ))
}

fn package_filename(package: &PluginPackageDescriptor, source: &str, fallback: &str) -> String {
    package.cache_key.clone().unwrap_or_else(|| {
        source
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback)
            .to_string()
    })
}

fn replace_plugin_from_archive(archive_path: &Path, plugin_dir: &Path) -> Result<()> {
    if plugin_dir.exists() {
        fs::remove_dir_all(plugin_dir).map_err(io_error("remove old plugin directory"))?;
    }
    fs::create_dir_all(plugin_dir).map_err(io_error("create plugin directory"))?;
    unzip_archive_flatten_first_dir(archive_path, plugin_dir)
}

fn unzip_archive_flatten_first_dir(archive_path: &Path, plugin_dir: &Path) -> Result<()> {
    let file = File::open(archive_path).map_err(io_error("open plugin archive"))?;
    let mut archive = ZipArchive::new(file).map_err(zip_error("open plugin archive"))?;
    validate_zip_entries(&mut archive)?;
    let root_prefix = first_entry_root(&mut archive)?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(zip_error("read plugin archive entry"))?;
        let Some(relative_path) = archive_relative_path(file.name(), root_prefix.as_deref()) else {
            continue;
        };
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        let target_path = plugin_dir.join(relative_path);
        if file.is_dir() {
            fs::create_dir_all(&target_path)
                .map_err(io_error("create plugin archive directory"))?;
            continue;
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(io_error("create plugin archive parent"))?;
        }
        let mut output =
            File::create(&target_path).map_err(io_error("create plugin archive file"))?;
        io::copy(&mut file, &mut output).map_err(io_error("write plugin archive file"))?;
    }
    Ok(())
}

fn validate_zip_entries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<()> {
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(zip_error("read plugin archive entry"))?;
        validate_archive_path(file.name())?;
    }
    Ok(())
}

fn first_entry_root<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Option<String>> {
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(zip_error("read plugin archive entry"))?;
        let name = file.name().trim_end_matches('/');
        if name.is_empty() {
            continue;
        }
        let first = name.split('/').next().unwrap_or_default();
        return Ok((name != first).then(|| first.to_string()));
    }
    Ok(None)
}

fn archive_relative_path(name: &str, root_prefix: Option<&str>) -> Option<PathBuf> {
    let name = name.trim_end_matches('/');
    let relative = match root_prefix {
        Some(prefix) => name.strip_prefix(prefix)?.trim_start_matches('/'),
        None => name,
    };
    Some(PathBuf::from(relative))
}

fn validate_archive_path(path: &str) -> Result<()> {
    if path.trim().is_empty() || path.contains('\\') || path.starts_with('/') {
        return Err(AstrbotError::Pipeline(format!(
            "plugin archive path {path:?} is unsafe"
        )));
    }
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(AstrbotError::Pipeline(format!(
                    "plugin archive path {path:?} is unsafe"
                )));
            }
        }
    }
    Ok(())
}

fn read_installed_metadata(
    plugin_id: &str,
    plugin_dir: &Path,
) -> Result<PluginMarketInstalledMetadata> {
    let metadata_path = plugin_dir.join("metadata.yaml");
    if metadata_path.exists() {
        let manifest = read_metadata_yaml(&metadata_path)?;
        return Ok(PluginMarketInstalledMetadata {
            plugin_id: plugin_id.to_string(),
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            authors: manifest.authors,
            readme: read_readme(plugin_dir),
        });
    }

    let metadata_path = plugin_dir.join("metadata.json");
    if metadata_path.exists() {
        let content = fs::read_to_string(&metadata_path).map_err(io_error("read metadata.json"))?;
        let value = serde_json::from_str::<serde_json::Value>(&content).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "failed to parse plugin metadata {}: {err}",
                metadata_path.display()
            ))
        })?;
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(plugin_id)
            .to_string();
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let description = value
            .get("desc")
            .or_else(|| value.get("description"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        let authors = value
            .get("author")
            .or_else(|| value.get("authors"))
            .map(author_list_from_json)
            .unwrap_or_default();
        return Ok(PluginMarketInstalledMetadata {
            plugin_id: plugin_id.to_string(),
            name,
            version,
            description,
            authors,
            readme: read_readme(plugin_dir),
        });
    }

    Err(AstrbotError::Pipeline(format!(
        "plugin {} metadata.yaml or metadata.json was not found after unpack",
        plugin_dir.display()
    )))
}

fn read_metadata_yaml(path: &Path) -> Result<PluginManifest> {
    let content = fs::read_to_string(path).map_err(io_error("read plugin metadata.yaml"))?;
    let mut values = HashMap::<String, Vec<String>>::new();
    let mut current_list_key: Option<String> = None;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(item) = line.strip_prefix("- ") {
            if let Some(key) = current_list_key.as_ref() {
                push_yaml_value(values.entry(key.clone()).or_default(), item);
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = clean_yaml_value(value);
        current_list_key = value.is_empty().then_some(key.clone());
        if !value.is_empty() {
            for item in value.trim_matches(['[', ']']).split(',') {
                push_yaml_value(values.entry(key.clone()).or_default(), item);
            }
        }
    }

    let name = first_value(&values, &["name"]).ok_or_else(|| {
        AstrbotError::Pipeline(format!(
            "plugin metadata {} is missing required name",
            path.display()
        ))
    })?;
    let version = first_value(&values, &["version"]).ok_or_else(|| {
        AstrbotError::Pipeline(format!(
            "plugin metadata {} is missing required version",
            path.display()
        ))
    })?;
    let mut manifest = PluginManifest::new(name, version);
    if let Some(description) = first_value(&values, &["desc", "description"]) {
        manifest = manifest.with_description(description);
    }
    for author in values
        .get("author")
        .into_iter()
        .flatten()
        .chain(values.get("authors").into_iter().flatten())
    {
        manifest = manifest.with_author(author.clone());
    }
    Ok(manifest)
}

fn read_requirements_plan(plugin_id: &str, plugin_dir: &Path) -> Result<PluginDependencyPlan> {
    let requirements_path = plugin_dir.join("requirements.txt");
    if !requirements_path.exists() {
        return Ok(PluginDependencyPlan::new(plugin_id));
    }
    let content = fs::read_to_string(&requirements_path).map_err(io_error("read requirements"))?;
    let mut plan = PluginDependencyPlan::new(plugin_id);
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let (name, version_req) = split_requirement(line);
        let mut dependency = PluginDependency::new(PluginDependencyKind::PythonPackage, name);
        if let Some(version_req) = version_req {
            dependency = dependency.with_version_req(version_req);
        }
        plan = plan.with_dependency(dependency);
    }
    Ok(plan)
}

fn split_requirement(line: &str) -> (&str, Option<&str>) {
    for operator in ["==", ">=", "<=", "~=", "!=", ">", "<"] {
        if let Some(index) = line.find(operator) {
            let name = line[..index].trim();
            let version_req = line[index..].trim();
            return (name, (!version_req.is_empty()).then_some(version_req));
        }
    }
    (line, None)
}

fn verify_md5(path: &Path, expected: &str) -> Result<String> {
    let bytes = fs::read(path).map_err(io_error("read plugin package checksum source"))?;
    let actual = format!("{:x}", md5::compute(bytes));
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(actual)
    } else {
        Err(AstrbotError::Pipeline(format!(
            "plugin package checksum mismatch for {}: expected {}, got {}",
            path.display(),
            expected,
            actual
        )))
    }
}

fn prepare_update_backup(plugin_dir: &Path, backup_dir: &Path) -> Result<()> {
    if backup_dir.exists() {
        fs::remove_dir_all(backup_dir).map_err(io_error("remove old rollback backup"))?;
    }
    if plugin_dir.exists() {
        copy_dir_all(plugin_dir, backup_dir).map_err(io_error("create rollback backup"))?;
    }
    Ok(())
}

fn restore_backup_if_needed(
    plugin_dir: &Path,
    backup_dir: Option<&Path>,
    steps: &mut Vec<PluginMarketStepRecord>,
) {
    let Some(backup_dir) = backup_dir else {
        return;
    };
    if !backup_dir.exists() {
        return;
    }
    if plugin_dir.exists()
        && let Err(err) = fs::remove_dir_all(plugin_dir)
    {
        steps.push(PluginMarketStepRecord::failure(
            "rollback",
            format!(
                "failed to clear failed plugin directory {}: {err}",
                plugin_dir.display()
            ),
        ));
        return;
    }
    match fs::rename(backup_dir, plugin_dir) {
        Ok(()) => steps.push(PluginMarketStepRecord::success(
            "rollback",
            format!("restored plugin rollback backup {}", plugin_dir.display()),
        )),
        Err(err) => steps.push(PluginMarketStepRecord::failure(
            "rollback",
            format!(
                "failed to restore rollback backup {} to {}: {err}",
                backup_dir.display(),
                plugin_dir.display()
            ),
        )),
    }
}

fn copy_dir_all(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(io_error("remove plugin path"))
    } else {
        fs::remove_file(path).map_err(io_error("remove plugin path"))
    }
}

fn safe_direct_filename(filename: &str) -> Result<String> {
    let trimmed = filename.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(AstrbotError::Pipeline(format!(
            "plugin archive filename {filename:?} is unsafe"
        )));
    }
    Ok(trimmed.to_string())
}

fn read_readme(plugin_dir: &Path) -> Option<String> {
    ["README.md", "readme.md"]
        .iter()
        .map(|filename| plugin_dir.join(filename))
        .find(|path| path.exists())
        .and_then(|path| fs::read_to_string(path).ok())
}

fn author_list_from_json(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(ToString::to_string)
            .collect(),
        serde_json::Value::String(value) => value
            .trim_matches(['[', ']'])
            .split(',')
            .map(clean_yaml_value)
            .filter(|value| !value.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn first_value(values: &HashMap<String, Vec<String>>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| values.get(*key))
        .find_map(|values| values.first().cloned())
}

fn push_yaml_value(values: &mut Vec<String>, value: &str) {
    let value = clean_yaml_value(value);
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}

fn clean_yaml_value(value: &str) -> String {
    value.trim().trim_matches(['"', '\'']).trim().to_string()
}

fn dependency_names(dependencies: &[PluginDependency]) -> Vec<String> {
    dependencies
        .iter()
        .map(|dependency| match dependency.version_req.as_deref() {
            Some(version_req) => format!("{}{}", dependency.name, version_req),
            None => dependency.name.clone(),
        })
        .collect()
}

fn zip_error(context: &'static str) -> impl FnOnce(zip::result::ZipError) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("zip {context}: {err}"))
}

fn io_error(context: &'static str) -> impl FnOnce(std::io::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("plugin market {context}: {err}"))
}
