use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn platform_tables() -> Vec<StorageTable> {
    vec![
        platform_stats_table(),
        preferences_table(),
        platform_message_history_table(),
        platform_sessions_table(),
        chatui_projects_table(),
        session_project_relations_table(),
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
