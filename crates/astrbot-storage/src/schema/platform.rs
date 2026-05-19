use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn platform_tables() -> Vec<StorageTable> {
    vec![
        platform_stats_table(),
        preferences_table(),
        platform_message_history_table(),
        platform_sessions_table(),
        chatui_projects_table(),
        session_project_relations_table(),
        session_rule_sets_table(),
        session_groups_table(),
        kb_profiles_table(),
        kb_documents_table(),
        kb_media_table(),
        kb_chunks_table(),
    ]
}

pub fn platform_stats_table() -> StorageTable {
    StorageTable::new(
        "platform_stats",
        vec![
            StorageColumn::new("timestamp", StorageColumnType::Timestamp),
            StorageColumn::new("platform_id", StorageColumnType::Text),
            StorageColumn::new("platform_type", StorageColumnType::Text),
            StorageColumn::new("count", StorageColumnType::Integer).default_value("0"),
        ],
    )
    .with_unique_key(["timestamp", "platform_id", "platform_type"])
}

pub fn preferences_table() -> StorageTable {
    StorageTable::new(
        "preferences",
        vec![
            StorageColumn::new("scope", StorageColumnType::Text),
            StorageColumn::new("scope_id", StorageColumnType::Text),
            StorageColumn::new("key", StorageColumnType::Text),
            StorageColumn::new("value", StorageColumnType::Json),
        ],
    )
    .with_unique_key(["scope", "scope_id", "key"])
}

pub fn platform_message_history_table() -> StorageTable {
    StorageTable::new(
        "platform_message_history",
        vec![
            StorageColumn::new("platform_id", StorageColumnType::Text),
            StorageColumn::new("user_id", StorageColumnType::Text),
            StorageColumn::new("sender_id", StorageColumnType::Text).nullable(),
            StorageColumn::new("sender_name", StorageColumnType::Text).nullable(),
            StorageColumn::new("content", StorageColumnType::Json),
        ],
    )
}

pub fn platform_sessions_table() -> StorageTable {
    StorageTable::new(
        "platform_sessions",
        vec![
            StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("platform_id", StorageColumnType::Text).default_value("'webchat'"),
            StorageColumn::new("creator", StorageColumnType::Text),
            StorageColumn::new("display_name", StorageColumnType::Text).nullable(),
            StorageColumn::new("is_group", StorageColumnType::Integer).default_value("0"),
            StorageColumn::new("created_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
            StorageColumn::new("updated_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
        ],
    )
}

pub fn chatui_projects_table() -> StorageTable {
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
    )
}

pub fn session_project_relations_table() -> StorageTable {
    StorageTable::new(
        "session_project_relations",
        vec![
            StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("project_id", StorageColumnType::Text),
        ],
    )
}

pub fn session_rule_sets_table() -> StorageTable {
    StorageTable::new(
        "session_rule_sets",
        vec![
            StorageColumn::new("umo", StorageColumnType::Text).primary_key(),
            StorageColumn::new("rule_set", StorageColumnType::Json),
        ],
    )
}

pub fn session_groups_table() -> StorageTable {
    StorageTable::new(
        "session_groups",
        vec![
            StorageColumn::new("group_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("group_record", StorageColumnType::Json),
        ],
    )
}

pub fn kb_profiles_table() -> StorageTable {
    StorageTable::new(
        "kb_profiles",
        vec![
            StorageColumn::new("kb_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("profile", StorageColumnType::Json),
        ],
    )
}

pub fn kb_documents_table() -> StorageTable {
    StorageTable::new(
        "kb_documents",
        vec![
            StorageColumn::new("doc_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("kb_id", StorageColumnType::Text),
            StorageColumn::new("document", StorageColumnType::Json),
        ],
    )
}

pub fn kb_media_table() -> StorageTable {
    StorageTable::new(
        "kb_media",
        vec![
            StorageColumn::new("media_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("doc_id", StorageColumnType::Text),
            StorageColumn::new("kb_id", StorageColumnType::Text),
            StorageColumn::new("media", StorageColumnType::Json),
        ],
    )
}

pub fn kb_chunks_table() -> StorageTable {
    StorageTable::new(
        "kb_chunks",
        vec![
            StorageColumn::new("chunk_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("doc_id", StorageColumnType::Text),
            StorageColumn::new("kb_id", StorageColumnType::Text),
            StorageColumn::new("chunk_index", StorageColumnType::Integer),
            StorageColumn::new("chunk", StorageColumnType::Json),
        ],
    )
}
