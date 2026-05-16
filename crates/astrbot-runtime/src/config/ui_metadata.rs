use super::schema::{ConfigFieldSchema, ConfigValueType, runtime_config_schema};

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigUiGroup {
    pub id: &'static str,
    pub title: &'static str,
    pub fields: Vec<ConfigUiField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigUiField {
    pub path: &'static str,
    pub control: ConfigUiControl,
    pub secret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
                        "provider_fallback.error_message",
                    ],
                ),
            },
            ConfigUiGroup {
                id: "platform",
                title: "Platforms",
                fields: fields_for_group(&schema.fields, &["platforms"]),
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
                    ],
                ),
            },
            ConfigUiGroup {
                id: "webchat",
                title: "WebChat",
                fields: fields_for_group(
                    &schema.fields,
                    &[
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
