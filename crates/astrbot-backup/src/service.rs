use std::fmt;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    BackupExportPackage, BackupExportPort, BackupExportRequest, BackupImportMode, BackupImportPort,
    BackupImportPrecheck, BackupImportResult, BackupJobKind, BackupJobSnapshot, BackupJobStore,
    BackupManifest, BackupProgressReader, BackupProgressSnapshot,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupExportJobRequest {
    pub task_id: String,
    pub astrbot_version: String,
    pub exported_at: String,
}

impl BackupExportJobRequest {
    pub fn new(
        task_id: impl Into<String>,
        astrbot_version: impl Into<String>,
        exported_at: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            astrbot_version: astrbot_version.into(),
            exported_at: exported_at.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupImportJobRequest {
    pub task_id: String,
    pub source_id: String,
    pub mode: BackupImportMode,
}

impl BackupImportJobRequest {
    pub fn new(
        task_id: impl Into<String>,
        source_id: impl Into<String>,
        mode: BackupImportMode,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            source_id: source_id.into(),
            mode,
        }
    }
}

#[async_trait]
pub trait BackupRepositoryPort: Send + Sync {
    async fn collect_export(&self, request: &BackupExportJobRequest)
    -> Result<BackupExportRequest>;

    async fn load_import_manifest(
        &self,
        request: &BackupImportJobRequest,
    ) -> Result<BackupManifest>;
}

#[derive(Clone)]
pub struct BackupJobService {
    repository: Arc<dyn BackupRepositoryPort>,
    exporter: Arc<dyn BackupExportPort>,
    importer: Arc<dyn BackupImportPort>,
    jobs: Arc<BackupJobStore>,
}

impl BackupJobService {
    pub fn new(
        repository: Arc<dyn BackupRepositoryPort>,
        exporter: Arc<dyn BackupExportPort>,
        importer: Arc<dyn BackupImportPort>,
    ) -> Self {
        Self::with_jobs(
            repository,
            exporter,
            importer,
            Arc::new(BackupJobStore::new()),
        )
    }

    pub fn with_jobs(
        repository: Arc<dyn BackupRepositoryPort>,
        exporter: Arc<dyn BackupExportPort>,
        importer: Arc<dyn BackupImportPort>,
        jobs: Arc<BackupJobStore>,
    ) -> Self {
        Self {
            repository,
            exporter,
            importer,
            jobs,
        }
    }

    pub fn jobs(&self) -> Arc<BackupJobStore> {
        self.jobs.clone()
    }

    pub async fn start_export(&self, request: BackupExportJobRequest) -> Result<BackupJobSnapshot> {
        self.jobs.create(
            &request.task_id,
            BackupJobKind::Export,
            BackupProgressSnapshot::queued("backup export queued"),
        )?;
        match self.run_export(&request).await {
            Ok(package) => self
                .jobs
                .complete(
                    &request.task_id,
                    format!(
                        "backup export completed with {} table dumps",
                        package.tables.len()
                    ),
                )?
                .ok_or_else(|| missing_job(&request.task_id)),
            Err(error) => {
                let message = error.to_string();
                let _ = self.jobs.fail(&request.task_id, message);
                Err(error)
            }
        }
    }

    pub async fn precheck_import(
        &self,
        request: &BackupImportJobRequest,
    ) -> Result<BackupImportPrecheck> {
        let manifest = self.repository.load_import_manifest(request).await?;
        self.precheck_manifest(&manifest).await
    }

    pub async fn precheck_manifest(
        &self,
        manifest: &BackupManifest,
    ) -> Result<BackupImportPrecheck> {
        self.importer.precheck_backup(manifest).await
    }

    pub async fn start_import(&self, request: BackupImportJobRequest) -> Result<BackupJobSnapshot> {
        self.jobs.create(
            &request.task_id,
            BackupJobKind::Import,
            BackupProgressSnapshot::queued("backup import queued"),
        )?;
        match self.run_import(&request).await {
            Ok(result) => self
                .jobs
                .complete(
                    &request.task_id,
                    format!(
                        "backup import completed with {} warnings",
                        result.warnings.len()
                    ),
                )?
                .ok_or_else(|| missing_job(&request.task_id)),
            Err(error) => {
                let message = error.to_string();
                let _ = self.jobs.fail(&request.task_id, message);
                Err(error)
            }
        }
    }

    async fn run_export(&self, request: &BackupExportJobRequest) -> Result<BackupExportPackage> {
        self.jobs.update_progress(
            &request.task_id,
            BackupProgressSnapshot::running("collect", 0, 3, "collecting backup inventory"),
        )?;
        let export_request = self.repository.collect_export(request).await?;

        self.jobs.update_progress(
            &request.task_id,
            BackupProgressSnapshot::running("archive", 1, 3, "building backup archive"),
        )?;
        let package = self.exporter.export_backup(export_request).await?;

        self.jobs.update_progress(
            &request.task_id,
            BackupProgressSnapshot::running("manifest", 2, 3, "writing backup manifest"),
        )?;
        Ok(package)
    }

    async fn run_import(&self, request: &BackupImportJobRequest) -> Result<BackupImportResult> {
        self.jobs.update_progress(
            &request.task_id,
            BackupProgressSnapshot::running("manifest", 0, 3, "loading backup manifest"),
        )?;
        let manifest = self.repository.load_import_manifest(request).await?;

        self.jobs.update_progress(
            &request.task_id,
            BackupProgressSnapshot::running("precheck", 1, 3, "checking backup compatibility"),
        )?;
        let precheck = self.importer.precheck_backup(&manifest).await?;
        if !precheck.can_import {
            return Err(AstrbotError::Pipeline(
                precheck
                    .error
                    .unwrap_or_else(|| "backup import precheck failed".to_string()),
            ));
        }

        self.jobs.update_progress(
            &request.task_id,
            BackupProgressSnapshot::running("restore", 2, 3, "restoring backup contents"),
        )?;
        let result = self
            .importer
            .import_backup(manifest, request.mode.clone())
            .await?;
        if !result.success {
            return Err(AstrbotError::Pipeline(result.errors.join("; ")));
        }
        Ok(result)
    }
}

#[async_trait]
impl BackupProgressReader for BackupJobService {
    async fn progress_snapshot(&self, task_id: &str) -> Result<Option<BackupJobSnapshot>> {
        self.jobs.progress_snapshot(task_id).await
    }

    async fn progress_snapshots(&self) -> Result<Vec<BackupJobSnapshot>> {
        self.jobs.progress_snapshots().await
    }
}

impl fmt::Debug for BackupJobService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupJobService")
            .field("jobs", &self.jobs)
            .finish_non_exhaustive()
    }
}

fn missing_job(task_id: &str) -> AstrbotError {
    AstrbotError::Pipeline(format!("backup job {task_id} was not found"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use serde_json::json;

    use super::{BackupExportJobRequest, BackupImportJobRequest, BackupRepositoryPort};
    use crate::{
        BackupExportPackage, BackupExportPort, BackupExportRequest, BackupImportMode,
        BackupImportPort, BackupImportPrecheck, BackupImportResult, BackupJobService,
        BackupJobStatus, BackupManifest, BackupTableDump,
    };

    struct FakeRepository {
        manifest: BackupManifest,
    }

    #[async_trait]
    impl BackupRepositoryPort for FakeRepository {
        async fn collect_export(
            &self,
            request: &BackupExportJobRequest,
        ) -> astrbot_core::Result<BackupExportRequest> {
            Ok(
                BackupExportRequest::new(&request.astrbot_version, &request.exported_at)
                    .with_table_dump(BackupTableDump::new(
                        "main_db",
                        "conversations",
                        vec![json!({"id": "c1"})],
                    )),
            )
        }

        async fn load_import_manifest(
            &self,
            _request: &BackupImportJobRequest,
        ) -> astrbot_core::Result<BackupManifest> {
            Ok(self.manifest.clone())
        }
    }

    struct RequestEchoExporter;

    #[async_trait]
    impl BackupExportPort for RequestEchoExporter {
        async fn export_backup(
            &self,
            request: BackupExportRequest,
        ) -> astrbot_core::Result<BackupExportPackage> {
            Ok(BackupExportPackage::from_request(request))
        }
    }

    struct FakeImporter {
        current_version: String,
        import_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl BackupImportPort for FakeImporter {
        async fn precheck_backup(
            &self,
            manifest: &BackupManifest,
        ) -> astrbot_core::Result<BackupImportPrecheck> {
            Ok(BackupImportPrecheck::from_manifest(
                manifest,
                &self.current_version,
            ))
        }

        async fn import_backup(
            &self,
            _manifest: BackupManifest,
            _mode: BackupImportMode,
        ) -> astrbot_core::Result<BackupImportResult> {
            self.import_calls.fetch_add(1, Ordering::Relaxed);
            Ok(BackupImportResult::success())
        }
    }

    #[tokio::test]
    async fn job_service_exports_through_repository_ports() {
        let import_calls = Arc::new(AtomicUsize::new(0));
        let service = BackupJobService::new(
            Arc::new(FakeRepository {
                manifest: BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z"),
            }),
            Arc::new(RequestEchoExporter),
            Arc::new(FakeImporter {
                current_version: "4.9.1".to_string(),
                import_calls,
            }),
        );

        let snapshot = service
            .start_export(BackupExportJobRequest::new(
                "export-1",
                "4.9.1",
                "2026-05-16T00:00:00Z",
            ))
            .await
            .expect("export should complete");

        assert_eq!(snapshot.progress.status, BackupJobStatus::Completed);
        assert_eq!(
            service
                .jobs()
                .snapshot("export-1")
                .expect("snapshot should load")
                .expect("snapshot should exist")
                .kind,
            crate::BackupJobKind::Export
        );
    }

    #[tokio::test]
    async fn job_service_blocks_incompatible_import_before_restore() {
        let import_calls = Arc::new(AtomicUsize::new(0));
        let service = BackupJobService::new(
            Arc::new(FakeRepository {
                manifest: BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z"),
            }),
            Arc::new(RequestEchoExporter),
            Arc::new(FakeImporter {
                current_version: "4.10.0".to_string(),
                import_calls: import_calls.clone(),
            }),
        );

        let error = service
            .start_import(BackupImportJobRequest::new(
                "import-1",
                "backup.zip",
                BackupImportMode::Replace,
            ))
            .await
            .expect_err("incompatible import should fail");

        assert!(error.to_string().contains("incompatible"));
        assert_eq!(import_calls.load(Ordering::Relaxed), 0);
        let snapshot = service
            .jobs()
            .snapshot("import-1")
            .expect("snapshot should load")
            .expect("snapshot should exist");
        assert_eq!(snapshot.progress.status, BackupJobStatus::Failed);
    }
}
