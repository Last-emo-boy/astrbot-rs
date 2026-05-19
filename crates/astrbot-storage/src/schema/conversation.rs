use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn conversation_tables() -> Vec<StorageTable> {
    vec![
        conversation_table(),
        conversation_messages_table(true),
        attachment_table(),
        conversation_references_table(),
        memory_records_table(),
        platform_bindings_table(),
        file_token_table(),
    ]
}

pub fn conversation_table() -> StorageTable {
    StorageTable::new(
        "conversations",
        vec![
            StorageColumn::new("conversation_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("platform_id", StorageColumnType::Text),
            StorageColumn::new("title", StorageColumnType::Text).nullable(),
            StorageColumn::new("persona_id", StorageColumnType::Text).nullable(),
            StorageColumn::new("is_current", StorageColumnType::Boolean).default_value("0"),
            StorageColumn::new("created_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
            StorageColumn::new("updated_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
        ],
    )
}

pub fn conversation_messages_table(with_created_at: bool) -> StorageTable {
    let mut columns = vec![
        StorageColumn::new("message_id", StorageColumnType::Text).primary_key(),
        StorageColumn::new("conversation_id", StorageColumnType::Text),
        StorageColumn::new("platform_id", StorageColumnType::Text),
        StorageColumn::new("message_chain", StorageColumnType::Json),
    ];
    if with_created_at {
        columns.push(
            StorageColumn::new("created_at", StorageColumnType::Timestamp)
                .default_value("CURRENT_TIMESTAMP"),
        );
    }

    StorageTable::new("conversation_messages", columns)
}

pub fn attachment_table() -> StorageTable {
    StorageTable::new(
        "attachments",
        vec![
            StorageColumn::new("attachment_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("source_url", StorageColumnType::Text),
            StorageColumn::new("stored_url", StorageColumnType::Text).nullable(),
            StorageColumn::new("filename", StorageColumnType::Text).nullable(),
            StorageColumn::new("content_type", StorageColumnType::Text).nullable(),
        ],
    )
}

pub fn conversation_references_table() -> StorageTable {
    StorageTable::new(
        "conversation_references",
        vec![
            StorageColumn::new("conversation_id", StorageColumnType::Text),
            StorageColumn::new("message_id", StorageColumnType::Text),
            StorageColumn::new("refs", StorageColumnType::Json),
        ],
    )
    .with_unique_key(["conversation_id", "message_id"])
}

pub fn memory_records_table() -> StorageTable {
    StorageTable::new(
        "memory_records",
        vec![
            StorageColumn::new("record_id", StorageColumnType::Integer).primary_key(),
            StorageColumn::new("platform_id", StorageColumnType::Text),
            StorageColumn::new("conversation_id", StorageColumnType::Text),
            StorageColumn::new("record", StorageColumnType::Json),
        ],
    )
}

pub fn platform_bindings_table() -> StorageTable {
    StorageTable::new(
        "platform_bindings",
        vec![
            StorageColumn::new("platform_id", StorageColumnType::Text),
            StorageColumn::new("conversation_id", StorageColumnType::Text),
            StorageColumn::new("record", StorageColumnType::Json),
        ],
    )
    .with_unique_key(["platform_id", "conversation_id"])
}

pub fn file_token_table() -> StorageTable {
    StorageTable::new(
        "file_tokens",
        vec![
            StorageColumn::new("token", StorageColumnType::Text).primary_key(),
            StorageColumn::new("file_path", StorageColumnType::Text),
            StorageColumn::new("scope", StorageColumnType::Text),
            StorageColumn::new("filename", StorageColumnType::Text).nullable(),
            StorageColumn::new("content_type", StorageColumnType::Text).nullable(),
            StorageColumn::new("expires_at_unix", StorageColumnType::Integer).nullable(),
            StorageColumn::new("single_use", StorageColumnType::Boolean).default_value("1"),
        ],
    )
}
