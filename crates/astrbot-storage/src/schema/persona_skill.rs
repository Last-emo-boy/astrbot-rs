use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn persona_skill_tables() -> Vec<StorageTable> {
    vec![
        personas_table(),
        persona_folders_table(),
        command_configs_table(),
        command_conflicts_table(),
    ]
}

pub fn personas_table() -> StorageTable {
    StorageTable::new(
        "personas",
        vec![
            StorageColumn::new("persona_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("system_prompt", StorageColumnType::Text),
            StorageColumn::new("begin_dialogs", StorageColumnType::Json).nullable(),
            StorageColumn::new("tools", StorageColumnType::Json).nullable(),
            StorageColumn::new("skills", StorageColumnType::Json).nullable(),
            StorageColumn::new("custom_error_message", StorageColumnType::Text).nullable(),
            StorageColumn::new("folder_id", StorageColumnType::Text).nullable(),
            StorageColumn::new("sort_order", StorageColumnType::Integer).default_value("0"),
        ],
    )
}

pub fn persona_folders_table() -> StorageTable {
    StorageTable::new(
        "persona_folders",
        vec![
            StorageColumn::new("folder_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("name", StorageColumnType::Text),
            StorageColumn::new("parent_id", StorageColumnType::Text).nullable(),
            StorageColumn::new("description", StorageColumnType::Text).nullable(),
            StorageColumn::new("sort_order", StorageColumnType::Integer).default_value("0"),
        ],
    )
}

pub fn command_configs_table() -> StorageTable {
    StorageTable::new(
        "command_configs",
        vec![
            StorageColumn::new("handler_full_name", StorageColumnType::Text).primary_key(),
            StorageColumn::new("plugin_name", StorageColumnType::Text),
            StorageColumn::new("module_path", StorageColumnType::Text),
            StorageColumn::new("original_command", StorageColumnType::Text),
            StorageColumn::new("resolved_command", StorageColumnType::Text).nullable(),
            StorageColumn::new("enabled", StorageColumnType::Boolean).default_value("1"),
            StorageColumn::new("extra_data", StorageColumnType::Json).nullable(),
        ],
    )
}

pub fn command_conflicts_table() -> StorageTable {
    StorageTable::new(
        "command_conflicts",
        vec![
            StorageColumn::new("conflict_key", StorageColumnType::Text),
            StorageColumn::new("handler_full_name", StorageColumnType::Text),
            StorageColumn::new("plugin_name", StorageColumnType::Text),
            StorageColumn::new("status", StorageColumnType::Text).default_value("'pending'"),
        ],
    )
    .with_unique_key(["conflict_key", "handler_full_name"])
}
