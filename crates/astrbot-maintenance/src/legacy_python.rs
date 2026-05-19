use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_conversation::{
    ConversationDirectory, ConversationRecord, SqliteConversationDirectory,
};
use astrbot_core::{AstrbotError, MessageChain, MessageComponent, MessageSession, Result};
use astrbot_cron::{
    CronJob, CronJobKind, CronJobRepository, CronJobSchedule, CronJobStatus,
    SqliteCronJobRepository,
};
use astrbot_persona::{
    PersonaDialogRole, PersonaDialogTurn, PersonaFolder, PersonaProfile, PersonaRepository,
    SqlitePersonaRepository,
};
use astrbot_runtime::RuntimeConfig;
use astrbot_storage::{
    ApiKeyRecord, ApiKeyRepository, ChatUiProjectRecord, ChatUiProjectRepository,
    ChatUiSessionRecord, ConversationHistoryRepository, ConversationMessageRecord, SqliteJsonStore,
    SqliteStorage,
};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value, json};
use zip::ZipArchive;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyPythonMigrationOptions {
    pub legacy_data_dir: Option<PathBuf>,
    pub legacy_sqlite_path: Option<PathBuf>,
    pub legacy_backup_zip: Option<PathBuf>,
    pub target_sqlite_path: PathBuf,
    pub target_config_path: Option<PathBuf>,
    pub backup_dir: Option<PathBuf>,
    pub platform_id_map: BTreeMap<String, BTreeMap<String, String>>,
}

impl LegacyPythonMigrationOptions {
    pub fn new(target_sqlite_path: impl Into<PathBuf>) -> Self {
        Self {
            legacy_data_dir: None,
            legacy_sqlite_path: None,
            legacy_backup_zip: None,
            target_sqlite_path: target_sqlite_path.into(),
            target_config_path: None,
            backup_dir: None,
            platform_id_map: BTreeMap::new(),
        }
    }

    pub fn with_legacy_data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.legacy_data_dir = Some(path.into());
        self
    }

    pub fn with_legacy_sqlite_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.legacy_sqlite_path = Some(path.into());
        self
    }

    pub fn with_legacy_backup_zip(mut self, path: impl Into<PathBuf>) -> Self {
        self.legacy_backup_zip = Some(path.into());
        self
    }

    pub fn with_target_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.target_config_path = Some(path.into());
        self
    }

    pub fn with_backup_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.backup_dir = Some(path.into());
        self
    }

    pub fn with_platform_id_map(
        mut self,
        platform_id_map: BTreeMap<String, BTreeMap<String, String>>,
    ) -> Self {
        self.platform_id_map = platform_id_map;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyPythonMigrationReport {
    pub source_sqlite_path: Option<String>,
    pub source_config_path: Option<String>,
    pub source_backup_zip: Option<String>,
    pub target_sqlite_path: String,
    pub target_config_path: Option<String>,
    pub backup: LegacyPythonMigrationBackup,
    pub restore: LegacyPythonMigrationRestore,
    pub tables: Vec<LegacyPythonMigrationTableReport>,
    pub fields: Vec<LegacyPythonMigrationFieldReport>,
    pub report_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyPythonMigrationBackup {
    pub backup_dir: String,
    pub files: Vec<LegacyPythonMigrationBackupFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyPythonMigrationBackupFile {
    pub role: String,
    pub original_path: String,
    pub backup_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyPythonMigrationRestore {
    pub automatic_restore_attempted: bool,
    pub automatic_restore_succeeded: bool,
    pub instructions: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyPythonMigrationTableReport {
    pub table: String,
    pub source: String,
    pub imported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyPythonMigrationFieldReport {
    pub table: String,
    pub field: String,
    pub target: String,
    pub imported: usize,
    pub skipped: usize,
    pub note: Option<String>,
}

struct LegacySource {
    sqlite_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
}

pub async fn run_legacy_python_migration(
    options: LegacyPythonMigrationOptions,
) -> Result<LegacyPythonMigrationReport> {
    let backup_root = migration_backup_root(&options);
    let source = resolve_legacy_source(&options, &backup_root)?;
    let backup = create_preflight_backup(&options, &source, &backup_root)?;
    let restore = LegacyPythonMigrationRestore {
        automatic_restore_attempted: false,
        automatic_restore_succeeded: false,
        instructions: restore_instruction(&backup),
    };
    let mut report = LegacyPythonMigrationReport {
        source_sqlite_path: source.sqlite_path.as_ref().map(display_path),
        source_config_path: source.config_path.as_ref().map(display_path),
        source_backup_zip: options.legacy_backup_zip.as_ref().map(display_path),
        target_sqlite_path: display_path(&options.target_sqlite_path),
        target_config_path: options.target_config_path.as_ref().map(display_path),
        backup,
        restore,
        tables: Vec::new(),
        fields: Vec::new(),
        report_path: None,
    };

    let result = run_legacy_python_migration_inner(&options, &source, &mut report).await;
    if let Err(error) = result {
        let restore_result = restore_preflight_backup(&report.backup);
        report.restore.automatic_restore_attempted = true;
        report.restore.automatic_restore_succeeded = restore_result.is_ok();
        if let Err(restore_error) = restore_result {
            report.restore.instructions = format!(
                "{} Automatic restore failed: {restore_error}",
                report.restore.instructions
            );
        }
        let _ = write_report(&mut report);
        return Err(AstrbotError::Pipeline(format!(
            "legacy Python migration failed: {error}. {}",
            report.restore.instructions
        )));
    }

    report.restore.instructions = format!(
        "No restore was needed. To undo manually, stop the runtime and copy target_* files from {} back to their original paths.",
        report.backup.backup_dir
    );
    write_report(&mut report)?;
    Ok(report)
}

async fn run_legacy_python_migration_inner(
    options: &LegacyPythonMigrationOptions,
    source: &LegacySource,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let store = SqliteJsonStore::open(&options.target_sqlite_path)?;
    let storage = SqliteStorage::open(&options.target_sqlite_path)?;
    if let Some(config_path) = source.config_path.as_ref() {
        migrate_personas_from_config(config_path, &store, report).await?;
    }

    if let Some(config_path) = options.target_config_path.as_ref() {
        migrate_runtime_config(config_path, source.config_path.as_ref(), report)?;
    } else {
        report.tables.push(LegacyPythonMigrationTableReport {
            table: "cmd_config.json".to_string(),
            source: "legacy config".to_string(),
            skipped: 1,
            message: Some("target config path is not configured".to_string()),
            ..LegacyPythonMigrationTableReport::default()
        });
    }

    let Some(sqlite_path) = source.sqlite_path.as_ref() else {
        report.tables.push(LegacyPythonMigrationTableReport {
            table: "legacy_sqlite".to_string(),
            source: "legacy SQLite".to_string(),
            skipped: 1,
            message: Some("no Python data_v4.db or data_v3.db source was found".to_string()),
            ..LegacyPythonMigrationTableReport::default()
        });
        return Ok(());
    };

    let conn = Connection::open(sqlite_path).map_err(sqlite_error("open legacy sqlite"))?;
    let api_key_rows = query_table(
        &conn,
        "api_keys",
        &[
            "key_id",
            "name",
            "key_hash",
            "key_prefix",
            "scopes",
            "created_by",
            "last_used_at",
            "expires_at",
            "revoked_at",
        ],
    )?;
    let persona_folder_rows = query_table(
        &conn,
        "persona_folders",
        &[
            "folder_id",
            "name",
            "parent_id",
            "description",
            "sort_order",
        ],
    )?;
    let persona_rows = query_table(
        &conn,
        "personas",
        &[
            "persona_id",
            "system_prompt",
            "begin_dialogs",
            "tools",
            "skills",
            "custom_error_message",
            "folder_id",
            "sort_order",
        ],
    )?;
    let conversation_rows = query_table(
        &conn,
        "conversations",
        &[
            "conversation_id",
            "platform_id",
            "user_id",
            "content",
            "title",
            "persona_id",
            "created_at",
            "updated_at",
            "token_usage",
        ],
    )?;
    let v3_conversation_rows = query_table(
        &conn,
        "webchat_conversation",
        &[
            "user_id",
            "cid",
            "history",
            "created_at",
            "updated_at",
            "title",
            "persona_id",
        ],
    )?;
    let platform_session_rows = query_table(
        &conn,
        "platform_sessions",
        &[
            "session_id",
            "platform_id",
            "creator",
            "display_name",
            "is_group",
            "created_at",
            "updated_at",
        ],
    )?;
    let chatui_project_rows = query_table(
        &conn,
        "chatui_projects",
        &[
            "project_id",
            "creator",
            "title",
            "emoji",
            "description",
            "created_at",
            "updated_at",
        ],
    )?;
    let project_relation_rows = query_table(
        &conn,
        "session_project_relations",
        &["session_id", "project_id"],
    )?;
    let cron_job_rows = query_table(
        &conn,
        "cron_jobs",
        &[
            "job_id",
            "name",
            "job_type",
            "cron_expression",
            "timezone",
            "payload",
            "description",
            "enabled",
            "persistent",
            "run_once",
            "status",
            "last_error",
        ],
    )?;
    drop(conn);

    migrate_api_keys(api_key_rows, &storage, report).await?;
    migrate_personas_from_sqlite(persona_folder_rows, persona_rows, &store, report).await?;
    migrate_conversations(
        conversation_rows,
        v3_conversation_rows,
        &storage,
        &store,
        options,
        report,
    )
    .await?;
    migrate_platform_sessions(platform_session_rows, &storage, report).await?;
    migrate_chatui_projects(chatui_project_rows, project_relation_rows, &storage, report).await?;
    migrate_cron_jobs(cron_job_rows, &store, report).await?;
    Ok(())
}

fn migrate_runtime_config(
    target_config_path: &Path,
    source_config_path: Option<&PathBuf>,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let source_config_path = source_config_path
        .map(PathBuf::as_path)
        .unwrap_or(target_config_path);
    let legacy = read_json_file(source_config_path).unwrap_or(Value::Null);
    let mut config = serde_json::to_value(RuntimeConfig::from_json_file(target_config_path)?)
        .map_err(|err| AstrbotError::Pipeline(format!("serialize runtime config: {err}")))?;
    let mut imported = 0;
    let mut skipped = 0;

    if let Some(dashboard) = legacy.get("dashboard").and_then(Value::as_object) {
        for (legacy_key, target_path) in [
            ("username", "dashboard_auth.username"),
            ("password", "dashboard_auth.password"),
            ("jwt_secret", "dashboard_auth.jwt_secret"),
            ("host", "webchat_server.host"),
            ("port", "webchat_server.port"),
            ("enable", "webchat_server.enabled"),
        ] {
            match dashboard.get(legacy_key) {
                Some(value) if !value.is_null() => {
                    set_json_path(&mut config, target_path, value.clone());
                    imported += 1;
                    report.fields.push(LegacyPythonMigrationFieldReport {
                        table: "cmd_config.json".to_string(),
                        field: format!("dashboard.{legacy_key}"),
                        target: target_path.to_string(),
                        imported: 1,
                        skipped: 0,
                        note: None,
                    });
                }
                _ => skipped += 1,
            }
        }
    }

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|err| AstrbotError::Pipeline(format!("serialize migrated config: {err}")))?;
    if let Some(parent) = target_config_path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create config directory"))?;
    }
    fs::write(target_config_path, serialized).map_err(io_error("write migrated config"))?;
    report.tables.push(LegacyPythonMigrationTableReport {
        table: "cmd_config.json".to_string(),
        source: display_path(source_config_path),
        imported,
        skipped,
        failed: 0,
        message: Some(
            "runtime config defaults merged and Python dashboard fields mapped".to_string(),
        ),
    });
    Ok(())
}

async fn migrate_api_keys(
    rows: Vec<BTreeMap<String, Value>>,
    storage: &SqliteStorage,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let mut existing = storage
        .list_api_keys()
        .await?
        .into_iter()
        .map(|record| record.key_id)
        .collect::<BTreeSet<_>>();
    let mut table = table_report("api_keys", "Python v4 api_keys");
    for row in rows {
        let Some(key_id) = row_text(&row, "key_id") else {
            table.failed += 1;
            continue;
        };
        if existing.contains(&key_id) {
            table.skipped += 1;
            continue;
        }
        let Some(key_hash) = row_text(&row, "key_hash") else {
            table.failed += 1;
            continue;
        };
        let mut record = ApiKeyRecord::new(
            key_id.clone(),
            row_text(&row, "name").unwrap_or_else(|| key_id.clone()),
            key_hash,
            row_text(&row, "key_prefix").unwrap_or_else(|| "ak_legacy".to_string()),
            row_string_array(&row, "scopes").unwrap_or_default(),
            row_text(&row, "created_by").unwrap_or_else(|| "legacy-python".to_string()),
        );
        if let Some(value) = row_text(&row, "last_used_at") {
            record = record.with_last_used_at(value);
        }
        if let Some(value) = row_text(&row, "expires_at") {
            record = record.with_expires_at(value);
        }
        if let Some(value) = row_text(&row, "revoked_at") {
            record = record.revoked(value);
        }
        storage.store_api_key(record).await?;
        existing.insert(key_id);
        table.imported += 1;
    }
    push_field(report, "api_keys", "key_id", "ApiKeyRecord.key_id", &table);
    push_field(report, "api_keys", "scopes", "ApiKeyRecord.scopes", &table);
    report.tables.push(table);
    Ok(())
}

async fn migrate_personas_from_sqlite(
    folder_rows: Vec<BTreeMap<String, Value>>,
    rows: Vec<BTreeMap<String, Value>>,
    store: &SqliteJsonStore,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let repository = SqlitePersonaRepository::new(store.clone());
    let mut folder_table = table_report("persona_folders", "Python v4 persona_folders");
    for row in folder_rows {
        let Some(folder_id) = row_text(&row, "folder_id") else {
            folder_table.failed += 1;
            continue;
        };
        if repository.folder(&folder_id).await?.is_some() {
            folder_table.skipped += 1;
            continue;
        }
        let mut folder = PersonaFolder::new(
            folder_id,
            row_text(&row, "name").unwrap_or_else(|| "Folder".to_string()),
        );
        if let Some(parent_id) = row_text(&row, "parent_id") {
            folder = folder.with_parent_id(parent_id);
        }
        if let Some(description) = row_text(&row, "description") {
            folder = folder.with_description(description);
        }
        if let Some(sort_order) = row_i64(&row, "sort_order") {
            folder = folder.with_sort_order(sort_order as i32);
        }
        repository.upsert_folder(folder).await?;
        folder_table.imported += 1;
    }
    push_field(
        report,
        "persona_folders",
        "folder_id",
        "PersonaFolder.id",
        &folder_table,
    );
    report.tables.push(folder_table);

    migrate_persona_rows(rows, &repository, "Python v4 personas", report).await
}

async fn migrate_personas_from_config(
    config_path: &Path,
    store: &SqliteJsonStore,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let legacy = read_json_file(config_path)?;
    let rows = legacy
        .get("persona")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(config_persona_to_row)
        .collect::<Vec<_>>();
    let repository = SqlitePersonaRepository::new(store.clone());
    migrate_persona_rows(rows, &repository, "Python cmd_config.json persona", report).await
}

async fn migrate_persona_rows(
    rows: Vec<BTreeMap<String, Value>>,
    repository: &SqlitePersonaRepository,
    source: &str,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let mut table = table_report("personas", source);
    for row in rows {
        let Some(persona_id) = row_text(&row, "persona_id").or_else(|| row_text(&row, "name"))
        else {
            table.failed += 1;
            continue;
        };
        if repository.persona(&persona_id).await?.is_some() {
            table.skipped += 1;
            continue;
        }
        let system_prompt = legacy_persona_prompt(&row);
        let mut profile = PersonaProfile::new(persona_id, system_prompt);
        profile.begin_dialogs = persona_dialogs(row.get("begin_dialogs"));
        profile.tools = row_string_array(&row, "tools");
        profile.skills = row_string_array(&row, "skills");
        if let Some(message) = row_text(&row, "custom_error_message") {
            profile = profile.with_custom_error_message(message);
        }
        if let Some(folder_id) = row_text(&row, "folder_id") {
            profile = profile.with_folder_id(folder_id);
        }
        if let Some(sort_order) = row_i64(&row, "sort_order") {
            profile = profile.with_sort_order(sort_order as i32);
        }
        repository.upsert_persona(profile).await?;
        table.imported += 1;
    }
    push_field(
        report,
        "personas",
        "persona_id",
        "PersonaProfile.id",
        &table,
    );
    push_field(
        report,
        "personas",
        "begin_dialogs",
        "PersonaProfile.begin_dialogs",
        &table,
    );
    report.tables.push(table);
    Ok(())
}

async fn migrate_conversations(
    rows: Vec<BTreeMap<String, Value>>,
    v3_rows: Vec<BTreeMap<String, Value>>,
    storage: &SqliteStorage,
    store: &SqliteJsonStore,
    options: &LegacyPythonMigrationOptions,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let directory = SqliteConversationDirectory::new(store.clone());
    let mut table = table_report("conversations", "Python v4 conversations");
    for row in rows {
        let Some(conversation_id) = row_text(&row, "conversation_id") else {
            table.failed += 1;
            continue;
        };
        let platform_id = row_text(&row, "platform_id")
            .map(|platform| map_platform_id(&options.platform_id_map, &platform))
            .unwrap_or_else(|| "webchat".to_string());
        import_conversation_row(
            storage,
            &directory,
            &platform_id,
            &conversation_id,
            &row,
            "content",
            &mut table,
        )
        .await?;
    }

    for row in v3_rows {
        let Some(conversation_id) = row_text(&row, "cid") else {
            table.failed += 1;
            continue;
        };
        let user_id = row_text(&row, "user_id").unwrap_or_else(|| conversation_id.clone());
        let platform_id = legacy_platform_from_user_id(&user_id, &options.platform_id_map);
        import_conversation_row(
            storage,
            &directory,
            &platform_id,
            &conversation_id,
            &row,
            "history",
            &mut table,
        )
        .await?;
    }

    push_field(
        report,
        "conversations",
        "content/history",
        "conversation_directory + conversation_messages",
        &table,
    );
    report.tables.push(table);
    Ok(())
}

async fn import_conversation_row(
    storage: &SqliteStorage,
    directory: &SqliteConversationDirectory,
    platform_id: &str,
    conversation_id: &str,
    row: &BTreeMap<String, Value>,
    history_field: &str,
    table: &mut LegacyPythonMigrationTableReport,
) -> Result<()> {
    if directory
        .conversation(platform_id, conversation_id)
        .await?
        .is_some()
    {
        table.skipped += 1;
    } else {
        let mut record = ConversationRecord::new(platform_id, conversation_id);
        if let Some(user_id) = row_text(row, "user_id") {
            record = record.with_user_id(user_id);
        }
        if let Some(history) = row.get(history_field).and_then(history_to_string) {
            record = record.with_history(history);
        }
        if let Some(title) = row_text(row, "title") {
            record = record.with_title(title);
        }
        if let Some(persona_id) = row_text(row, "persona_id") {
            record = record.with_persona_id(persona_id);
        }
        if let Some(created_at) = row_i64(row, "created_at") {
            record = record.with_created_at(created_at);
        }
        if let Some(updated_at) = row_i64(row, "updated_at") {
            record = record.with_updated_at(updated_at);
        }
        if let Some(token_usage) = row_u64(row, "token_usage") {
            record = record.with_token_usage(token_usage);
        }
        directory.upsert_conversation(record).await?;
        table.imported += 1;
    }

    let existing_message_ids = storage
        .messages_for_conversation(conversation_id)
        .await?
        .into_iter()
        .filter_map(|record| record.message_id)
        .collect::<BTreeSet<_>>();
    let messages = row
        .get(history_field)
        .and_then(history_items)
        .unwrap_or_default();
    for (index, message) in messages.iter().enumerate() {
        let message_id = format!("legacy-python-{conversation_id}-{index}");
        if existing_message_ids.contains(&message_id) {
            table.skipped += 1;
            continue;
        }
        storage
            .append_message(
                ConversationMessageRecord::new(
                    MessageSession::new(platform_id, conversation_id),
                    message_chain_from_legacy(message),
                )
                .with_message_id(message_id),
            )
            .await?;
        table.imported += 1;
    }
    Ok(())
}

async fn migrate_platform_sessions(
    rows: Vec<BTreeMap<String, Value>>,
    storage: &SqliteStorage,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let mut table = table_report("platform_sessions", "Python v4 platform_sessions");
    for row in rows {
        let Some(session_id) = row_text(&row, "session_id") else {
            table.failed += 1;
            continue;
        };
        if storage.chatui_session(&session_id).await?.is_some() {
            table.skipped += 1;
            continue;
        }
        let mut record = ChatUiSessionRecord::new(
            session_id,
            row_text(&row, "platform_id").unwrap_or_else(|| "webchat".to_string()),
            row_text(&row, "creator").unwrap_or_else(|| "guest".to_string()),
        );
        if let Some(display_name) = row_text(&row, "display_name") {
            record = record.with_display_name(display_name);
        }
        if row_bool(&row, "is_group").unwrap_or(false) {
            record = record.group();
        }
        if let Some(created_at) = row_text(&row, "created_at") {
            record = record.with_created_at(created_at);
        }
        if let Some(updated_at) = row_text(&row, "updated_at") {
            record = record.with_updated_at(updated_at);
        }
        storage.upsert_chatui_session(record).await?;
        table.imported += 1;
    }
    push_field(
        report,
        "platform_sessions",
        "session_id",
        "ChatUiSessionRecord.session_id",
        &table,
    );
    report.tables.push(table);
    Ok(())
}

async fn migrate_chatui_projects(
    rows: Vec<BTreeMap<String, Value>>,
    relation_rows: Vec<BTreeMap<String, Value>>,
    storage: &SqliteStorage,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let mut table = table_report("chatui_projects", "Python v4 chatui_projects");
    for row in rows {
        let Some(project_id) = row_text(&row, "project_id") else {
            table.failed += 1;
            continue;
        };
        if storage.chatui_project(&project_id).await?.is_some() {
            table.skipped += 1;
            continue;
        }
        let mut record = ChatUiProjectRecord::new(
            project_id,
            row_text(&row, "creator").unwrap_or_else(|| "guest".to_string()),
            row_text(&row, "title").unwrap_or_else(|| "Untitled".to_string()),
        );
        if let Some(emoji) = row_text(&row, "emoji") {
            record = record.with_emoji(emoji);
        }
        if let Some(description) = row_text(&row, "description") {
            record = record.with_description(description);
        }
        if let Some(created_at) = row_text(&row, "created_at") {
            record = record.with_created_at(created_at);
        }
        if let Some(updated_at) = row_text(&row, "updated_at") {
            record = record.with_updated_at(updated_at);
        }
        storage.create_chatui_project(record).await?;
        table.imported += 1;
    }

    for row in relation_rows {
        let (Some(session_id), Some(project_id)) =
            (row_text(&row, "session_id"), row_text(&row, "project_id"))
        else {
            table.failed += 1;
            continue;
        };
        storage
            .assign_session_to_project(&session_id, &project_id)
            .await?;
        table.imported += 1;
    }
    push_field(
        report,
        "chatui_projects",
        "project_id",
        "ChatUiProjectRecord.project_id",
        &table,
    );
    report.tables.push(table);
    Ok(())
}

async fn migrate_cron_jobs(
    rows: Vec<BTreeMap<String, Value>>,
    store: &SqliteJsonStore,
    report: &mut LegacyPythonMigrationReport,
) -> Result<()> {
    let repository = SqliteCronJobRepository::new(store.clone());
    let mut table = table_report("cron_jobs", "Python v4 cron_jobs");
    for row in rows {
        let Some(job_id) = row_text(&row, "job_id") else {
            table.failed += 1;
            continue;
        };
        if repository.job(&job_id).await?.is_some() {
            table.skipped += 1;
            continue;
        }
        let payload = row_json(&row, "payload").unwrap_or_else(|| json!({}));
        let run_once = row_bool(&row, "run_once").unwrap_or(false);
        let mut schedule = if run_once {
            let Some(run_at) = payload.get("run_at").and_then(Value::as_str) else {
                table.failed += 1;
                continue;
            };
            CronJobSchedule::run_once_at(run_at)
        } else {
            let Some(expression) = row_text(&row, "cron_expression") else {
                table.failed += 1;
                continue;
            };
            CronJobSchedule::cron(expression)
        };
        if let Some(timezone) = row_text(&row, "timezone") {
            schedule = schedule.with_timezone(timezone);
        }
        let kind = match row_text(&row, "job_type").as_deref() {
            Some("basic") => CronJobKind::Basic,
            _ => CronJobKind::ActiveAgent,
        };
        let mut job = CronJob::new(
            job_id,
            row_text(&row, "name").unwrap_or_else(|| "legacy job".to_string()),
            kind,
            schedule,
        )
        .with_payload(payload)
        .persistent(row_bool(&row, "persistent").unwrap_or(true));
        if let Some(description) = row_text(&row, "description") {
            job = job.with_description(description);
        }
        if !row_bool(&row, "enabled").unwrap_or(true) {
            job = job.disabled();
        }
        job.status = match row_text(&row, "status").as_deref() {
            Some("running") => CronJobStatus::Running,
            Some("completed") => CronJobStatus::Completed,
            Some("failed") => CronJobStatus::Failed,
            Some("disabled") => CronJobStatus::Disabled,
            _ => CronJobStatus::Scheduled,
        };
        job.last_error = row_text(&row, "last_error");
        repository.upsert_job(job).await?;
        table.imported += 1;
    }
    push_field(report, "cron_jobs", "payload", "CronJob.payload", &table);
    push_field(
        report,
        "cron_jobs",
        "cron_expression/run_at",
        "CronJob.schedule",
        &table,
    );
    report.tables.push(table);
    Ok(())
}

fn resolve_legacy_source(
    options: &LegacyPythonMigrationOptions,
    backup_root: &Path,
) -> Result<LegacySource> {
    let mut data_dir = options.legacy_data_dir.clone().or_else(|| {
        options
            .target_config_path
            .as_ref()
            .and_then(|path| path.parent().map(Path::to_path_buf))
    });
    let mut sqlite_path = options.legacy_sqlite_path.clone();
    let mut config_path = options.target_config_path.clone();

    if let Some(zip_path) = options.legacy_backup_zip.as_ref() {
        let extracted = extract_legacy_backup(zip_path, backup_root)?;
        if extracted.config_path.is_some() {
            config_path = extracted.config_path.clone();
        }
        if extracted.sqlite_path.is_some() {
            sqlite_path = extracted.sqlite_path.clone();
        }
        data_dir = Some(extracted.root.clone());
    }

    if sqlite_path.is_none() {
        if let Some(dir) = data_dir.as_ref() {
            for candidate in ["data_v4.db", "data_v3.db"] {
                let path = dir.join(candidate);
                if path.is_file() {
                    sqlite_path = Some(path);
                    break;
                }
            }
        }
    }
    if config_path.is_none() {
        if let Some(dir) = data_dir.as_ref() {
            let path = dir.join("cmd_config.json");
            if path.is_file() {
                config_path = Some(path);
            }
        }
    }

    Ok(LegacySource {
        sqlite_path,
        config_path,
    })
}

struct ExtractedLegacyBackup {
    root: PathBuf,
    sqlite_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
}

fn extract_legacy_backup(zip_path: &Path, backup_root: &Path) -> Result<ExtractedLegacyBackup> {
    let file = File::open(zip_path).map_err(io_error("open legacy backup zip"))?;
    let mut archive = ZipArchive::new(file).map_err(zip_error("read legacy backup zip"))?;
    let root = backup_root.join(format!("legacy-python-extracted-{}", unix_seconds()));
    fs::create_dir_all(&root).map_err(io_error("create legacy backup extract dir"))?;
    let mut sqlite_path = None;
    let mut config_path = None;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(zip_error("read legacy backup entry"))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().replace('\\', "/");
        let Some(filename) = name.rsplit('/').next() else {
            continue;
        };
        let role = match filename {
            "cmd_config.json" => "cmd_config.json",
            "data_v4.db" => "data_v4.db",
            "data_v3.db" => "data_v3.db",
            "shared_preferences.json" => "shared_preferences.json",
            _ => continue,
        };
        let target = root.join(role);
        let mut output = File::create(&target).map_err(io_error("create extracted entry"))?;
        io::copy(&mut file, &mut output).map_err(io_error("extract legacy backup entry"))?;
        match role {
            "cmd_config.json" => config_path = Some(target),
            "data_v4.db" => sqlite_path = Some(target),
            "data_v3.db" if sqlite_path.is_none() => sqlite_path = Some(target),
            _ => {}
        }
    }
    Ok(ExtractedLegacyBackup {
        root,
        sqlite_path,
        config_path,
    })
}

fn create_preflight_backup(
    options: &LegacyPythonMigrationOptions,
    source: &LegacySource,
    backup_root: &Path,
) -> Result<LegacyPythonMigrationBackup> {
    let backup_dir = backup_root.join(format!("legacy-python-preflight-{}", unix_seconds()));
    fs::create_dir_all(&backup_dir).map_err(io_error("create migration backup dir"))?;
    let mut files = Vec::new();
    if let Some(path) = options.target_config_path.as_ref() {
        copy_backup_file("target_config", path, &backup_dir, &mut files)?;
    }
    copy_backup_file(
        "target_sqlite",
        &options.target_sqlite_path,
        &backup_dir,
        &mut files,
    )?;
    for sidecar in sqlite_sidecars(&options.target_sqlite_path) {
        copy_backup_file("target_sqlite_sidecar", &sidecar, &backup_dir, &mut files)?;
    }
    if let Some(path) = source.config_path.as_ref() {
        copy_backup_file("source_config", path, &backup_dir, &mut files)?;
    }
    if let Some(path) = source.sqlite_path.as_ref() {
        copy_backup_file("source_sqlite", path, &backup_dir, &mut files)?;
    }
    if let Some(path) = options.legacy_backup_zip.as_ref() {
        copy_backup_file("source_backup_zip", path, &backup_dir, &mut files)?;
    }
    Ok(LegacyPythonMigrationBackup {
        backup_dir: display_path(&backup_dir),
        files,
    })
}

fn copy_backup_file(
    role: &str,
    original: &Path,
    backup_dir: &Path,
    files: &mut Vec<LegacyPythonMigrationBackupFile>,
) -> Result<()> {
    if !original.is_file() {
        return Ok(());
    }
    let filename = original
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let backup_path = backup_dir.join(format!("{role}-{filename}"));
    fs::copy(original, &backup_path).map_err(io_error("copy migration backup file"))?;
    files.push(LegacyPythonMigrationBackupFile {
        role: role.to_string(),
        original_path: display_path(original),
        backup_path: display_path(&backup_path),
    });
    Ok(())
}

fn restore_preflight_backup(backup: &LegacyPythonMigrationBackup) -> io::Result<()> {
    for file in &backup.files {
        if !file.role.starts_with("target_") {
            continue;
        }
        let original = PathBuf::from(&file.original_path);
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&file.backup_path, original)?;
    }
    Ok(())
}

fn write_report(report: &mut LegacyPythonMigrationReport) -> Result<()> {
    let path = PathBuf::from(&report.backup.backup_dir).join("legacy-python-migration-report.json");
    report.report_path = Some(display_path(&path));
    let payload = serde_json::to_string_pretty(report)
        .map_err(|err| AstrbotError::Pipeline(format!("serialize migration report: {err}")))?;
    fs::write(path, payload).map_err(io_error("write migration report"))
}

fn query_table(
    conn: &Connection,
    table: &str,
    columns: &[&str],
) -> Result<Vec<BTreeMap<String, Value>>> {
    if !table_exists(conn, table)? {
        return Ok(Vec::new());
    }
    let existing = table_columns(conn, table)?;
    let select = columns
        .iter()
        .map(|column| {
            if existing.contains(*column) {
                quote_ident(column)
            } else {
                format!("NULL AS {}", quote_ident(column))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {select} FROM {}", quote_ident(table));
    let mut statement = conn.prepare(&sql).map_err(sqlite_error("prepare query"))?;
    let rows = statement
        .query_map([], |row| row_to_map(row, columns))
        .map_err(sqlite_error("query table"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sqlite_error("collect table rows"))?;
    Ok(rows)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(sqlite_error("check table"))
}

fn table_columns(conn: &Connection, table: &str) -> Result<BTreeSet<String>> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({})", quote_ident(table)))
        .map_err(sqlite_error("prepare table info"))?;
    statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sqlite_error("query table info"))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .map_err(sqlite_error("collect table info"))
}

fn row_to_map(row: &Row<'_>, columns: &[&str]) -> rusqlite::Result<BTreeMap<String, Value>> {
    let mut map = BTreeMap::new();
    for (index, column) in columns.iter().enumerate() {
        map.insert(
            (*column).to_string(),
            sqlite_value_to_json(row.get_ref(index)?),
        );
    }
    Ok(map)
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::Number(Number::from(value)),
        ValueRef::Real(value) => Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => Value::String(String::from_utf8_lossy(value).to_string()),
    }
}

fn read_json_file(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path).map_err(io_error("read json file"))?;
    let content = content.trim_start_matches('\u{feff}');
    serde_json::from_str(content)
        .map_err(|err| AstrbotError::Pipeline(format!("parse json file {}: {err}", path.display())))
}

fn config_persona_to_row(value: Value) -> BTreeMap<String, Value> {
    let mut row = BTreeMap::new();
    if let Value::Object(map) = value {
        for (key, value) in map {
            let target = match key.as_str() {
                "name" => "persona_id",
                "prompt" => "system_prompt",
                _ => key.as_str(),
            };
            row.insert(target.to_string(), value);
        }
    }
    row
}

fn legacy_persona_prompt(row: &BTreeMap<String, Value>) -> String {
    let mut system_prompt = row_text(row, "system_prompt")
        .or_else(|| row_text(row, "prompt"))
        .unwrap_or_default();
    if let Some(mood_dialogs) = row.get("mood_imitation_dialogs").and_then(Value::as_array) {
        let mut parts = Vec::new();
        for (index, dialog) in mood_dialogs.iter().enumerate() {
            let speaker = if index % 2 == 0 { "A" } else { "B" };
            parts.push(format!(
                "{speaker}: {}",
                value_to_plain_text(dialog).unwrap_or_default()
            ));
        }
        if !parts.is_empty() {
            system_prompt.push_str(
                "\nHere are few shots of dialogs, imitate the tone of B when responding:\n",
            );
            system_prompt.push_str(&parts.join("\n"));
        }
    }
    system_prompt
}

fn persona_dialogs(value: Option<&Value>) -> Vec<PersonaDialogTurn> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| match item {
                    Value::Object(map) => {
                        let role = match map.get("role").and_then(Value::as_str) {
                            Some("assistant") => PersonaDialogRole::Assistant,
                            _ => PersonaDialogRole::User,
                        };
                        let content = map
                            .get("content")
                            .and_then(value_to_plain_text)
                            .unwrap_or_default();
                        Some(PersonaDialogTurn::new(role, content))
                    }
                    _ => {
                        let role = if index % 2 == 0 {
                            PersonaDialogRole::User
                        } else {
                            PersonaDialogRole::Assistant
                        };
                        Some(PersonaDialogTurn::new(
                            role,
                            value_to_plain_text(item).unwrap_or_default(),
                        ))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn message_chain_from_legacy(value: &Value) -> MessageChain {
    let components = message_components_from_legacy(value);
    if components.is_empty() {
        MessageChain::plain(value_to_plain_text(value).unwrap_or_default())
    } else {
        MessageChain::new(components)
    }
}

fn message_components_from_legacy(value: &Value) -> Vec<MessageComponent> {
    match value {
        Value::Object(map) => {
            if let Some(content) = map.get("content") {
                return message_components_from_legacy(content);
            }
            match map.get("type").and_then(Value::as_str) {
                Some("image") => map
                    .get("url")
                    .or_else(|| map.get("file"))
                    .or_else(|| map.get("path"))
                    .and_then(Value::as_str)
                    .map(|url| vec![MessageComponent::image(url)])
                    .unwrap_or_default(),
                Some("record") | Some("audio") => map
                    .get("url")
                    .and_then(Value::as_str)
                    .map(|url| vec![MessageComponent::record(url)])
                    .unwrap_or_default(),
                Some("video") => map
                    .get("url")
                    .and_then(Value::as_str)
                    .map(|url| vec![MessageComponent::video(url)])
                    .unwrap_or_default(),
                Some("file") => {
                    let url = map.get("url").and_then(Value::as_str).unwrap_or_default();
                    let name = map.get("name").and_then(Value::as_str).unwrap_or("file");
                    if url.is_empty() {
                        Vec::new()
                    } else {
                        vec![MessageComponent::file(name, url)]
                    }
                }
                _ => value_to_plain_text(value)
                    .map(|text| vec![MessageComponent::plain(text)])
                    .unwrap_or_default(),
            }
        }
        Value::Array(items) => items
            .iter()
            .flat_map(message_components_from_legacy)
            .collect(),
        _ => value_to_plain_text(value)
            .map(|text| vec![MessageComponent::plain(text)])
            .unwrap_or_default(),
    }
}

fn history_items(value: &Value) -> Option<Vec<Value>> {
    let parsed = match value {
        Value::String(value) => serde_json::from_str::<Value>(value).ok()?,
        other => other.clone(),
    };
    parsed.as_array().cloned()
}

fn history_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Null => None,
        other => serde_json::to_string(other).ok(),
    }
}

fn row_text(row: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    value_to_plain_text(row.get(key)?).and_then(non_empty)
}

fn value_to_plain_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(value_to_plain_text)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Value::Object(map) => map
            .get("text")
            .or_else(|| map.get("content"))
            .and_then(value_to_plain_text)
            .or_else(|| serde_json::to_string(value).ok()),
    }
}

fn row_json(row: &BTreeMap<String, Value>, key: &str) -> Option<Value> {
    match row.get(key)? {
        Value::String(value) => serde_json::from_str(value).ok(),
        Value::Null => None,
        other => Some(other.clone()),
    }
}

fn row_string_array(row: &BTreeMap<String, Value>, key: &str) -> Option<Vec<String>> {
    let value = row_json(row, key).or_else(|| row.get(key).cloned())?;
    match value {
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(value_to_plain_text)
                .filter_map(non_empty)
                .collect(),
        ),
        Value::String(value) => non_empty(value).map(|value| vec![value]),
        _ => None,
    }
}

fn row_bool(row: &BTreeMap<String, Value>, key: &str) -> Option<bool> {
    match row.get(key)? {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => value.as_i64().map(|value| value != 0),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn row_i64(row: &BTreeMap<String, Value>, key: &str) -> Option<i64> {
    match row.get(key)? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

fn row_u64(row: &BTreeMap<String, Value>, key: &str) -> Option<u64> {
    row_i64(row, key).and_then(|value| u64::try_from(value).ok())
}

fn set_json_path(root: &mut Value, path: &str, value: Value) {
    let mut current = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if let Value::Object(map) = current {
                map.insert(part.to_string(), value);
            }
            return;
        }
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        let Value::Object(map) = current else {
            return;
        };
        current = map
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

fn table_report(table: &str, source: &str) -> LegacyPythonMigrationTableReport {
    LegacyPythonMigrationTableReport {
        table: table.to_string(),
        source: source.to_string(),
        ..LegacyPythonMigrationTableReport::default()
    }
}

fn push_field(
    report: &mut LegacyPythonMigrationReport,
    table: &str,
    field: &str,
    target: &str,
    table_report: &LegacyPythonMigrationTableReport,
) {
    report.fields.push(LegacyPythonMigrationFieldReport {
        table: table.to_string(),
        field: field.to_string(),
        target: target.to_string(),
        imported: table_report.imported,
        skipped: table_report.skipped,
        note: None,
    });
}

fn map_platform_id(
    platform_id_map: &BTreeMap<String, BTreeMap<String, String>>,
    old_platform_name: &str,
) -> String {
    platform_id_map
        .get(old_platform_name)
        .and_then(|entry| entry.get("platform_id").or_else(|| entry.get("default")))
        .cloned()
        .unwrap_or_else(|| old_platform_name.to_string())
}

fn legacy_platform_from_user_id(
    user_id: &str,
    platform_id_map: &BTreeMap<String, BTreeMap<String, String>>,
) -> String {
    user_id
        .split_once(':')
        .map(|(platform, _)| map_platform_id(platform_id_map, platform))
        .unwrap_or_else(|| "webchat".to_string())
}

fn migration_backup_root(options: &LegacyPythonMigrationOptions) -> PathBuf {
    options.backup_dir.clone().unwrap_or_else(|| {
        options
            .target_sqlite_path
            .parent()
            .map(|parent| parent.join("backups").join("migrations"))
            .unwrap_or_else(|| PathBuf::from("backups").join("migrations"))
    })
}

fn restore_instruction(backup: &LegacyPythonMigrationBackup) -> String {
    format!(
        "Target backup is at {}. Stop the runtime, then copy each target_* backup file to its original_path.",
        backup.backup_dir
    )
}

fn sqlite_sidecars(path: &Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
        path.with_extension("db-wal"),
        path.with_extension("db-shm"),
    ]
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn sqlite_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("legacy sqlite {context}: {err}"))
}

fn io_error(context: &'static str) -> impl FnOnce(io::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("legacy migration {context}: {err}"))
}

fn zip_error(context: &'static str) -> impl FnOnce(zip::result::ZipError) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("legacy migration {context}: {err}"))
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;

    use astrbot_conversation::{ConversationDirectory, SqliteConversationDirectory};
    use astrbot_cron::{CronJobRepository, SqliteCronJobRepository};
    use astrbot_persona::{PersonaRepository, SqlitePersonaRepository};
    use astrbot_storage::{
        ApiKeyRepository, ChatProjectRepository, ChatUiProjectRepository,
        ConversationHistoryRepository, SqliteJsonStore, SqliteStorage,
    };
    use rusqlite::{Connection, params};
    use serde_json::json;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::{LegacyPythonMigrationOptions, run_legacy_python_migration};

    #[tokio::test]
    async fn legacy_python_migration_imports_v4_sqlite_config_and_is_idempotent() {
        let root = temp_root("full");
        let _ = std::fs::remove_dir_all(&root);
        let source_dir = root.join("source");
        let backup_dir = root.join("backups");
        let target_db = root.join("target").join("main.sqlite");
        let target_config = root.join("target").join("cmd_config.json");
        std::fs::create_dir_all(&source_dir).expect("source dir");
        std::fs::create_dir_all(target_config.parent().expect("target parent"))
            .expect("target dir");
        std::fs::write(
            &target_config,
            serde_json::to_string(&json!({
                "dashboard": {
                    "enable": true,
                    "username": "admin",
                    "password": "legacy-password",
                    "jwt_secret": "legacy-jwt",
                    "host": "0.0.0.0",
                    "port": 6199
                },
                "persona": [
                    {
                        "name": "config-persona",
                        "prompt": "from config",
                        "begin_dialogs": ["hi", "hello"]
                    }
                ]
            }))
            .expect("config json"),
        )
        .expect("write config");
        seed_source_db(&source_dir.join("data_v4.db"));

        let report = run_legacy_python_migration(
            LegacyPythonMigrationOptions::new(&target_db)
                .with_target_config_path(&target_config)
                .with_legacy_data_dir(&source_dir)
                .with_backup_dir(&backup_dir),
        )
        .await
        .expect("migration should run");

        assert!(
            report
                .backup
                .files
                .iter()
                .any(|file| file.role == "target_config")
        );
        assert!(
            report
                .report_path
                .as_ref()
                .is_some_and(|path| PathBuf::from(path).is_file())
        );
        assert!(table_imported(&report, "conversations") >= 3);
        assert_eq!(table_imported(&report, "api_keys"), 1);
        assert!(table_imported(&report, "personas") >= 2);
        assert_eq!(table_imported(&report, "platform_sessions"), 1);
        assert_eq!(table_imported(&report, "chatui_projects"), 2);
        assert_eq!(table_imported(&report, "cron_jobs"), 1);

        let migrated_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&target_config).expect("target config"))
                .expect("config parses");
        assert_eq!(migrated_config["dashboard_auth"]["username"], "admin");
        assert_eq!(migrated_config["webchat_server"]["port"], 6199);

        let storage = SqliteStorage::open(&target_db).expect("target sqlite");
        assert_eq!(storage.list_api_keys().await.expect("api keys").len(), 1);
        assert_eq!(
            storage
                .messages_for_conversation("conv-1")
                .await
                .expect("messages")
                .len(),
            2
        );
        assert!(
            storage
                .chatui_project("project-1")
                .await
                .expect("project")
                .is_some()
        );
        assert_eq!(
            ChatProjectRepository::project_sessions(&storage, "project-1")
                .await
                .expect("project sessions")
                .len(),
            1
        );
        let json_store = SqliteJsonStore::open(&target_db).expect("json store");
        let persona_repo = SqlitePersonaRepository::new(json_store.clone());
        assert_eq!(
            persona_repo.list_personas().await.expect("personas").len(),
            2
        );
        let cron_repo = SqliteCronJobRepository::new(json_store.clone());
        assert_eq!(cron_repo.list_jobs(None).await.expect("jobs").len(), 1);
        let conversations = SqliteConversationDirectory::new(json_store);
        assert!(
            conversations
                .conversation("webchat", "conv-1")
                .await
                .expect("conversation")
                .is_some()
        );

        let second = run_legacy_python_migration(
            LegacyPythonMigrationOptions::new(&target_db)
                .with_target_config_path(&target_config)
                .with_legacy_data_dir(&source_dir)
                .with_backup_dir(&backup_dir),
        )
        .await
        .expect("second migration should run");
        assert!(
            second
                .tables
                .iter()
                .map(|table| table.skipped)
                .sum::<usize>()
                > 0
        );
        assert_eq!(storage.list_api_keys().await.expect("api keys").len(), 1);
        assert_eq!(
            storage
                .messages_for_conversation("conv-1")
                .await
                .expect("messages")
                .len(),
            2
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn legacy_python_migration_accepts_backup_zip_source() {
        let root = temp_root("zip");
        let _ = std::fs::remove_dir_all(&root);
        let source_dir = root.join("source");
        let backup_dir = root.join("backups");
        let target_db = root.join("target.sqlite");
        let target_config = root.join("cmd_config.json");
        std::fs::create_dir_all(&source_dir).expect("source dir");
        let config = source_dir.join("cmd_config.json");
        std::fs::write(
            &config,
            r#"{"dashboard":{"username":"zip-admin","port":6200}}"#,
        )
        .expect("config");
        seed_source_db(&source_dir.join("data_v4.db"));
        let zip_path = root.join("python-backup.zip");
        write_legacy_zip(&zip_path, &source_dir);

        let report = run_legacy_python_migration(
            LegacyPythonMigrationOptions::new(&target_db)
                .with_target_config_path(&target_config)
                .with_legacy_backup_zip(&zip_path)
                .with_backup_dir(&backup_dir),
        )
        .await
        .expect("zip migration should run");

        assert_eq!(table_imported(&report, "api_keys"), 1);
        let migrated_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&target_config).expect("target config"))
                .expect("config");
        assert_eq!(migrated_config["dashboard_auth"]["username"], "zip-admin");
        assert_eq!(
            SqliteStorage::open(&target_db)
                .expect("target db")
                .list_api_keys()
                .await
                .expect("api keys")
                .len(),
            1
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn legacy_python_migration_restores_target_after_failure() {
        let root = temp_root("failure");
        let _ = std::fs::remove_dir_all(&root);
        let target_db = root.join("main.sqlite");
        let target_config = root.join("cmd_config.json");
        let bad_source = root.join("data_v4.db");
        let backup_dir = root.join("backups");
        std::fs::create_dir_all(&root).expect("root");
        let original_config = r#"{"webchat_server":{"enabled":true}}"#;
        std::fs::write(&target_config, original_config).expect("target config");
        std::fs::write(&bad_source, b"not sqlite").expect("bad sqlite");

        let error = run_legacy_python_migration(
            LegacyPythonMigrationOptions::new(&target_db)
                .with_target_config_path(&target_config)
                .with_legacy_sqlite_path(&bad_source)
                .with_backup_dir(&backup_dir),
        )
        .await
        .expect_err("bad source should fail");

        assert!(error.to_string().contains("Target backup is at"));
        assert_eq!(
            std::fs::read_to_string(&target_config).expect("restored config"),
            original_config
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn seed_source_db(path: &std::path::Path) {
        let conn = Connection::open(path).expect("source sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE api_keys (
                key_id TEXT PRIMARY KEY,
                name TEXT,
                key_hash TEXT,
                key_prefix TEXT,
                scopes TEXT,
                created_by TEXT,
                last_used_at TEXT,
                expires_at TEXT,
                revoked_at TEXT
            );
            CREATE TABLE conversations (
                conversation_id TEXT PRIMARY KEY,
                platform_id TEXT,
                user_id TEXT,
                content TEXT,
                title TEXT,
                persona_id TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                token_usage INTEGER
            );
            CREATE TABLE persona_folders (
                folder_id TEXT PRIMARY KEY,
                name TEXT,
                parent_id TEXT,
                description TEXT,
                sort_order INTEGER
            );
            CREATE TABLE personas (
                persona_id TEXT PRIMARY KEY,
                system_prompt TEXT,
                begin_dialogs TEXT,
                tools TEXT,
                skills TEXT,
                custom_error_message TEXT,
                folder_id TEXT,
                sort_order INTEGER
            );
            CREATE TABLE platform_sessions (
                session_id TEXT PRIMARY KEY,
                platform_id TEXT,
                creator TEXT,
                display_name TEXT,
                is_group INTEGER,
                created_at TEXT,
                updated_at TEXT
            );
            CREATE TABLE chatui_projects (
                project_id TEXT PRIMARY KEY,
                creator TEXT,
                title TEXT,
                emoji TEXT,
                description TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            CREATE TABLE session_project_relations (
                session_id TEXT UNIQUE,
                project_id TEXT
            );
            CREATE TABLE cron_jobs (
                job_id TEXT PRIMARY KEY,
                name TEXT,
                job_type TEXT,
                cron_expression TEXT,
                timezone TEXT,
                payload TEXT,
                description TEXT,
                enabled INTEGER,
                persistent INTEGER,
                run_once INTEGER,
                status TEXT,
                last_error TEXT
            );
            "#,
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO api_keys VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, NULL)",
            params![
                "key-1",
                "Automation",
                "hash-1",
                "ak_legacy",
                r#"["management.read"]"#,
                "admin"
            ],
        )
        .expect("api key");
        conn.execute(
            "INSERT INTO conversations VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "conv-1",
                "webchat",
                "webchat:user-1",
                r#"[{"role":"user","content":"hello"},{"role":"assistant","content":"world"}]"#,
                "Demo",
                "persona-1",
                1770000000_i64,
                1770000100_i64,
                42_i64
            ],
        )
        .expect("conversation");
        conn.execute(
            "INSERT INTO persona_folders VALUES (?1, ?2, NULL, ?3, ?4)",
            params!["folder-1", "Folder", "Imported", 1_i64],
        )
        .expect("folder");
        conn.execute(
            "INSERT INTO personas VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "persona-1",
                "from sqlite",
                r#"["question","answer"]"#,
                r#"["tool-a"]"#,
                r#"["skill-a"]"#,
                "try later",
                "folder-1",
                2_i64
            ],
        )
        .expect("persona");
        conn.execute(
            "INSERT INTO platform_sessions VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "session-1",
                "webchat",
                "guest",
                "Guest",
                0_i64,
                "2026-05-19T00:00:00Z",
                "2026-05-19T00:01:00Z"
            ],
        )
        .expect("session");
        conn.execute(
            "INSERT INTO chatui_projects VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "project-1",
                "guest",
                "Project",
                "P",
                "Imported project",
                "2026-05-19T00:00:00Z",
                "2026-05-19T00:01:00Z"
            ],
        )
        .expect("project");
        conn.execute(
            "INSERT INTO session_project_relations VALUES (?1, ?2)",
            params!["session-1", "project-1"],
        )
        .expect("relation");
        conn.execute(
            "INSERT INTO cron_jobs VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
            params![
                "job-1",
                "Follow up",
                "active_agent",
                "Asia/Shanghai",
                r#"{"session":"webchat:conv-1","note":"follow","run_at":"2026-05-20T00:00:00Z"}"#,
                "follow",
                1_i64,
                1_i64,
                1_i64,
                "scheduled"
            ],
        )
        .expect("cron");
    }

    fn write_legacy_zip(zip_path: &std::path::Path, source_dir: &std::path::Path) {
        let file = std::fs::File::create(zip_path).expect("zip file");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for filename in ["cmd_config.json", "data_v4.db"] {
            zip.start_file(format!("data/{filename}"), options)
                .expect("start zip file");
            let bytes = std::fs::read(source_dir.join(filename)).expect("source file");
            zip.write_all(&bytes).expect("zip write");
        }
        zip.finish().expect("finish zip");
    }

    fn table_imported(report: &super::LegacyPythonMigrationReport, table: &str) -> usize {
        report
            .tables
            .iter()
            .filter(|entry| entry.table == table)
            .map(|entry| entry.imported)
            .sum()
    }

    fn temp_root(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "astrbot-maintenance-legacy-python-{}-{suffix}",
            std::process::id()
        ))
    }
}
