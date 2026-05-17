use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn ops_tables() -> Vec<StorageTable> {
    vec![api_keys_table(), cron_jobs_table()]
}

pub fn api_keys_table() -> StorageTable {
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
    )
}

pub fn cron_jobs_table() -> StorageTable {
    StorageTable::new(
        "cron_jobs",
        vec![
            StorageColumn::new("job_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("name", StorageColumnType::Text),
            StorageColumn::new("job_type", StorageColumnType::Text),
            StorageColumn::new("cron_expression", StorageColumnType::Text).nullable(),
            StorageColumn::new("timezone", StorageColumnType::Text).nullable(),
            StorageColumn::new("payload", StorageColumnType::Json),
            StorageColumn::new("enabled", StorageColumnType::Boolean).default_value("1"),
            StorageColumn::new("status", StorageColumnType::Text).default_value("'scheduled'"),
        ],
    )
}
