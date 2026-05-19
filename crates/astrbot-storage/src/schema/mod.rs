mod conversation;
mod main;
mod ops;
mod persona_skill;
mod platform;
mod provider;
mod repository_ports;
mod table;

pub use table::{StorageColumn, StorageColumnType, StorageSchema, StorageTable};

impl StorageSchema {
    pub fn repository_port_schema() -> Self {
        repository_ports::repository_port_schema()
    }

    pub fn astrbot_main_v4() -> Self {
        main::astrbot_main_v4()
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
        assert!(schema.table("file_tokens").is_some());
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
        assert!(schema.table("file_tokens").is_some());
        assert!(schema.table("session_rule_sets").is_some());
        assert!(schema.table("kb_profiles").is_some());
    }

    #[test]
    fn table_families_can_evolve_without_reordering_public_schema_contract() {
        let table_names = StorageSchema::astrbot_main_v4()
            .tables
            .into_iter()
            .map(|table| table.name)
            .collect::<Vec<_>>();

        assert_eq!(
            table_names,
            vec![
                "conversations",
                "conversation_messages",
                "attachments",
                "conversation_references",
                "memory_records",
                "platform_bindings",
                "file_tokens",
                "provider_preferences",
                "config_snapshots",
                "platform_stats",
                "preferences",
                "platform_message_history",
                "platform_sessions",
                "chatui_projects",
                "session_project_relations",
                "session_rule_sets",
                "session_groups",
                "kb_profiles",
                "kb_documents",
                "kb_media",
                "kb_chunks",
                "personas",
                "persona_folders",
                "command_configs",
                "command_conflicts",
                "api_keys",
                "cron_jobs",
                "storage_migrations",
            ]
        );
    }
}
