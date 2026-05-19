use super::schema::{ConfigFieldSchema, ConfigValueType, runtime_config_schema};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfigUiMetadata {
    pub groups: Vec<ConfigUiGroup>,
}

impl ConfigUiMetadata {
    pub fn field(&self, path: &str) -> Option<&ConfigUiField> {
        self.groups
            .iter()
            .flat_map(|group| group.fields.iter())
            .find(|field| field.path == path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfigUiGroup {
    pub id: &'static str,
    pub title: &'static str,
    pub fields: Vec<ConfigUiField>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfigUiField {
    pub path: &'static str,
    pub control: ConfigUiControl,
    pub secret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigUiControl {
    Toggle,
    Number,
    Text,
    Password,
    List,
    Object,
}

pub fn runtime_config_ui_metadata() -> ConfigUiMetadata {
    let schema = runtime_config_schema();

    ConfigUiMetadata {
        groups: vec![
            ConfigUiGroup {
                id: "runtime",
                title: "Runtime",
                fields: fields_for_group(
                    &schema.fields,
                    &[
                        "event_queue_capacity",
                        "paths.root_dir",
                        "paths.data_dir",
                        "paths.temp_dir",
                        "paths.plugin_dir",
                        "paths.attachment_dir",
                        "paths.generated_media_dir",
                    ],
                ),
            },
            ConfigUiGroup {
                id: "provider",
                title: "Providers",
                fields: fields_for_group(
                    &schema.fields,
                    &[
                        "default_chat_provider_id",
                        "chat_providers",
                        "chat_providers[].api_key",
                        "provider_sources",
                        "provider_sources[].api_key",
                        "provider_fallback.error_message",
                        "provider_fallback.wake_prefix",
                    ],
                ),
            },
            ConfigUiGroup {
                id: "platform",
                title: "Platforms",
                fields: fields_for_group(
                    &schema.fields,
                    &[
                        "platforms",
                        "platforms[].options",
                        "platforms[].secrets",
                        "platforms[].secrets.telegram_token",
                        "platforms[].secrets.bot_token",
                        "platforms[].secrets.app_token",
                        "platforms[].secrets.signing_secret",
                        "platforms[].secrets.app_secret",
                        "platforms[].secrets.channel_access_token",
                        "platforms[].secrets.channel_secret",
                        "platforms[].secrets.ws_reverse_token",
                        "platforms[].secrets.corpid",
                        "platforms[].secrets.secret",
                        "platforms[].secrets.wecomaibot_token",
                        "platforms[].secrets.wecomaibot_encoding_aes_key",
                        "platforms[].secrets.wecomaibot_ws_bot_id",
                        "platforms[].secrets.wecomaibot_ws_secret",
                        "platforms[].secrets.client_secret",
                    ],
                ),
            },
            ConfigUiGroup {
                id: "policy",
                title: "Policies",
                fields: fields_for_group(
                    &schema.fields,
                    &[
                        "wake_check.wake_prefixes",
                        "whitelist_policy.enabled",
                        "content_safety.rejection_message",
                        "content_safety.internal_keywords.enabled",
                        "content_safety.internal_keywords.extra_keywords",
                        "content_safety.baidu_aip.enabled",
                        "content_safety.baidu_aip.app_id",
                        "content_safety.baidu_aip.api_key",
                        "content_safety.baidu_aip.secret_key",
                        "result_decorate.reply_prefix",
                        "result_decorate.only_llm_result",
                        "result_decorate.tts_enabled",
                        "result_decorate.tts_provider_id",
                        "result_decorate.tts_dual_output",
                        "result_decorate.tts_use_file_service",
                        "result_decorate.t2i_enabled",
                        "result_decorate.t2i_word_threshold",
                        "result_decorate.t2i_strategy",
                        "result_decorate.t2i_endpoint",
                        "result_decorate.t2i_use_file_service",
                        "result_decorate.t2i_active_template",
                        "result_decorate.content_safety_after_transform",
                    ],
                ),
            },
            ConfigUiGroup {
                id: "webchat",
                title: "WebChat",
                fields: fields_for_group(
                    &schema.fields,
                    &[
                        "dashboard_auth.username",
                        "dashboard_auth.password",
                        "dashboard_auth.jwt_secret",
                        "dashboard_auth.token_ttl_seconds",
                        "webchat_server.enabled",
                        "webchat_server.host",
                        "webchat_server.port",
                    ],
                ),
            },
        ],
    }
}

fn fields_for_group(
    schema_fields: &[ConfigFieldSchema],
    paths: &[&'static str],
) -> Vec<ConfigUiField> {
    paths
        .iter()
        .filter_map(|path| schema_fields.iter().find(|field| field.path == *path))
        .map(|field| ConfigUiField {
            path: field.path,
            control: ui_control_for(field),
            secret: field.secret,
        })
        .collect()
}

fn ui_control_for(field: &ConfigFieldSchema) -> ConfigUiControl {
    if field.secret {
        return ConfigUiControl::Password;
    }

    match field.value_type {
        ConfigValueType::Bool => ConfigUiControl::Toggle,
        ConfigValueType::Integer => ConfigUiControl::Number,
        ConfigValueType::String | ConfigValueType::OptionalString => ConfigUiControl::Text,
        ConfigValueType::List => ConfigUiControl::List,
        ConfigValueType::Object => ConfigUiControl::Object,
    }
}
