#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageColumnType {
    Text,
    Integer,
    Boolean,
    Json,
    Binary,
    Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageColumn {
    pub name: String,
    pub column_type: StorageColumnType,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub default_value: Option<String>,
}

impl StorageColumn {
    pub fn new(name: impl Into<String>, column_type: StorageColumnType) -> Self {
        Self {
            name: name.into(),
            column_type,
            nullable: false,
            primary_key: false,
            unique: false,
            default_value: None,
        }
    }

    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    pub fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self
    }

    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    pub fn default_value(mut self, default_value: impl Into<String>) -> Self {
        self.default_value = Some(default_value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageTable {
    pub name: String,
    pub columns: Vec<StorageColumn>,
    pub unique_keys: Vec<Vec<String>>,
}

impl StorageTable {
    pub fn new(name: impl Into<String>, columns: Vec<StorageColumn>) -> Self {
        Self {
            name: name.into(),
            columns,
            unique_keys: Vec::new(),
        }
    }

    pub fn with_unique_key<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.unique_keys
            .push(columns.into_iter().map(Into::into).collect());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageSchema {
    pub name: String,
    pub version: u32,
    pub tables: Vec<StorageTable>,
}

impl StorageSchema {
    pub fn new(name: impl Into<String>, version: u32, tables: Vec<StorageTable>) -> Self {
        Self {
            name: name.into(),
            version,
            tables,
        }
    }

    pub fn table(&self, name: &str) -> Option<&StorageTable> {
        self.tables.iter().find(|table| table.name == name)
    }

    pub fn repository_port_schema() -> Self {
        Self::new(
            "repository_ports",
            1,
            vec![
                StorageTable::new(
                    "conversation_messages",
                    vec![
                        StorageColumn::new("message_id", StorageColumnType::Text)
                            .primary_key()
                            .nullable(),
                        StorageColumn::new("platform_id", StorageColumnType::Text),
                        StorageColumn::new("conversation_id", StorageColumnType::Text),
                        StorageColumn::new("message_chain", StorageColumnType::Json),
                    ],
                ),
                StorageTable::new(
                    "provider_preferences",
                    vec![
                        StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("provider_id", StorageColumnType::Text),
                    ],
                ),
                StorageTable::new(
                    "attachments",
                    vec![
                        StorageColumn::new("attachment_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("source_url", StorageColumnType::Text),
                        StorageColumn::new("stored_url", StorageColumnType::Text).nullable(),
                        StorageColumn::new("filename", StorageColumnType::Text).nullable(),
                        StorageColumn::new("content_type", StorageColumnType::Text).nullable(),
                    ],
                ),
                StorageTable::new(
                    "config_snapshots",
                    vec![
                        StorageColumn::new("snapshot_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("config", StorageColumnType::Json),
                        StorageColumn::new("note", StorageColumnType::Text).nullable(),
                    ],
                ),
                StorageTable::new(
                    "api_keys",
                    vec![
                        StorageColumn::new("key_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("name", StorageColumnType::Text),
                        StorageColumn::new("key_hash", StorageColumnType::Text).unique(),
                        StorageColumn::new("key_prefix", StorageColumnType::Text),
                        StorageColumn::new("scopes", StorageColumnType::Json).nullable(),
                        StorageColumn::new("created_by", StorageColumnType::Text),
                        StorageColumn::new("expires_at", StorageColumnType::Timestamp).nullable(),
                        StorageColumn::new("revoked_at", StorageColumnType::Timestamp).nullable(),
                    ],
                ),
            ],
        )
    }

    pub fn astrbot_main_v4() -> Self {
        Self::new(
            "main_db",
            4,
            vec![
                StorageTable::new(
                    "conversations",
                    vec![
                        StorageColumn::new("conversation_id", StorageColumnType::Text)
                            .primary_key(),
                        StorageColumn::new("platform_id", StorageColumnType::Text),
                        StorageColumn::new("created_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                        StorageColumn::new("updated_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                    ],
                ),
                StorageTable::new(
                    "conversation_messages",
                    vec![
                        StorageColumn::new("message_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("conversation_id", StorageColumnType::Text),
                        StorageColumn::new("platform_id", StorageColumnType::Text),
                        StorageColumn::new("message_chain", StorageColumnType::Json),
                        StorageColumn::new("created_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                    ],
                ),
                StorageTable::new(
                    "provider_preferences",
                    vec![
                        StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("provider_id", StorageColumnType::Text),
                        StorageColumn::new("updated_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                    ],
                ),
                StorageTable::new(
                    "attachments",
                    vec![
                        StorageColumn::new("attachment_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("source_url", StorageColumnType::Text),
                        StorageColumn::new("stored_url", StorageColumnType::Text).nullable(),
                        StorageColumn::new("filename", StorageColumnType::Text).nullable(),
                        StorageColumn::new("content_type", StorageColumnType::Text).nullable(),
                    ],
                ),
                StorageTable::new(
                    "config_snapshots",
                    vec![
                        StorageColumn::new("snapshot_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("config", StorageColumnType::Json),
                        StorageColumn::new("note", StorageColumnType::Text).nullable(),
                        StorageColumn::new("created_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                    ],
                ),
                StorageTable::new(
                    "platform_stats",
                    vec![
                        StorageColumn::new("timestamp", StorageColumnType::Timestamp),
                        StorageColumn::new("platform_id", StorageColumnType::Text),
                        StorageColumn::new("platform_type", StorageColumnType::Text),
                        StorageColumn::new("count", StorageColumnType::Integer).default_value("0"),
                    ],
                ),
                StorageTable::new(
                    "preferences",
                    vec![
                        StorageColumn::new("scope", StorageColumnType::Text),
                        StorageColumn::new("scope_id", StorageColumnType::Text),
                        StorageColumn::new("key", StorageColumnType::Text),
                        StorageColumn::new("value", StorageColumnType::Json),
                    ],
                )
                .with_unique_key(["scope", "scope_id", "key"]),
                StorageTable::new(
                    "personas",
                    vec![
                        StorageColumn::new("persona_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("system_prompt", StorageColumnType::Text),
                        StorageColumn::new("begin_dialogs", StorageColumnType::Json).nullable(),
                        StorageColumn::new("tools", StorageColumnType::Json).nullable(),
                        StorageColumn::new("skills", StorageColumnType::Json).nullable(),
                        StorageColumn::new("folder_id", StorageColumnType::Text).nullable(),
                        StorageColumn::new("sort_order", StorageColumnType::Integer)
                            .default_value("0"),
                    ],
                ),
                StorageTable::new(
                    "persona_folders",
                    vec![
                        StorageColumn::new("folder_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("name", StorageColumnType::Text),
                        StorageColumn::new("parent_id", StorageColumnType::Text).nullable(),
                        StorageColumn::new("description", StorageColumnType::Text).nullable(),
                        StorageColumn::new("sort_order", StorageColumnType::Integer)
                            .default_value("0"),
                    ],
                ),
                StorageTable::new(
                    "platform_message_history",
                    vec![
                        StorageColumn::new("platform_id", StorageColumnType::Text),
                        StorageColumn::new("user_id", StorageColumnType::Text),
                        StorageColumn::new("sender_id", StorageColumnType::Text).nullable(),
                        StorageColumn::new("sender_name", StorageColumnType::Text).nullable(),
                        StorageColumn::new("content", StorageColumnType::Json),
                    ],
                ),
                StorageTable::new(
                    "platform_sessions",
                    vec![
                        StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("platform_id", StorageColumnType::Text)
                            .default_value("'webchat'"),
                        StorageColumn::new("creator", StorageColumnType::Text),
                        StorageColumn::new("display_name", StorageColumnType::Text).nullable(),
                        StorageColumn::new("is_group", StorageColumnType::Integer)
                            .default_value("0"),
                    ],
                ),
                StorageTable::new(
                    "api_keys",
                    vec![
                        StorageColumn::new("key_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("name", StorageColumnType::Text),
                        StorageColumn::new("key_hash", StorageColumnType::Text).unique(),
                        StorageColumn::new("key_prefix", StorageColumnType::Text),
                        StorageColumn::new("scopes", StorageColumnType::Json).nullable(),
                        StorageColumn::new("created_by", StorageColumnType::Text),
                        StorageColumn::new("expires_at", StorageColumnType::Timestamp).nullable(),
                        StorageColumn::new("revoked_at", StorageColumnType::Timestamp).nullable(),
                    ],
                ),
                StorageTable::new(
                    "cron_jobs",
                    vec![
                        StorageColumn::new("job_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("name", StorageColumnType::Text),
                        StorageColumn::new("job_type", StorageColumnType::Text),
                        StorageColumn::new("cron_expression", StorageColumnType::Text).nullable(),
                        StorageColumn::new("timezone", StorageColumnType::Text).nullable(),
                        StorageColumn::new("payload", StorageColumnType::Json),
                        StorageColumn::new("enabled", StorageColumnType::Boolean)
                            .default_value("1"),
                        StorageColumn::new("status", StorageColumnType::Text)
                            .default_value("'scheduled'"),
                    ],
                ),
                StorageTable::new(
                    "chatui_projects",
                    vec![
                        StorageColumn::new("project_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("creator", StorageColumnType::Text),
                        StorageColumn::new("emoji", StorageColumnType::Text).nullable(),
                        StorageColumn::new("title", StorageColumnType::Text),
                        StorageColumn::new("description", StorageColumnType::Text).nullable(),
                        StorageColumn::new("created_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                        StorageColumn::new("updated_at", StorageColumnType::Timestamp)
                            .default_value("CURRENT_TIMESTAMP"),
                    ],
                ),
                StorageTable::new(
                    "session_project_relations",
                    vec![
                        StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
                        StorageColumn::new("project_id", StorageColumnType::Text),
                    ],
                ),
                StorageTable::new(
                    "command_configs",
                    vec![
                        StorageColumn::new("handler_full_name", StorageColumnType::Text)
                            .primary_key(),
                        StorageColumn::new("plugin_name", StorageColumnType::Text),
                        StorageColumn::new("module_path", StorageColumnType::Text),
                        StorageColumn::new("original_command", StorageColumnType::Text),
                        StorageColumn::new("resolved_command", StorageColumnType::Text).nullable(),
                        StorageColumn::new("enabled", StorageColumnType::Boolean)
                            .default_value("1"),
                        StorageColumn::new("extra_data", StorageColumnType::Json).nullable(),
                    ],
                ),
                StorageTable::new(
                    "command_conflicts",
                    vec![
                        StorageColumn::new("conflict_key", StorageColumnType::Text),
                        StorageColumn::new("handler_full_name", StorageColumnType::Text),
                        StorageColumn::new("plugin_name", StorageColumnType::Text),
                        StorageColumn::new("status", StorageColumnType::Text)
                            .default_value("'pending'"),
                    ],
                )
                .with_unique_key(["conflict_key", "handler_full_name"]),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::StorageSchema;

    #[test]
    fn repository_port_schema_names_existing_tables() {
        let schema = StorageSchema::repository_port_schema();

        assert_eq!(schema.version, 1);
        assert!(schema.table("conversation_messages").is_some());
        assert!(schema.table("provider_preferences").is_some());
        assert!(schema.table("attachments").is_some());
        assert!(schema.table("config_snapshots").is_some());
        assert!(schema.table("api_keys").is_some());
    }

    #[test]
    fn astrbot_main_schema_keeps_identity_and_core_tables() {
        let schema = StorageSchema::astrbot_main_v4();

        assert_eq!(schema.name, "main_db");
        assert_eq!(schema.version, 4);
        assert!(schema.table("conversations").is_some());
        assert!(schema.table("platform_stats").is_some());
        assert!(schema.table("preferences").is_some());
        assert!(schema.table("personas").is_some());
        assert!(schema.table("api_keys").is_some());
        assert!(schema.table("cron_jobs").is_some());
        assert!(schema.table("command_configs").is_some());
    }
}
