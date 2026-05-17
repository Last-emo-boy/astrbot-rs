use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MaintenanceOperationId(String);

impl MaintenanceOperationId {
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let id = id.into();
        (!id.trim().is_empty()).then_some(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceOperationKind {
    ProjectUpdate,
    DashboardUpdate,
    PackageInstall,
    Migration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceOperationStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceOperationEvent {
    pub message: String,
    pub percent: Option<u8>,
}

impl MaintenanceOperationEvent {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            percent: None,
        }
    }

    pub fn with_percent(mut self, percent: u8) -> Self {
        self.percent = Some(percent.min(100));
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceOperationProgress {
    pub status: MaintenanceOperationStatus,
    pub events: Vec<MaintenanceOperationEvent>,
    pub error: Option<String>,
}

impl MaintenanceOperationProgress {
    pub fn queued() -> Self {
        Self {
            status: MaintenanceOperationStatus::Queued,
            events: Vec::new(),
            error: None,
        }
    }

    pub fn running(mut self, event: impl Into<String>) -> Self {
        self.status = MaintenanceOperationStatus::Running;
        self.events.push(MaintenanceOperationEvent::new(event));
        self
    }

    pub fn completed(mut self, event: impl Into<String>) -> Self {
        self.status = MaintenanceOperationStatus::Completed;
        self.events
            .push(MaintenanceOperationEvent::new(event).with_percent(100));
        self.error = None;
        self
    }

    pub fn failed(mut self, error: impl Into<String>) -> Self {
        self.status = MaintenanceOperationStatus::Failed;
        self.error = Some(error.into());
        self
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            MaintenanceOperationStatus::Completed
                | MaintenanceOperationStatus::Failed
                | MaintenanceOperationStatus::Cancelled
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceOperationSummary {
    pub operation_id: MaintenanceOperationId,
    pub kind: MaintenanceOperationKind,
    pub progress: MaintenanceOperationProgress,
}

impl MaintenanceOperationSummary {
    pub fn new(operation_id: MaintenanceOperationId, kind: MaintenanceOperationKind) -> Self {
        Self {
            operation_id,
            kind,
            progress: MaintenanceOperationProgress::queued(),
        }
    }

    pub fn with_progress(mut self, progress: MaintenanceOperationProgress) -> Self {
        self.progress = progress;
        self
    }
}

#[async_trait]
pub trait MaintenanceOperationStore: Send + Sync {
    async fn put_operation(&self, summary: MaintenanceOperationSummary) -> Result<()>;

    async fn get_operation(
        &self,
        operation_id: &MaintenanceOperationId,
    ) -> Result<Option<MaintenanceOperationSummary>>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryMaintenanceOperationStore {
    operations: Arc<RwLock<Vec<MaintenanceOperationSummary>>>,
}

impl InMemoryMaintenanceOperationStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MaintenanceOperationStore for InMemoryMaintenanceOperationStore {
    async fn put_operation(&self, summary: MaintenanceOperationSummary) -> Result<()> {
        let mut operations = self
            .operations
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("maintenance store lock: {err}")))?;
        if let Some(existing) = operations
            .iter_mut()
            .find(|current| current.operation_id == summary.operation_id)
        {
            *existing = summary;
        } else {
            operations.push(summary);
        }
        Ok(())
    }

    async fn get_operation(
        &self,
        operation_id: &MaintenanceOperationId,
    ) -> Result<Option<MaintenanceOperationSummary>> {
        self.operations
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("maintenance store lock: {err}")))
            .map(|operations| {
                operations
                    .iter()
                    .find(|summary| &summary.operation_id == operation_id)
                    .cloned()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InMemoryMaintenanceOperationStore, MaintenanceOperationId, MaintenanceOperationKind,
        MaintenanceOperationProgress, MaintenanceOperationStatus, MaintenanceOperationStore,
        MaintenanceOperationSummary,
    };

    #[tokio::test]
    async fn operation_store_updates_progress_for_dashboard_polling() {
        let store = InMemoryMaintenanceOperationStore::new();
        let operation_id = MaintenanceOperationId::new("op-1").expect("id");
        let summary = MaintenanceOperationSummary::new(
            operation_id.clone(),
            MaintenanceOperationKind::ProjectUpdate,
        )
        .with_progress(
            MaintenanceOperationProgress::queued()
                .running("downloading")
                .completed("done"),
        );

        store
            .put_operation(summary)
            .await
            .expect("operation should store");
        let loaded = store
            .get_operation(&operation_id)
            .await
            .expect("operation should load")
            .expect("operation");

        assert_eq!(loaded.progress.status, MaintenanceOperationStatus::Completed);
        assert!(loaded.progress.is_terminal());
    }
}
