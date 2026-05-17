use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::operation::{
    MaintenanceOperationId, MaintenanceOperationKind, MaintenanceOperationProgress,
    MaintenanceOperationStore, MaintenanceOperationSummary,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseMetadata {
    pub version: String,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

impl ReleaseMetadata {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            title: None,
            notes: None,
            published_at: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = non_empty(title);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseUpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub has_new_version: bool,
    pub dashboard_version: Option<String>,
    pub dashboard_has_new_version: bool,
}

impl ReleaseUpdateCheck {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            current_version: current_version.into(),
            latest_version: None,
            has_new_version: false,
            dashboard_version: None,
            dashboard_has_new_version: false,
        }
    }

    pub fn with_latest_version(mut self, latest_version: impl Into<String>) -> Self {
        let latest_version = latest_version.into();
        self.has_new_version = normalize_version(&latest_version) != normalize_version(&self.current_version);
        self.latest_version = Some(latest_version);
        self
    }

    pub fn with_dashboard_version(mut self, dashboard_version: impl Into<String>) -> Self {
        let dashboard_version = dashboard_version.into();
        self.dashboard_has_new_version =
            normalize_version(&dashboard_version) != normalize_version(&self.current_version);
        self.dashboard_version = Some(dashboard_version);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectUpdatePlan {
    pub version: Option<String>,
    pub latest: bool,
    pub proxy: Option<String>,
    pub update_dashboard: bool,
    pub install_requirements: bool,
    pub reboot: bool,
}

impl ProjectUpdatePlan {
    pub fn latest() -> Self {
        Self {
            version: None,
            latest: true,
            proxy: None,
            update_dashboard: true,
            install_requirements: true,
            reboot: true,
        }
    }

    pub fn version(version: impl Into<String>) -> Self {
        Self {
            version: non_empty(version),
            latest: false,
            ..Self::latest()
        }
    }

    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = non_empty(proxy).map(|value| value.trim_end_matches('/').to_string());
        self
    }

    pub fn with_reboot(mut self, reboot: bool) -> Self {
        self.reboot = reboot;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardUpdatePlan {
    pub version: String,
    pub latest: bool,
    pub proxy: Option<String>,
    pub clear_site_data: bool,
}

impl DashboardUpdatePlan {
    pub fn for_runtime_version(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            latest: false,
            proxy: None,
            clear_site_data: true,
        }
    }

    pub fn latest() -> Self {
        Self {
            version: String::new(),
            latest: true,
            proxy: None,
            clear_site_data: true,
        }
    }
}

#[async_trait]
pub trait ReleaseUpdateService: Send + Sync {
    async fn check_updates(&self) -> Result<ReleaseUpdateCheck>;

    async fn releases(&self) -> Result<Vec<ReleaseMetadata>>;

    async fn plan_project_update(&self, plan: ProjectUpdatePlan) -> Result<MaintenanceOperationSummary>;

    async fn plan_dashboard_update(
        &self,
        plan: DashboardUpdatePlan,
    ) -> Result<MaintenanceOperationSummary>;
}

pub struct PlannedReleaseUpdateService<'a> {
    operations: &'a dyn MaintenanceOperationStore,
    current_version: String,
    latest_version: Option<String>,
    dashboard_version: Option<String>,
}

impl<'a> PlannedReleaseUpdateService<'a> {
    pub fn new(
        operations: &'a dyn MaintenanceOperationStore,
        current_version: impl Into<String>,
    ) -> Self {
        Self {
            operations,
            current_version: current_version.into(),
            latest_version: None,
            dashboard_version: None,
        }
    }

    pub fn with_latest_version(mut self, latest_version: impl Into<String>) -> Self {
        self.latest_version = non_empty(latest_version);
        self
    }

    pub fn with_dashboard_version(mut self, dashboard_version: impl Into<String>) -> Self {
        self.dashboard_version = non_empty(dashboard_version);
        self
    }
}

#[async_trait]
impl ReleaseUpdateService for PlannedReleaseUpdateService<'_> {
    async fn check_updates(&self) -> Result<ReleaseUpdateCheck> {
        let mut check = ReleaseUpdateCheck::new(self.current_version.clone());
        if let Some(latest_version) = &self.latest_version {
            check = check.with_latest_version(latest_version.clone());
        }
        if let Some(dashboard_version) = &self.dashboard_version {
            check = check.with_dashboard_version(dashboard_version.clone());
        }
        Ok(check)
    }

    async fn releases(&self) -> Result<Vec<ReleaseMetadata>> {
        Ok(self
            .latest_version
            .iter()
            .cloned()
            .map(ReleaseMetadata::new)
            .collect())
    }

    async fn plan_project_update(&self, plan: ProjectUpdatePlan) -> Result<MaintenanceOperationSummary> {
        let version_label = plan
            .version
            .clone()
            .unwrap_or_else(|| "latest".to_string());
        let operation_id = MaintenanceOperationId::new(format!("project-update-{version_label}"))
            .ok_or_else(|| AstrbotError::Pipeline("invalid project update operation id".to_string()))?;
        let mut progress = MaintenanceOperationProgress::queued()
            .running("project update planned");
        if plan.update_dashboard {
            progress = progress.running("dashboard update planned");
        }
        if plan.install_requirements {
            progress = progress.running("requirements install planned");
        }
        if plan.reboot {
            progress = progress.running("runtime reboot required");
        }
        let summary = MaintenanceOperationSummary::new(
            operation_id,
            MaintenanceOperationKind::ProjectUpdate,
        )
        .with_progress(progress);
        self.operations.put_operation(summary.clone()).await?;
        Ok(summary)
    }

    async fn plan_dashboard_update(
        &self,
        plan: DashboardUpdatePlan,
    ) -> Result<MaintenanceOperationSummary> {
        let version_label = if plan.latest {
            "latest".to_string()
        } else {
            plan.version.clone()
        };
        let operation_id = MaintenanceOperationId::new(format!("dashboard-update-{version_label}"))
            .ok_or_else(|| AstrbotError::Pipeline("invalid dashboard update operation id".to_string()))?;
        let summary = MaintenanceOperationSummary::new(
            operation_id,
            MaintenanceOperationKind::DashboardUpdate,
        )
        .with_progress(MaintenanceOperationProgress::queued().running("dashboard update planned"));
        self.operations.put_operation(summary.clone()).await?;
        Ok(summary)
    }
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn non_empty(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{PlannedReleaseUpdateService, ProjectUpdatePlan, ReleaseUpdateService};
    use crate::operation::InMemoryMaintenanceOperationStore;

    #[tokio::test]
    async fn release_check_compares_project_and_dashboard_versions() {
        let store = InMemoryMaintenanceOperationStore::new();
        let service = PlannedReleaseUpdateService::new(&store, "v4.0.0")
            .with_latest_version("v4.1.0")
            .with_dashboard_version("v4.0.0");

        let check = service.check_updates().await.expect("check");

        assert!(check.has_new_version);
        assert!(!check.dashboard_has_new_version);
    }

    #[tokio::test]
    async fn project_update_plan_records_composed_astrbot_update_steps() {
        let store = InMemoryMaintenanceOperationStore::new();
        let service = PlannedReleaseUpdateService::new(&store, "v4.0.0");

        let summary = service
            .plan_project_update(ProjectUpdatePlan::version("v4.1.0").with_reboot(true))
            .await
            .expect("plan");

        assert_eq!(summary.operation_id.as_str(), "project-update-v4.1.0");
        assert_eq!(summary.progress.events.len(), 4);
    }
}
