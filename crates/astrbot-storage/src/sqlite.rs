use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, MessageSession, Result};
use astrbot_session::{
    ProviderCapability, SessionBatchScope, SessionBatchTarget, SessionGroup,
    SessionProviderPreference, SessionRule, SessionRuleKey, SessionRuleSet, SessionRuleValue,
    SessionServiceRule, SessionServiceRulePatch,
};
use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::schema::{StorageColumnType, StorageSchema};
use crate::{
    ApiKeyRecord, ApiKeyRepository, AttachmentRecord, AttachmentRepository,
    ChatProjectCreateRecord, ChatProjectRecord, ChatProjectRepository, ChatProjectUpdateRecord,
    ChatUiProjectRecord, ChatUiProjectRepository, ChatUiSessionRecord, ConfigSnapshotRecord,
    ConfigSnapshotRepository, ConversationHistoryRepository, ConversationMessageRecord,
    FileTokenRecord, FileTokenRepository, FileTokenScope, KbDocumentRecord, KbDocumentRepository,
    KbMediaRecord, KbProfileRecord, PlatformSessionRecord, PlatformStatsRecord,
    PlatformStatsRepository, RepositoryBackendKind, RepositoryImplementationDescriptor,
    SessionBatchUpdateReport, SessionGroupRepository, SessionProjectMembershipRecord,
    SessionRuleRepository, StorageRepositoryBoundary,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlitePragma {
    pub key: String,
    pub value: String,
}

impl SqlitePragma {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteStorageConfig {
    pub path: PathBuf,
    pub pragmas: Vec<SqlitePragma>,
}

impl SqliteStorageConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            pragmas: Self::astrbot_default_pragmas(),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(":memory:")
    }

    pub fn astrbot_default_pragmas() -> Vec<SqlitePragma> {
        vec![
            SqlitePragma::new("journal_mode", "WAL"),
            SqlitePragma::new("synchronous", "NORMAL"),
            SqlitePragma::new("cache_size", "20000"),
            SqlitePragma::new("temp_store", "MEMORY"),
            SqlitePragma::new("mmap_size", "134217728"),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteStoragePlan {
    pub config: SqliteStorageConfig,
    pub schema: StorageSchema,
}

impl SqliteStoragePlan {
    pub fn new(config: SqliteStorageConfig, schema: StorageSchema) -> Self {
        Self { config, schema }
    }

    pub fn astrbot_main(path: impl Into<PathBuf>) -> Self {
        Self::new(
            SqliteStorageConfig::new(path),
            StorageSchema::astrbot_main_v4(),
        )
    }

    pub fn create_table_statements(&self) -> Vec<String> {
        self.schema
            .tables
            .iter()
            .map(|table| {
                let mut column_defs = table
                    .columns
                    .iter()
                    .map(|column| {
                        let mut parts = vec![
                            column.name.clone(),
                            sqlite_type(&column.column_type).to_string(),
                        ];
                        if !column.nullable {
                            parts.push("NOT NULL".to_string());
                        }
                        if column.primary_key {
                            parts.push("PRIMARY KEY".to_string());
                        }
                        if column.unique {
                            parts.push("UNIQUE".to_string());
                        }
                        if let Some(default_value) = &column.default_value {
                            parts.push(format!("DEFAULT {default_value}"));
                        }
                        parts.join(" ")
                    })
                    .collect::<Vec<_>>();

                for unique_key in &table.unique_keys {
                    column_defs.push(format!("UNIQUE({})", unique_key.join(", ")));
                }

                format!(
                    "CREATE TABLE IF NOT EXISTS {} ({})",
                    table.name,
                    column_defs.join(", ")
                )
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct SqliteJsonStore {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteJsonStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| AstrbotError::Pipeline(format!("sqlite create directory: {err}")))?;
        }
        Self::from_connection(Connection::open(path).map_err(sqlite_error("open json store"))?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(
            Connection::open_in_memory().map_err(sqlite_error("open in-memory json store"))?,
        )
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS astrbot_json_store (
                   namespace TEXT NOT NULL,
                   record_key TEXT NOT NULL,
                   record_json TEXT NOT NULL,
                   PRIMARY KEY(namespace, record_key)
                 );
                 CREATE TABLE IF NOT EXISTS astrbot_json_counters (
                   namespace TEXT PRIMARY KEY,
                   next_value INTEGER NOT NULL
                 );",
            )
            .map_err(sqlite_error("create json store tables"))?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn put_json<T: Serialize>(&self, namespace: &str, key: &str, value: &T) -> Result<()> {
        let namespace = required_text(namespace, "json namespace")?;
        let key = required_text(key, "json key")?;
        let json = to_json(value)?;
        self.conn()?
            .execute(
                "INSERT INTO astrbot_json_store (namespace, record_key, record_json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(namespace, record_key) DO UPDATE SET
                   record_json = excluded.record_json",
                params![namespace, key, json],
            )
            .map_err(sqlite_error("put json record"))?;
        Ok(())
    }

    pub fn get_json<T: DeserializeOwned>(&self, namespace: &str, key: &str) -> Result<Option<T>> {
        let namespace = required_text(namespace, "json namespace")?;
        let key = required_text(key, "json key")?;
        self.conn()?
            .query_row(
                "SELECT record_json FROM astrbot_json_store WHERE namespace = ?1 AND record_key = ?2",
                params![namespace, key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error("get json record"))?
            .map(|json| from_json(&json))
            .transpose()
    }

    pub fn list_json<T: DeserializeOwned>(&self, namespace: &str) -> Result<Vec<T>> {
        let namespace = required_text(namespace, "json namespace")?;
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT record_json FROM astrbot_json_store WHERE namespace = ?1 ORDER BY record_key",
            )
            .map_err(sqlite_error("prepare json record list"))?;
        let rows = statement
            .query_map([namespace], |row| row.get::<_, String>(0))
            .map_err(sqlite_error("query json record list"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect json record list"))?;
        rows.into_iter().map(|json| from_json(&json)).collect()
    }

    pub fn delete_json(&self, namespace: &str, key: &str) -> Result<bool> {
        let namespace = required_text(namespace, "json namespace")?;
        let key = required_text(key, "json key")?;
        let affected = self
            .conn()?
            .execute(
                "DELETE FROM astrbot_json_store WHERE namespace = ?1 AND record_key = ?2",
                params![namespace, key],
            )
            .map_err(sqlite_error("delete json record"))?;
        Ok(affected > 0)
    }

    pub fn next_record_id(&self, namespace: &str) -> Result<u64> {
        let namespace = required_text(namespace, "counter namespace")?;
        let conn = self.conn()?;
        let current = conn
            .query_row(
                "SELECT next_value FROM astrbot_json_counters WHERE namespace = ?1",
                [namespace],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error("load json counter"))?
            .unwrap_or(1);
        conn.execute(
            "INSERT INTO astrbot_json_counters (namespace, next_value)
             VALUES (?1, ?2)
             ON CONFLICT(namespace) DO UPDATE SET next_value = excluded.next_value",
            params![namespace, current + 1],
        )
        .map_err(sqlite_error("store json counter"))?;
        Ok(current as u64)
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|err| AstrbotError::Pipeline(format!("sqlite json store lock: {err}")))
    }
}

pub struct SqliteStorage {
    config: SqliteStorageConfig,
    schema: StorageSchema,
    connection: Mutex<Connection>,
}

impl SqliteStorage {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::open_with_config(SqliteStorageConfig::new(path))
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::open_with_config(SqliteStorageConfig::in_memory())
    }

    pub fn open_with_config(config: SqliteStorageConfig) -> Result<Self> {
        let schema = StorageSchema::astrbot_main_v4();
        let connection = if config.path == PathBuf::from(":memory:") {
            Connection::open_in_memory()
        } else {
            if let Some(parent) = config.path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    AstrbotError::Pipeline(format!("sqlite create directory: {err}"))
                })?;
            }
            Connection::open(&config.path)
        }
        .map_err(sqlite_error("open"))?;

        let storage = Self {
            config,
            schema,
            connection: Mutex::new(connection),
        };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn config(&self) -> &SqliteStorageConfig {
        &self.config
    }

    pub fn schema(&self) -> &StorageSchema {
        &self.schema
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn()?;
        for pragma in &self.config.pragmas {
            conn.pragma_update(None, &pragma.key, &pragma.value)
                .map_err(sqlite_error("apply pragma"))?;
        }
        let plan = SqliteStoragePlan::new(self.config.clone(), self.schema.clone());
        for statement in plan.create_table_statements() {
            conn.execute(&statement, [])
                .map_err(sqlite_error("create table"))?;
        }
        for statement in sqlite_indexes() {
            conn.execute(statement, [])
                .map_err(sqlite_error("create index"))?;
        }
        ensure_column(&conn, "api_keys", "last_used_at", "TEXT")?;
        Ok(())
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|err| AstrbotError::Pipeline(format!("sqlite connection lock: {err}")))
    }

    fn next_project_id(conn: &Connection) -> Result<String> {
        let mut next = conn
            .query_row("SELECT COUNT(*) + 1 FROM chatui_projects", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sqlite_error("allocate project id"))?;
        loop {
            let candidate = format!("project-{next}");
            let exists = conn
                .query_row(
                    "SELECT 1 FROM chatui_projects WHERE project_id = ?1",
                    [&candidate],
                    |_| Ok(()),
                )
                .optional()
                .map_err(sqlite_error("check project id"))?
                .is_some();
            if !exists {
                return Ok(candidate);
            }
            next += 1;
        }
    }
}

#[async_trait]
impl StorageRepositoryBoundary for SqliteStorage {
    fn descriptor(&self) -> RepositoryImplementationDescriptor {
        RepositoryImplementationDescriptor::new("main", RepositoryBackendKind::Sqlite, &self.schema)
    }

    async fn health_check(&self) -> Result<()> {
        self.conn()?
            .query_row("SELECT 1", [], |_| Ok(()))
            .map_err(sqlite_error("health check"))
    }
}

#[async_trait]
impl ApiKeyRepository for SqliteStorage {
    async fn store_api_key(&self, record: ApiKeyRecord) -> Result<()> {
        let scopes = to_json(&record.scopes)?;
        self.conn()?
            .execute(
                "INSERT INTO api_keys (key_id, name, key_hash, key_prefix, scopes, created_by, last_used_at, expires_at, revoked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(key_id) DO UPDATE SET
                   name = excluded.name,
                   key_hash = excluded.key_hash,
                   key_prefix = excluded.key_prefix,
                   scopes = excluded.scopes,
                   created_by = excluded.created_by,
                   last_used_at = excluded.last_used_at,
                   expires_at = excluded.expires_at,
                   revoked_at = excluded.revoked_at",
                params![
                    record.key_id,
                    record.name,
                    record.key_hash,
                    record.key_prefix,
                    scopes,
                    record.created_by,
                    record.last_used_at,
                    record.expires_at,
                    record.revoked_at,
                ],
            )
            .map_err(sqlite_error("store api key"))?;
        Ok(())
    }

    async fn api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>> {
        self.conn()?
            .query_row(
                "SELECT key_id, name, key_hash, key_prefix, scopes, created_by, last_used_at, expires_at, revoked_at
                 FROM api_keys WHERE key_hash = ?1",
                [key_hash],
                api_key_from_row,
            )
            .optional()
            .map_err(sqlite_error("load api key"))
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT key_id, name, key_hash, key_prefix, scopes, created_by, last_used_at, expires_at, revoked_at
                 FROM api_keys ORDER BY key_id",
            )
            .map_err(sqlite_error("prepare api key list"))?;
        statement
            .query_map([], api_key_from_row)
            .map_err(sqlite_error("query api key list"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect api key list"))
    }

    async fn revoke_api_key(&self, key_id: &str, revoked_at: String) -> Result<bool> {
        let affected = self
            .conn()?
            .execute(
                "UPDATE api_keys SET revoked_at = ?2 WHERE key_id = ?1",
                params![key_id, revoked_at],
            )
            .map_err(sqlite_error("revoke api key"))?;
        Ok(affected > 0)
    }

    async fn delete_api_key(&self, key_id: &str) -> Result<bool> {
        let affected = self
            .conn()?
            .execute("DELETE FROM api_keys WHERE key_id = ?1", [key_id])
            .map_err(sqlite_error("delete api key"))?;
        Ok(affected > 0)
    }
}

#[async_trait]
impl FileTokenRepository for SqliteStorage {
    async fn put_file_token(&self, record: FileTokenRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO file_tokens (token, file_path, scope, filename, content_type, expires_at_unix, single_use)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(token) DO UPDATE SET
                   file_path = excluded.file_path,
                   scope = excluded.scope,
                   filename = excluded.filename,
                   content_type = excluded.content_type,
                   expires_at_unix = excluded.expires_at_unix,
                   single_use = excluded.single_use",
                params![
                    required_text(&record.token, "file token")?,
                    record.file_path.to_string_lossy().to_string(),
                    record.scope.as_str(),
                    record.filename,
                    record.content_type,
                    record.expires_at_unix.map(|value| value as i64),
                    bool_int(record.single_use),
                ],
            )
            .map_err(sqlite_error("put file token"))?;
        Ok(())
    }

    async fn file_token(&self, token: &str) -> Result<Option<FileTokenRecord>> {
        self.conn()?
            .query_row(
                "SELECT token, file_path, scope, filename, content_type, expires_at_unix, single_use
                 FROM file_tokens WHERE token = ?1",
                [token.trim()],
                file_token_from_row,
            )
            .optional()
            .map_err(sqlite_error("load file token"))
    }

    async fn consume_file_token(
        &self,
        token: &str,
        now_unix: u64,
    ) -> Result<Option<FileTokenRecord>> {
        let token = token.trim();
        let Some(record) = self.file_token(token).await? else {
            return Ok(None);
        };
        if record.is_expired_at(now_unix) {
            self.conn()?
                .execute("DELETE FROM file_tokens WHERE token = ?1", [token])
                .map_err(sqlite_error("delete expired file token"))?;
            return Ok(None);
        }
        if record.single_use {
            self.conn()?
                .execute("DELETE FROM file_tokens WHERE token = ?1", [token])
                .map_err(sqlite_error("consume file token"))?;
        }
        Ok(Some(record))
    }

    async fn remove_expired_file_tokens(&self, now_unix: u64) -> Result<usize> {
        self.conn()?
            .execute(
                "DELETE FROM file_tokens WHERE expires_at_unix IS NOT NULL AND expires_at_unix <= ?1",
                [now_unix as i64],
            )
            .map_err(sqlite_error("remove expired file tokens"))
    }
}

#[async_trait]
impl AttachmentRepository for SqliteStorage {
    async fn put_attachment(&self, record: AttachmentRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO attachments (attachment_id, source_url, stored_url, filename, content_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(attachment_id) DO UPDATE SET
                   source_url = excluded.source_url,
                   stored_url = excluded.stored_url,
                   filename = excluded.filename,
                   content_type = excluded.content_type",
                params![
                    record.attachment_id,
                    record.source_url,
                    record.stored_url,
                    record.filename,
                    record.content_type,
                ],
            )
            .map_err(sqlite_error("put attachment"))?;
        Ok(())
    }

    async fn attachment(&self, attachment_id: &str) -> Result<Option<AttachmentRecord>> {
        self.conn()?
            .query_row(
                "SELECT attachment_id, source_url, stored_url, filename, content_type
                 FROM attachments WHERE attachment_id = ?1",
                [attachment_id],
                attachment_from_row,
            )
            .optional()
            .map_err(sqlite_error("load attachment"))
    }
}

#[async_trait]
impl ConversationHistoryRepository for SqliteStorage {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()> {
        let message_id = record
            .message_id
            .clone()
            .unwrap_or_else(|| format!("msg-{}", unix_nanos()));
        let chain = to_json(&record.chain)?;
        self.conn()?
            .execute(
                "INSERT INTO conversation_messages (message_id, conversation_id, platform_id, message_chain)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    message_id,
                    record.session.conversation_id,
                    record.session.platform_id,
                    chain,
                ],
            )
            .map_err(sqlite_error("append conversation message"))?;
        Ok(())
    }

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT message_id, conversation_id, platform_id, message_chain
                 FROM conversation_messages WHERE conversation_id = ?1 ORDER BY rowid",
            )
            .map_err(sqlite_error("prepare conversation messages"))?;
        let rows = statement
            .query_map([conversation_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(sqlite_error("query conversation messages"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect conversation messages"))?;
        rows.into_iter()
            .map(|(message_id, conversation_id, platform_id, chain)| {
                Ok(ConversationMessageRecord {
                    message_id: Some(message_id),
                    session: MessageSession::new(platform_id, conversation_id),
                    chain: from_json(&chain)?,
                })
            })
            .collect()
    }
}

#[async_trait]
impl ConfigSnapshotRepository for SqliteStorage {
    async fn put_snapshot(&self, record: ConfigSnapshotRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO config_snapshots (snapshot_id, config, note)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(snapshot_id) DO UPDATE SET
                   config = excluded.config,
                   note = excluded.note",
                params![record.snapshot_id, record.config.to_string(), record.note],
            )
            .map_err(sqlite_error("put config snapshot"))?;
        Ok(())
    }

    async fn snapshot(&self, snapshot_id: &str) -> Result<Option<ConfigSnapshotRecord>> {
        self.conn()?
            .query_row(
                "SELECT snapshot_id, config, note FROM config_snapshots WHERE snapshot_id = ?1",
                [snapshot_id],
                config_snapshot_from_row,
            )
            .optional()
            .map_err(sqlite_error("load config snapshot"))?
            .map(|(snapshot_id, config, note)| {
                Ok(ConfigSnapshotRecord {
                    snapshot_id,
                    config: from_json(&config)?,
                    note,
                })
            })
            .transpose()
    }

    async fn latest_snapshot(&self) -> Result<Option<ConfigSnapshotRecord>> {
        self.conn()?
            .query_row(
                "SELECT snapshot_id, config, note FROM config_snapshots ORDER BY rowid DESC LIMIT 1",
                [],
                config_snapshot_from_row,
            )
            .optional()
            .map_err(sqlite_error("load latest config snapshot"))?
            .map(|(snapshot_id, config, note)| {
                Ok(ConfigSnapshotRecord {
                    snapshot_id,
                    config: from_json(&config)?,
                    note,
                })
            })
            .transpose()
    }
}

#[async_trait]
impl PlatformStatsRepository for SqliteStorage {
    async fn increment_platform_stats(&self, record: PlatformStatsRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO platform_stats (timestamp, platform_id, platform_type, count)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(timestamp, platform_id, platform_type)
                 DO UPDATE SET count = count + excluded.count",
                params![
                    record.timestamp,
                    record.platform_id,
                    record.platform_type,
                    record.count,
                ],
            )
            .map_err(sqlite_error("increment platform stats"))?;
        Ok(())
    }

    async fn platform_stats_since(&self, timestamp: &str) -> Result<Vec<PlatformStatsRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT timestamp, platform_id, platform_type, count
                 FROM platform_stats WHERE timestamp >= ?1
                 ORDER BY timestamp, platform_id, platform_type",
            )
            .map_err(sqlite_error("prepare platform stats"))?;
        statement
            .query_map([timestamp], platform_stats_from_row)
            .map_err(sqlite_error("query platform stats"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect platform stats"))
    }

    async fn total_message_count(&self) -> Result<i64> {
        self.conn()?
            .query_row(
                "SELECT COALESCE(SUM(count), 0) FROM platform_stats",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error("total platform stats"))
    }
}

#[async_trait]
impl ChatProjectRepository for SqliteStorage {
    async fn create_project(&self, record: ChatProjectCreateRecord) -> Result<ChatProjectRecord> {
        let conn = self.conn()?;
        let project_id = Self::next_project_id(&conn)?;
        let project = ChatProjectRecord::new(project_id, record.creator, record.title, record.now)
            .with_emoji(record.emoji)
            .with_description(record.description);
        insert_project(&conn, &project)?;
        Ok(project)
    }

    async fn project_by_id(&self, project_id: &str) -> Result<Option<ChatProjectRecord>> {
        self.conn()?
            .query_row(
                "SELECT project_id, creator, title, emoji, description, created_at, updated_at
                 FROM chatui_projects WHERE project_id = ?1",
                [project_id],
                project_from_row,
            )
            .optional()
            .map_err(sqlite_error("load chat project"))
    }

    async fn projects_by_creator(&self, creator: &str) -> Result<Vec<ChatProjectRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT project_id, creator, title, emoji, description, created_at, updated_at
                 FROM chatui_projects WHERE creator = ?1
                 ORDER BY updated_at DESC, project_id",
            )
            .map_err(sqlite_error("prepare chat projects"))?;
        statement
            .query_map([creator], project_from_row)
            .map_err(sqlite_error("query chat projects"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect chat projects"))
    }

    async fn update_project(
        &self,
        project_id: &str,
        record: ChatProjectUpdateRecord,
    ) -> Result<bool> {
        let mut project = match self.project_by_id(project_id).await? {
            Some(project) => project,
            None => return Ok(false),
        };
        if let Some(title) = record.title {
            project.title = title;
        }
        if let Some(emoji) = record.emoji {
            project.emoji = Some(emoji);
        }
        if let Some(description) = record.description {
            project.description = Some(description);
        }
        project.updated_at = record.updated_at;
        let conn = self.conn()?;
        insert_project(&conn, &project)?;
        Ok(true)
    }

    async fn delete_project(&self, project_id: &str) -> Result<bool> {
        let conn = self.conn()?;
        let affected = conn
            .execute(
                "DELETE FROM chatui_projects WHERE project_id = ?1",
                [project_id],
            )
            .map_err(sqlite_error("delete chat project"))?;
        if affected > 0 {
            conn.execute(
                "DELETE FROM session_project_relations WHERE project_id = ?1",
                [project_id],
            )
            .map_err(sqlite_error("delete chat project memberships"))?;
        }
        Ok(affected > 0)
    }

    async fn upsert_platform_session(&self, record: PlatformSessionRecord) -> Result<()> {
        let conn = self.conn()?;
        insert_platform_session(&conn, &record)
    }

    async fn platform_session(&self, session_id: &str) -> Result<Option<PlatformSessionRecord>> {
        self.conn()?
            .query_row(
                "SELECT session_id, platform_id, creator, display_name, is_group, created_at, updated_at
                 FROM platform_sessions WHERE session_id = ?1",
                [session_id],
                platform_session_from_row,
            )
            .optional()
            .map_err(sqlite_error("load platform session"))
    }

    async fn add_session_to_project(
        &self,
        session_id: &str,
        project_id: &str,
    ) -> Result<SessionProjectMembershipRecord> {
        self.conn()?
            .execute(
                "INSERT INTO session_project_relations (session_id, project_id)
                 VALUES (?1, ?2)
                 ON CONFLICT(session_id) DO UPDATE SET project_id = excluded.project_id",
                params![
                    required_text(session_id, "session_id")?,
                    required_text(project_id, "project_id")?
                ],
            )
            .map_err(sqlite_error("assign session project"))?;
        Ok(SessionProjectMembershipRecord::new(session_id, project_id))
    }

    async fn remove_session_from_project(&self, session_id: &str) -> Result<bool> {
        let affected = self
            .conn()?
            .execute(
                "DELETE FROM session_project_relations WHERE session_id = ?1",
                [session_id],
            )
            .map_err(sqlite_error("remove session project"))?;
        Ok(affected > 0)
    }

    async fn project_sessions(&self, project_id: &str) -> Result<Vec<PlatformSessionRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare(
                "SELECT s.session_id, s.platform_id, s.creator, s.display_name, s.is_group, s.created_at, s.updated_at
                 FROM platform_sessions s
                 JOIN session_project_relations r ON r.session_id = s.session_id
                 WHERE r.project_id = ?1
                 ORDER BY s.updated_at DESC, s.session_id",
            )
            .map_err(sqlite_error("prepare project sessions"))?;
        statement
            .query_map([project_id], platform_session_from_row)
            .map_err(sqlite_error("query project sessions"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect project sessions"))
    }

    async fn project_by_session(
        &self,
        session_id: &str,
        creator: &str,
    ) -> Result<Option<ChatProjectRecord>> {
        self.conn()?
            .query_row(
                "SELECT p.project_id, p.creator, p.title, p.emoji, p.description, p.created_at, p.updated_at
                 FROM chatui_projects p
                 JOIN session_project_relations r ON r.project_id = p.project_id
                 WHERE r.session_id = ?1 AND p.creator = ?2",
                params![session_id, creator],
                project_from_row,
            )
            .optional()
            .map_err(sqlite_error("load project by session"))
    }
}

#[async_trait]
impl ChatUiProjectRepository for SqliteStorage {
    async fn create_chatui_project(
        &self,
        record: ChatUiProjectRecord,
    ) -> Result<ChatUiProjectRecord> {
        let conn = self.conn()?;
        insert_chatui_project(&conn, &record)?;
        Ok(record)
    }

    async fn chatui_project(&self, project_id: &str) -> Result<Option<ChatUiProjectRecord>> {
        self.project_by_id(project_id)
            .await
            .map(|project| project.map(Into::into))
    }

    async fn chatui_projects_by_creator(&self, creator: &str) -> Result<Vec<ChatUiProjectRecord>> {
        self.projects_by_creator(creator)
            .await
            .map(|projects| projects.into_iter().map(Into::into).collect())
    }

    async fn update_chatui_project(
        &self,
        record: ChatUiProjectRecord,
    ) -> Result<Option<ChatUiProjectRecord>> {
        if self.chatui_project(&record.project_id).await?.is_none() {
            return Ok(None);
        }
        let conn = self.conn()?;
        insert_chatui_project(&conn, &record)?;
        Ok(Some(record))
    }

    async fn delete_chatui_project(&self, project_id: &str) -> Result<bool> {
        self.delete_project(project_id).await
    }

    async fn upsert_chatui_session(&self, record: ChatUiSessionRecord) -> Result<()> {
        let conn = self.conn()?;
        insert_chatui_session(&conn, &record)
    }

    async fn chatui_session(&self, session_id: &str) -> Result<Option<ChatUiSessionRecord>> {
        self.platform_session(session_id)
            .await
            .map(|session| session.map(Into::into))
    }

    async fn assign_session_to_project(&self, session_id: &str, project_id: &str) -> Result<()> {
        self.add_session_to_project(session_id, project_id)
            .await
            .map(|_| ())
    }

    async fn remove_session_from_project(&self, session_id: &str) -> Result<bool> {
        ChatProjectRepository::remove_session_from_project(self, session_id).await
    }

    async fn project_sessions(&self, project_id: &str) -> Result<Vec<ChatUiSessionRecord>> {
        ChatProjectRepository::project_sessions(self, project_id)
            .await
            .map(|sessions| sessions.into_iter().map(Into::into).collect())
    }

    async fn project_by_session(
        &self,
        session_id: &str,
        creator: &str,
    ) -> Result<Option<ChatUiProjectRecord>> {
        ChatProjectRepository::project_by_session(self, session_id, creator)
            .await
            .map(|project| project.map(Into::into))
    }
}

#[async_trait]
impl SessionRuleRepository for SqliteStorage {
    async fn upsert_rule(&self, rule: SessionRule) -> Result<()> {
        let existing = self.rule_set(&rule.umo).await?;
        let mut rule_set = existing.unwrap_or_else(|| {
            SessionRuleSet::new(rule.umo.clone()).expect("rule umo was validated")
        });
        rule_set = rule_set.with_rule(rule);
        if rule_set.has_any_rule() {
            let conn = self.conn()?;
            put_rule_set(&conn, &rule_set)?;
        }
        Ok(())
    }

    async fn rule_set(&self, umo: &str) -> Result<Option<SessionRuleSet>> {
        self.conn()?
            .query_row(
                "SELECT rule_set FROM session_rule_sets WHERE umo = ?1",
                [required_text(umo, "umo")?],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error("load session rule set"))?
            .map(|json| from_json::<SessionRuleSet>(&json))
            .transpose()
    }

    async fn list_rule_sets(&self) -> Result<Vec<SessionRuleSet>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare("SELECT rule_set FROM session_rule_sets ORDER BY umo")
            .map_err(sqlite_error("prepare session rule sets"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sqlite_error("query session rule sets"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect session rule sets"))?;
        rows.into_iter()
            .map(|json| from_json::<SessionRuleSet>(&json))
            .collect()
    }

    async fn delete_rule(&self, umo: &str, key: SessionRuleKey) -> Result<bool> {
        let Some(mut rule_set) = self.rule_set(umo).await? else {
            return Ok(false);
        };
        let removed = match key {
            SessionRuleKey::Service => rule_set.service.take().is_some(),
            SessionRuleKey::Plugin => rule_set.plugin.take().is_some(),
            SessionRuleKey::KnowledgeBase => rule_set.knowledge_base.take().is_some(),
            SessionRuleKey::Provider(capability) => {
                let before = rule_set.provider_preferences.len();
                rule_set
                    .provider_preferences
                    .retain(|preference| preference.capability != capability);
                before != rule_set.provider_preferences.len()
            }
        };
        if !removed {
            return Ok(false);
        }
        if rule_set.has_any_rule() {
            let conn = self.conn()?;
            put_rule_set(&conn, &rule_set)?;
        } else {
            self.delete_rule_set(umo).await?;
        }
        Ok(true)
    }

    async fn delete_rule_set(&self, umo: &str) -> Result<bool> {
        let affected = self
            .conn()?
            .execute("DELETE FROM session_rule_sets WHERE umo = ?1", [umo])
            .map_err(sqlite_error("delete session rule set"))?;
        Ok(affected > 0)
    }

    async fn apply_service_rule_patch(
        &self,
        umos: &[String],
        patch: SessionServiceRulePatch,
    ) -> Result<SessionBatchUpdateReport> {
        if !patch.has_changes() {
            return Ok(SessionBatchUpdateReport::default());
        }

        let mut success_count = 0;
        let mut failed_umos = Vec::new();
        for umo in umos {
            let umo = umo.trim();
            if umo.is_empty() {
                failed_umos.push(umo.to_string());
                continue;
            }
            let mut rule_set = self
                .rule_set(umo)
                .await?
                .unwrap_or_else(|| SessionRuleSet::new(umo).expect("trimmed umo"));
            let mut service = rule_set
                .service
                .take()
                .unwrap_or_else(SessionServiceRule::new);
            service.merge_patch(patch.clone());
            rule_set.service = Some(service);
            let conn = self.conn()?;
            put_rule_set(&conn, &rule_set)?;
            success_count += 1;
        }
        Ok(SessionBatchUpdateReport::new(success_count, failed_umos))
    }

    async fn set_provider_preference(
        &self,
        umo: &str,
        preference: SessionProviderPreference,
    ) -> Result<()> {
        self.upsert_rule(
            SessionRule::new(
                umo,
                SessionRuleKey::Provider(preference.capability),
                SessionRuleValue::Provider(preference),
            )
            .expect("umo was validated"),
        )
        .await
    }

    async fn provider_preference(
        &self,
        umo: &str,
        capability: ProviderCapability,
    ) -> Result<Option<String>> {
        Ok(self
            .rule_set(umo)
            .await?
            .and_then(|rule_set| rule_set.provider_for(capability).map(ToString::to_string)))
    }
}

#[async_trait]
impl SessionGroupRepository for SqliteStorage {
    async fn upsert_group(&self, group: SessionGroup) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO session_groups (group_id, group_record)
                 VALUES (?1, ?2)
                 ON CONFLICT(group_id) DO UPDATE SET group_record = excluded.group_record",
                params![required_text(&group.id, "group_id")?, to_json(&group)?],
            )
            .map_err(sqlite_error("upsert session group"))?;
        Ok(())
    }

    async fn group(&self, group_id: &str) -> Result<Option<SessionGroup>> {
        self.conn()?
            .query_row(
                "SELECT group_record FROM session_groups WHERE group_id = ?1",
                [required_text(group_id, "group_id")?],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error("load session group"))?
            .map(|json| from_json::<SessionGroup>(&json))
            .transpose()
    }

    async fn list_groups(&self) -> Result<Vec<SessionGroup>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare("SELECT group_record FROM session_groups ORDER BY group_id")
            .map_err(sqlite_error("prepare session groups"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sqlite_error("query session groups"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect session groups"))?;
        let mut groups = rows
            .into_iter()
            .map(|json| from_json::<SessionGroup>(&json))
            .collect::<Result<Vec<_>>>()?;
        groups.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(groups)
    }

    async fn delete_group(&self, group_id: &str) -> Result<bool> {
        let affected = self
            .conn()?
            .execute("DELETE FROM session_groups WHERE group_id = ?1", [group_id])
            .map_err(sqlite_error("delete session group"))?;
        Ok(affected > 0)
    }

    async fn resolve_batch_target(
        &self,
        scope: SessionBatchScope,
        all_umos: Vec<String>,
    ) -> Result<SessionBatchTarget> {
        let groups = self.list_groups().await?;
        Ok(SessionBatchTarget::resolve(scope, all_umos, &groups))
    }
}

#[async_trait]
impl KbDocumentRepository for SqliteStorage {
    async fn upsert_profile(&self, profile: KbProfileRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO kb_profiles (kb_id, profile)
                 VALUES (?1, ?2)
                 ON CONFLICT(kb_id) DO UPDATE SET profile = excluded.profile",
                params![profile.kb_id, to_json(&profile)?],
            )
            .map_err(sqlite_error("upsert kb profile"))?;
        Ok(())
    }

    async fn get_profile(&self, kb_id: &str) -> Result<Option<KbProfileRecord>> {
        self.conn()?
            .query_row(
                "SELECT profile FROM kb_profiles WHERE kb_id = ?1",
                [kb_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error("load kb profile"))?
            .map(|json| from_json(&json))
            .transpose()
    }

    async fn upsert_document(&self, document: KbDocumentRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO kb_documents (doc_id, kb_id, document)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(doc_id) DO UPDATE SET
                   kb_id = excluded.kb_id,
                   document = excluded.document",
                params![document.doc_id, document.kb_id, to_json(&document)?],
            )
            .map_err(sqlite_error("upsert kb document"))?;
        Ok(())
    }

    async fn list_documents(&self, kb_id: &str) -> Result<Vec<KbDocumentRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare("SELECT document FROM kb_documents WHERE kb_id = ?1 ORDER BY doc_id")
            .map_err(sqlite_error("prepare kb documents"))?;
        let rows = statement
            .query_map([kb_id], |row| row.get::<_, String>(0))
            .map_err(sqlite_error("query kb documents"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect kb documents"))?;
        rows.into_iter().map(|json| from_json(&json)).collect()
    }

    async fn upsert_media(&self, media: KbMediaRecord) -> Result<()> {
        self.conn()?
            .execute(
                "INSERT INTO kb_media (media_id, doc_id, kb_id, media)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(media_id) DO UPDATE SET
                   doc_id = excluded.doc_id,
                   kb_id = excluded.kb_id,
                   media = excluded.media",
                params![media.media_id, media.doc_id, media.kb_id, to_json(&media)?],
            )
            .map_err(sqlite_error("upsert kb media"))?;
        Ok(())
    }

    async fn list_media(&self, doc_id: &str) -> Result<Vec<KbMediaRecord>> {
        let conn = self.conn()?;
        let mut statement = conn
            .prepare("SELECT media FROM kb_media WHERE doc_id = ?1 ORDER BY media_id")
            .map_err(sqlite_error("prepare kb media"))?;
        let rows = statement
            .query_map([doc_id], |row| row.get::<_, String>(0))
            .map_err(sqlite_error("query kb media"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error("collect kb media"))?;
        rows.into_iter().map(|json| from_json(&json)).collect()
    }
}

fn sqlite_type(column_type: &StorageColumnType) -> &'static str {
    match column_type {
        StorageColumnType::Text => "TEXT",
        StorageColumnType::Integer => "INTEGER",
        StorageColumnType::Boolean => "INTEGER",
        StorageColumnType::Json => "JSON",
        StorageColumnType::Timestamp => "DATETIME",
        StorageColumnType::Binary => "BLOB",
    }
}

fn sqlite_indexes() -> &'static [&'static str] {
    &[
        "CREATE INDEX IF NOT EXISTS idx_conversation_messages_conversation_id ON conversation_messages(conversation_id)",
        "CREATE INDEX IF NOT EXISTS idx_kb_documents_kb_id ON kb_documents(kb_id)",
        "CREATE INDEX IF NOT EXISTS idx_kb_media_doc_id ON kb_media(doc_id)",
    ]
}

fn ensure_column(conn: &Connection, table: &str, column: &str, column_type: &str) -> Result<()> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(sqlite_error("prepare table info"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sqlite_error("query table info"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sqlite_error("collect table info"))?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}"),
        [],
    )
    .map_err(sqlite_error("add column"))?;
    Ok(())
}

fn api_key_from_row(row: &Row<'_>) -> rusqlite::Result<ApiKeyRecord> {
    let scopes: Option<String> = row.get(4)?;
    Ok(ApiKeyRecord {
        key_id: row.get(0)?,
        name: row.get(1)?,
        key_hash: row.get(2)?,
        key_prefix: row.get(3)?,
        scopes: scopes
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default(),
        created_by: row.get(5)?,
        last_used_at: row.get(6)?,
        expires_at: row.get(7)?,
        revoked_at: row.get(8)?,
    })
}

fn file_token_from_row(row: &Row<'_>) -> rusqlite::Result<FileTokenRecord> {
    let expires_at: Option<i64> = row.get(5)?;
    Ok(FileTokenRecord {
        token: row.get(0)?,
        file_path: PathBuf::from(row.get::<_, String>(1)?),
        scope: FileTokenScope::from(row.get::<_, String>(2)?.as_str()),
        filename: row.get(3)?,
        content_type: row.get(4)?,
        expires_at_unix: expires_at.map(|value| value as u64),
        single_use: row.get::<_, i64>(6)? != 0,
    })
}

fn attachment_from_row(row: &Row<'_>) -> rusqlite::Result<AttachmentRecord> {
    Ok(AttachmentRecord {
        attachment_id: row.get(0)?,
        source_url: row.get(1)?,
        stored_url: row.get(2)?,
        filename: row.get(3)?,
        content_type: row.get(4)?,
    })
}

fn config_snapshot_from_row(row: &Row<'_>) -> rusqlite::Result<(String, String, Option<String>)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn platform_stats_from_row(row: &Row<'_>) -> rusqlite::Result<PlatformStatsRecord> {
    Ok(PlatformStatsRecord {
        timestamp: row.get(0)?,
        platform_id: row.get(1)?,
        platform_type: row.get(2)?,
        count: row.get(3)?,
    })
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<ChatProjectRecord> {
    Ok(ChatProjectRecord {
        project_id: row.get(0)?,
        creator: row.get(1)?,
        title: row.get(2)?,
        emoji: row.get(3)?,
        description: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn platform_session_from_row(row: &Row<'_>) -> rusqlite::Result<PlatformSessionRecord> {
    Ok(PlatformSessionRecord {
        session_id: row.get(0)?,
        platform_id: row.get(1)?,
        creator: row.get(2)?,
        display_name: row.get(3)?,
        is_group: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn insert_project(conn: &Connection, project: &ChatProjectRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO chatui_projects (project_id, creator, title, emoji, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(project_id) DO UPDATE SET
           creator = excluded.creator,
           title = excluded.title,
           emoji = excluded.emoji,
           description = excluded.description,
           created_at = excluded.created_at,
           updated_at = excluded.updated_at",
        params![
            project.project_id,
            project.creator,
            project.title,
            project.emoji,
            project.description,
            project.created_at,
            project.updated_at,
        ],
    )
    .map_err(sqlite_error("insert chat project"))?;
    Ok(())
}

fn insert_chatui_project(conn: &Connection, record: &ChatUiProjectRecord) -> Result<()> {
    let created_at = record
        .created_at
        .clone()
        .or_else(|| record.updated_at.clone())
        .unwrap_or_else(default_timestamp);
    let updated_at = record
        .updated_at
        .clone()
        .unwrap_or_else(|| created_at.clone());
    let project = ChatProjectRecord::new(
        record.project_id.clone(),
        record.creator.clone(),
        record.title.clone(),
        created_at,
    )
    .with_emoji(record.emoji.clone())
    .with_description(record.description.clone())
    .with_updated_at(updated_at);
    insert_project(conn, &project)
}

fn insert_platform_session(conn: &Connection, session: &PlatformSessionRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO platform_sessions (session_id, platform_id, creator, display_name, is_group, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(session_id) DO UPDATE SET
           platform_id = excluded.platform_id,
           creator = excluded.creator,
           display_name = excluded.display_name,
           is_group = excluded.is_group,
           created_at = excluded.created_at,
           updated_at = excluded.updated_at",
        params![
            session.session_id,
            session.platform_id,
            session.creator,
            session.display_name,
            bool_int(session.is_group),
            session.created_at,
            session.updated_at,
        ],
    )
    .map_err(sqlite_error("insert platform session"))?;
    Ok(())
}

fn insert_chatui_session(conn: &Connection, record: &ChatUiSessionRecord) -> Result<()> {
    let created_at = record
        .created_at
        .clone()
        .or_else(|| record.updated_at.clone())
        .unwrap_or_else(default_timestamp);
    let updated_at = record
        .updated_at
        .clone()
        .unwrap_or_else(|| created_at.clone());
    insert_platform_session(
        conn,
        &PlatformSessionRecord {
            session_id: record.session_id.clone(),
            platform_id: record.platform_id.clone(),
            creator: record.creator.clone(),
            display_name: record.display_name.clone(),
            is_group: record.is_group,
            created_at,
            updated_at,
        },
    )
}

fn put_rule_set(conn: &Connection, rule_set: &SessionRuleSet) -> Result<()> {
    conn.execute(
        "INSERT INTO session_rule_sets (umo, rule_set)
         VALUES (?1, ?2)
         ON CONFLICT(umo) DO UPDATE SET rule_set = excluded.rule_set",
        params![rule_set.umo, to_json(rule_set)?],
    )
    .map_err(sqlite_error("put session rule set"))?;
    Ok(())
}

fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AstrbotError::Pipeline(format!("{field} is required")));
    }
    Ok(value)
}

fn bool_int(value: bool) -> i64 {
    i64::from(value)
}

fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|err| AstrbotError::Pipeline(format!("json encode: {err}")))
}

fn from_json<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|err| AstrbotError::Pipeline(format!("json decode: {err}")))
}

fn sqlite_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("sqlite {context}: {err}"))
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn default_timestamp() -> String {
    format!("unix:{}", unix_nanos())
}

#[cfg(test)]
mod tests {
    use super::{SqliteStorage, SqliteStoragePlan};
    use crate::{
        ApiKeyRecord, ApiKeyRepository, AttachmentRecord, AttachmentRepository,
        ChatProjectCreateRecord, ChatProjectRepository, ConfigSnapshotRecord,
        ConfigSnapshotRepository, ConversationHistoryRepository, ConversationMessageRecord,
        FileTokenRecord, FileTokenRepository, FileTokenScope, KbDocumentRecord,
        KbDocumentRepository, KbProfileRecord, PlatformSessionRecord, PlatformStatsRecord,
        PlatformStatsRepository, SessionGroupRepository, SessionRuleRepository,
    };
    use astrbot_core::{MessageChain, MessageSession};
    use astrbot_session::{
        ProviderCapability, SessionGroup, SessionProviderPreference, SessionRule, SessionRuleKey,
        SessionRuleValue, SessionServiceRule,
    };
    use serde_json::json;

    #[test]
    fn sqlite_plan_keeps_astrbot_pragmas_and_schema_tables() {
        let plan = SqliteStoragePlan::astrbot_main("data.db");

        assert!(plan.config.pragmas.iter().any(|p| p.key == "journal_mode"));
        assert!(
            plan.create_table_statements()
                .iter()
                .any(|sql| sql.contains("CREATE TABLE IF NOT EXISTS conversations"))
        );
        assert!(
            plan.create_table_statements()
                .iter()
                .any(|sql| sql.contains("CREATE TABLE IF NOT EXISTS file_tokens"))
        );
    }

    #[tokio::test]
    async fn sqlite_storage_persists_core_management_records_after_reopen() {
        let db_path =
            std::env::temp_dir().join(format!("astrbot-storage-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db_path);

        {
            let storage = SqliteStorage::open(&db_path).expect("sqlite should open");
            storage
                .store_api_key(
                    ApiKeyRecord::new(
                        "key-1",
                        "Dashboard",
                        "hash-1",
                        "ak_test_",
                        ["management.read"],
                        "admin",
                    )
                    .with_last_used_at("2026-05-18T00:00:00Z"),
                )
                .await
                .expect("api key should persist");
            storage
                .put_file_token(
                    FileTokenRecord::new("token-1", "backup.zip", FileTokenScope::Backup)
                        .reusable(),
                )
                .await
                .expect("file token should persist");
            storage
                .put_attachment(
                    AttachmentRecord::new("att-1", "https://example.test/a.png")
                        .with_stored_url("attachments/a.png"),
                )
                .await
                .expect("attachment should persist");
            storage
                .append_message(ConversationMessageRecord::new(
                    MessageSession::new("webchat", "conversation-1"),
                    MessageChain::plain("hello"),
                ))
                .await
                .expect("message should persist");
            storage
                .put_snapshot(ConfigSnapshotRecord::new("snap-1", json!({"version": 1})))
                .await
                .expect("snapshot should persist");
            storage
                .increment_platform_stats(PlatformStatsRecord::new(
                    "2026-05-18T00:00:00Z",
                    "webchat",
                    "webchat",
                    2,
                ))
                .await
                .expect("stats should persist");
            let project = storage
                .create_project(ChatProjectCreateRecord::new(
                    "alice",
                    "Ops",
                    "2026-05-18T00:00:00Z",
                ))
                .await
                .expect("project should persist");
            storage
                .upsert_platform_session(PlatformSessionRecord::new(
                    "session-1",
                    "webchat",
                    "alice",
                    "2026-05-18T00:00:00Z",
                ))
                .await
                .expect("session should persist");
            storage
                .add_session_to_project("session-1", &project.project_id)
                .await
                .expect("membership should persist");
            storage
                .upsert_rule(
                    SessionRule::new(
                        "webchat:private:alice",
                        SessionRuleKey::Service,
                        SessionRuleValue::Service(
                            SessionServiceRule::new().with_llm_enabled(false),
                        ),
                    )
                    .expect("session rule"),
                )
                .await
                .expect("rule should persist");
            storage
                .set_provider_preference(
                    "webchat:private:alice",
                    SessionProviderPreference::new(ProviderCapability::ChatCompletion, "provider")
                        .expect("provider preference"),
                )
                .await
                .expect("provider preference should persist");
            storage
                .upsert_group(
                    SessionGroup::new("team", "Team")
                        .expect("group")
                        .with_umos(["webchat:private:alice"]),
                )
                .await
                .expect("group should persist");
            storage
                .upsert_profile(KbProfileRecord {
                    kb_id: "kb-1".to_string(),
                    name: "Docs".to_string(),
                    description: None,
                    embedding_provider_id: "embed".to_string(),
                    doc_count: 1,
                    chunk_count: 2,
                })
                .await
                .expect("kb profile should persist");
            storage
                .upsert_document(KbDocumentRecord {
                    doc_id: "doc-1".to_string(),
                    kb_id: "kb-1".to_string(),
                    name: "intro.txt".to_string(),
                    file_type: "txt".to_string(),
                    file_size: 5,
                    file_path: None,
                    chunk_count: 1,
                    media_count: 0,
                })
                .await
                .expect("kb document should persist");
        }

        let reopened = SqliteStorage::open(&db_path).expect("sqlite should reopen");
        assert_eq!(
            reopened
                .api_key_by_hash("hash-1")
                .await
                .expect("key")
                .expect("key should exist")
                .last_used_at
                .as_deref(),
            Some("2026-05-18T00:00:00Z")
        );
        assert!(
            reopened
                .file_token("token-1")
                .await
                .expect("token")
                .is_some()
        );
        assert!(
            reopened
                .attachment("att-1")
                .await
                .expect("attachment")
                .is_some()
        );
        assert_eq!(
            reopened
                .messages_for_conversation("conversation-1")
                .await
                .expect("messages")[0]
                .chain
                .plain_text(),
            "hello"
        );
        assert_eq!(
            reopened
                .latest_snapshot()
                .await
                .expect("snapshot")
                .expect("snapshot")
                .config,
            json!({"version": 1})
        );
        assert_eq!(reopened.total_message_count().await.expect("count"), 2);
        assert_eq!(
            ChatProjectRepository::project_sessions(&reopened, "project-1")
                .await
                .expect("project sessions")
                .len(),
            1
        );
        assert_eq!(
            reopened
                .provider_preference("webchat:private:alice", ProviderCapability::ChatCompletion)
                .await
                .expect("provider")
                .as_deref(),
            Some("provider")
        );
        assert!(reopened.group("team").await.expect("group").is_some());
        assert_eq!(
            reopened
                .list_documents("kb-1")
                .await
                .expect("documents")
                .len(),
            1
        );

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }
}
