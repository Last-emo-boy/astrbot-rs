mod api_key;
mod attachment;
pub mod backup;
mod chat_project;
mod config_snapshot;
mod conversation;
mod file_token;
mod memory;
mod migration;
mod platform_binding;
mod provider_preference;
mod repository;
mod schema;
mod sqlite;
mod stats;
mod temp_artifact;

pub use api_key::{ApiKeyRecord, ApiKeyRepository, InMemoryApiKeyRepository};
pub use attachment::{AttachmentRecord, AttachmentRepository, InMemoryAttachmentRepository};
pub use backup::{
    BACKUP_MANIFEST_VERSION, BACKUP_UPLOAD_CHUNK_SIZE, BACKUP_UPLOAD_EXPIRE_SECONDS,
    BackupArchiveEntry, BackupChunkReceipt, BackupDirectoryStat, BackupExportJobRequest,
    BackupExportPackage, BackupExportPort, BackupExportRequest, BackupFileEntry,
    BackupImportJobRequest, BackupImportMode, BackupImportPort, BackupImportPrecheck,
    BackupImportResult, BackupJobKind, BackupJobService, BackupJobSnapshot, BackupJobStatus,
    BackupJobStore, BackupManifest, BackupProgressReader, BackupProgressSnapshot,
    BackupRepositoryPort, BackupSchemaVersion, BackupTableDump, BackupUploadCompletePlan,
    BackupUploadManager, BackupUploadSession, BackupUploadStart, BackupVersionStatus,
};
pub use chat_project::{
    ChatProjectCreateRecord, ChatProjectRecord, ChatProjectRepository, ChatProjectUpdateRecord,
    ChatUiProjectRecord, ChatUiProjectRepository, ChatUiSessionRecord,
    DEFAULT_CHAT_UI_PROJECT_EMOJI, InMemoryChatProjectRepository, InMemoryChatUiProjectRepository,
    PlatformSessionRecord, SessionProjectMembershipRecord,
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
pub use memory::{InMemoryMemoryRepository, MemoryRepository};
pub use migration::{
    DeclarativeMigration, InMemoryMigrationStateRepository, MigrationOperation, MigrationOutcome,
    MigrationRecord, MigrationRunner, MigrationStateRepository, StorageMigration,
};
pub use platform_binding::{
    InMemoryPlatformRoutingBindingRepository, PlatformRoutingBindingRecord,
    PlatformRoutingBindingRepository,
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
