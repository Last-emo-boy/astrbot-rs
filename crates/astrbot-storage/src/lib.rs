mod api_key;
mod attachment;
pub mod backup;
mod config_snapshot;
mod conversation;
mod file_token;
mod migration;
mod provider_preference;
mod repository;
mod schema;
mod sqlite;
mod stats;
mod temp_artifact;

pub use api_key::{ApiKeyRecord, ApiKeyRepository, InMemoryApiKeyRepository};
pub use attachment::{AttachmentRecord, AttachmentRepository, InMemoryAttachmentRepository};
pub use backup::{
    BACKUP_MANIFEST_VERSION, BackupArchiveEntry, BackupDirectoryStat, BackupExportPackage,
    BackupExportPort, BackupExportRequest, BackupFileEntry, BackupImportMode, BackupImportPort,
    BackupImportPrecheck, BackupImportResult, BackupManifest, BackupSchemaVersion, BackupTableDump,
    BackupVersionStatus,
};
pub use config_snapshot::{
    ConfigSnapshotRecord, ConfigSnapshotRepository, InMemoryConfigSnapshotRepository,
};
pub use conversation::{
    ConversationHistoryRepository, ConversationMessageRecord, InMemoryConversationHistoryRepository,
};
pub use file_token::{
    FileTokenRecord, FileTokenRepository, FileTokenScope, InMemoryFileTokenRepository,
};
pub use migration::{
    DeclarativeMigration, InMemoryMigrationStateRepository, MigrationOperation, MigrationOutcome,
    MigrationRecord, MigrationRunner, MigrationStateRepository, StorageMigration,
};
pub use provider_preference::{
    InMemoryProviderPreferenceRepository, ProviderPreferenceRecord, ProviderPreferenceRepository,
};
pub use repository::{
    RepositoryBackendKind, RepositoryImplementationDescriptor, StorageRepositoryBoundary,
};
pub use schema::{StorageColumn, StorageColumnType, StorageSchema, StorageTable};
pub use sqlite::{SqlitePragma, SqliteStorageConfig, SqliteStoragePlan};
pub use stats::{InMemoryPlatformStatsRepository, PlatformStatsRecord, PlatformStatsRepository};
pub use temp_artifact::{
    TempArtifactCleaner, TempArtifactCleanupPlan, TempArtifactCleanupPolicy,
    TempArtifactDescriptor, TempArtifactFileInfo, TempArtifactRoot, safe_artifact_segment,
};
