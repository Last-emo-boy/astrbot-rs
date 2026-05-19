use serde::Serialize;
use serde_json::{Value, json};

use crate::RuntimeConfig;

pub const RUNTIME_CONFIG_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeConfigSchema {
    pub version: u16,
    pub fields: Vec<ConfigFieldSchema>,
}

impl RuntimeConfigSchema {
    pub fn field(&self, path: &str) -> Option<&ConfigFieldSchema> {
        self.fields.iter().find(|field| field.path == path)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConfigFieldSchema {
    pub path: &'static str,
    pub value_type: ConfigValueType,
    pub default_value: Value,
    pub secret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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
            field(
                "provider_sources",
                ConfigValueType::List,
                json!(default.provider_sources),
            ),
            secret_field(
                "provider_sources[].api_key",
                ConfigValueType::OptionalString,
            ),
            field("platforms", ConfigValueType::List, json!(default.platforms)),
            field("platforms[].options", ConfigValueType::Object, json!({})),
            secret_field("platforms[].secrets", ConfigValueType::Object),
            secret_field(
                "platforms[].secrets.telegram_token",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.bot_token",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.app_token",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.signing_secret",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.app_secret",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.channel_access_token",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.channel_secret",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.ws_reverse_token",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.corpid",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.secret",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.wecomaibot_token",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.wecomaibot_encoding_aes_key",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.wecomaibot_ws_bot_id",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.wecomaibot_ws_secret",
                ConfigValueType::OptionalString,
            ),
            secret_field(
                "platforms[].secrets.client_secret",
                ConfigValueType::OptionalString,
            ),
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
                "content_safety.internal_keywords.enabled",
                ConfigValueType::Bool,
                json!(default.content_safety.internal_keywords.enabled),
            ),
            field(
                "content_safety.internal_keywords.extra_keywords",
                ConfigValueType::List,
                json!(default.content_safety.internal_keywords.extra_keywords),
            ),
            field(
                "content_safety.baidu_aip.enabled",
                ConfigValueType::Bool,
                json!(default.content_safety.baidu_aip.enabled),
            ),
            field(
                "content_safety.baidu_aip.app_id",
                ConfigValueType::String,
                json!(default.content_safety.baidu_aip.app_id),
            ),
            secret_field("content_safety.baidu_aip.api_key", ConfigValueType::String),
            secret_field(
                "content_safety.baidu_aip.secret_key",
                ConfigValueType::String,
            ),
            field(
                "provider_fallback.error_message",
                ConfigValueType::OptionalString,
                json!(default.provider_fallback.error_message),
            ),
            field(
                "provider_fallback.wake_prefix",
                ConfigValueType::String,
                json!(default.provider_fallback.wake_prefix),
            ),
            field(
                "result_decorate.reply_prefix",
                ConfigValueType::OptionalString,
                json!(default.result_decorate.reply_prefix),
            ),
            field(
                "result_decorate.only_llm_result",
                ConfigValueType::Bool,
                json!(default.result_decorate.only_llm_result),
            ),
            field(
                "result_decorate.tts_enabled",
                ConfigValueType::Bool,
                json!(default.result_decorate.tts_enabled),
            ),
            field(
                "result_decorate.tts_provider_id",
                ConfigValueType::OptionalString,
                json!(default.result_decorate.tts_provider_id),
            ),
            field(
                "result_decorate.tts_dual_output",
                ConfigValueType::Bool,
                json!(default.result_decorate.tts_dual_output),
            ),
            field(
                "result_decorate.tts_use_file_service",
                ConfigValueType::Bool,
                json!(default.result_decorate.tts_use_file_service),
            ),
            field(
                "result_decorate.t2i_enabled",
                ConfigValueType::Bool,
                json!(default.result_decorate.t2i_enabled),
            ),
            field(
                "result_decorate.t2i_word_threshold",
                ConfigValueType::Integer,
                json!(default.result_decorate.t2i_word_threshold),
            ),
            field(
                "result_decorate.t2i_strategy",
                ConfigValueType::String,
                json!(default.result_decorate.t2i_strategy),
            ),
            field(
                "result_decorate.t2i_endpoint",
                ConfigValueType::OptionalString,
                json!(default.result_decorate.t2i_endpoint),
            ),
            field(
                "result_decorate.t2i_use_file_service",
                ConfigValueType::Bool,
                json!(default.result_decorate.t2i_use_file_service),
            ),
            field(
                "result_decorate.t2i_active_template",
                ConfigValueType::String,
                json!(default.result_decorate.t2i_active_template),
            ),
            field(
                "result_decorate.content_safety_after_transform",
                ConfigValueType::Bool,
                json!(default.result_decorate.content_safety_after_transform),
            ),
            field(
                "dashboard_auth.username",
                ConfigValueType::String,
                json!(default.dashboard_auth.username),
            ),
            secret_field("dashboard_auth.password", ConfigValueType::String),
            secret_field("dashboard_auth.jwt_secret", ConfigValueType::String),
            field(
                "dashboard_auth.token_ttl_seconds",
                ConfigValueType::Integer,
                json!(default.dashboard_auth.token_ttl_seconds),
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
