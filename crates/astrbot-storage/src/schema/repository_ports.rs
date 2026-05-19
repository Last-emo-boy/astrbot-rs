use super::conversation::{attachment_table, conversation_messages_table, file_token_table};
use super::ops::api_keys_table;
use super::provider::{config_snapshots_table, provider_preferences_table};
use super::{StorageSchema, StorageTable};

pub fn repository_port_schema() -> StorageSchema {
    StorageSchema::new("repository_ports", 1, repository_port_tables())
}

pub fn repository_port_tables() -> Vec<StorageTable> {
    let mut conversation_messages = conversation_messages_table(false);
    if let Some(message_id) = conversation_messages
        .columns
        .iter_mut()
        .find(|column| column.name == "message_id")
    {
        message_id.nullable = true;
    }

    vec![
        conversation_messages,
        provider_preferences_table(false),
        attachment_table(),
        file_token_table(),
        config_snapshots_table(false),
        api_keys_table(),
    ]
}
