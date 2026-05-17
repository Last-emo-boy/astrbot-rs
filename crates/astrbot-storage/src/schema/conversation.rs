use super::{StorageColumn, StorageColumnType, StorageTable};

pub fn conversation_tables() -> Vec<StorageTable> {
    vec![
        conversation_table(),
        conversation_messages_table(true),
        attachment_table(),
    ]
}

pub fn conversation_table() -> StorageTable {
    StorageTable::new(
        "conversations",
        vec![
            StorageColumn::new("conversation_id", StorageColumnType::Text).primary_key(),
            StorageColumn::new("platform_id", StorageColumnType::Text),
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
