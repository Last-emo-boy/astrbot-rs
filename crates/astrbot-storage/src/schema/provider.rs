use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn provider_tables() -> Vec<StorageTable> {
    vec![
        provider_preferences_table(true),
        config_snapshots_table(true),
    ]
}

pub fn provider_preferences_table(with_updated_at: bool) -> StorageTable {
    let mut columns = vec![
        StorageColumn::new("session_id", StorageColumnType::Text).primary_key(),
        StorageColumn::new("provider_id", StorageColumnType::Text),
    ];
    if with_updated_at {
        columns.push(
            StorageColumn::new("updated_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
        );
    }

    StorageTable::new("provider_preferences", columns)
}

pub fn config_snapshots_table(with_created_at: bool) -> StorageTable {
    let mut columns = vec![
        StorageColumn::new("snapshot_id", StorageColumnType::Text).primary_key(),
        StorageColumn::new("config", StorageColumnType::Json),
        StorageColumn::new("note", StorageColumnType::Text).nullable(),
    ];
    if with_created_at {
        columns.push(
            StorageColumn::new("created_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
        );
    }

    StorageTable::new("config_snapshots", columns)
}
