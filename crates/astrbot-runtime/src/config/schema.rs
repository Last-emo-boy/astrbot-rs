use serde_json::{Value, json};

use crate::RuntimeConfig;

pub const RUNTIME_CONFIG_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeConfigSchema {
    pub version: u16,
    pub fields: Vec<ConfigFieldSchema>,
}

impl RuntimeConfigSchema {
    pub fn field(&self, path: &str) -> Option<&ConfigFieldSchema> {
        self.fields.iter().find(|field| field.path == path)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigFieldSchema {
    pub path: &'static str,
    pub value_type: ConfigValueType,
    pub default_value: Value,
    pub secret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigValueType {
    Bool,
    Integer,
    String,
    OptionalString,
    List,
    Object,
}

pub fn runtime_config_schema() -> RuntimeConfigSchema {
    let default = RuntimeConfig::default();

    RuntimeConfigSchema {
        version: RUNTIME_CONFIG_SCHEMA_VERSION,
        fields: vec![
            field(
                "event_queue_capacity",
                ConfigValueType::Integer,
                json!(default.event_queue_capacity),
            ),
            field("paths", ConfigValueType::Object, json!(default.paths)),
            field(
                "paths.root_dir",
                ConfigValueType::OptionalString,
                json!(default.paths.root_dir),
            ),
            field(
                "paths.data_dir",
                ConfigValueType::OptionalString,
                json!(default.paths.data_dir),
            ),
            field(
                "paths.temp_dir",
                ConfigValueType::OptionalString,
                json!(default.paths.temp_dir),
            ),
            field(
                "paths.plugin_dir",
                ConfigValueType::OptionalString,
                json!(default.paths.plugin_dir),
            ),
            field(
                "paths.attachment_dir",
                ConfigValueType::OptionalString,
                json!(default.paths.attachment_dir),
            ),
            field(
                "paths.generated_media_dir",
                ConfigValueType::OptionalString,
                json!(default.paths.generated_media_dir),
            ),
            field(
                "default_chat_provider_id",
                ConfigValueType::String,
                json!(default.default_chat_provider_id),
            ),
            field(
                "chat_providers",
                ConfigValueType::List,
                json!(default.chat_providers),
            ),
            secret_field("chat_providers[].api_key", ConfigValueType::OptionalString),
            field("platforms", ConfigValueType::List, json!(default.platforms)),
            field(
                "wake_check.wake_prefixes",
                ConfigValueType::List,
                json!(default.wake_check.wake_prefixes),
            ),
            field(
                "whitelist_policy.enabled",
                ConfigValueType::Bool,
                json!(default.whitelist_policy.enabled),
            ),
            field(
                "content_safety.rejection_message",
                ConfigValueType::OptionalString,
                json!(default.content_safety.rejection_message),
            ),
            field(
                "provider_fallback.error_message",
                ConfigValueType::OptionalString,
                json!(default.provider_fallback.error_message),
            ),
            field(
                "webchat_server.enabled",
                ConfigValueType::Bool,
                json!(default.webchat_server.enabled),
            ),
            field(
                "webchat_server.host",
                ConfigValueType::String,
                json!(default.webchat_server.host),
            ),
            field(
                "webchat_server.port",
                ConfigValueType::Integer,
                json!(default.webchat_server.port),
            ),
        ],
    }
}

fn field(
    path: &'static str,
    value_type: ConfigValueType,
    default_value: Value,
) -> ConfigFieldSchema {
    ConfigFieldSchema {
        path,
        value_type,
        default_value,
        secret: false,
    }
}

fn secret_field(path: &'static str, value_type: ConfigValueType) -> ConfigFieldSchema {
    ConfigFieldSchema {
        path,
        value_type,
        default_value: Value::Null,
        secret: true,
    }
}
