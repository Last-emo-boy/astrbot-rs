use std::path::{Path, PathBuf};

use astrbot_core::{AstrbotError, Result};
use serde::Serialize;
use serde_json::Value;

use crate::config_io::{read_runtime_config, write_runtime_config};
use crate::{
    RUNTIME_CONFIG_SCHEMA_VERSION, RuntimeConfig, RuntimeConfigSchema, runtime_config_schema,
};

#[derive(Clone, Debug)]
pub struct RuntimeConfigService {
    config_path: PathBuf,
}

impl RuntimeConfigService {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn schema(&self) -> RuntimeConfigSchema {
        runtime_config_schema()
    }

    pub fn read_config(&self) -> Result<RuntimeConfig> {
        read_runtime_config(&self.config_path)
    }

    pub fn preview_update_value(&self, candidate: Value) -> Result<RuntimeConfigUpdatePreview> {
        let current = self.read_config()?;
        let next = validate_runtime_config_value(candidate)?;
        Ok(RuntimeConfigUpdatePreview::new(current, next))
    }

    pub fn save_update_value(&self, candidate: Value) -> Result<RuntimeConfigUpdatePreview> {
        let preview = self.preview_update_value(candidate)?;
        if preview.plan.write_required {
            write_runtime_config(&self.config_path, &preview.config)?;
        }
        Ok(preview)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeConfigUpdatePreview {
    pub config: RuntimeConfig,
    pub plan: RuntimeConfigMutationPlan,
}

impl RuntimeConfigUpdatePreview {
    pub fn new(current: RuntimeConfig, next: RuntimeConfig) -> Self {
        let plan = RuntimeConfigMutationPlan::new(&current, &next);
        Self { config: next, plan }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuntimeConfigMutationPlan {
    pub schema_version: u16,
    pub changed_fields: Vec<String>,
    pub reload_action: RuntimeConfigReloadAction,
    pub write_required: bool,
    pub reload_required: bool,
    pub restart_required: bool,
}

impl RuntimeConfigMutationPlan {
    pub fn new(current: &RuntimeConfig, next: &RuntimeConfig) -> Self {
        let changed_fields = changed_runtime_fields(current, next);
        let reload_action = RuntimeConfigReloadAction::from_changed_fields(&changed_fields);
        let write_required = !changed_fields.is_empty();

        Self {
            schema_version: RUNTIME_CONFIG_SCHEMA_VERSION,
            changed_fields,
            reload_action,
            write_required,
            reload_required: reload_action == RuntimeConfigReloadAction::ReloadRuntime,
            restart_required: reload_action == RuntimeConfigReloadAction::RestartRuntime,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeConfigReloadAction {
    Noop,
    ReloadRuntime,
    RestartRuntime,
}

impl RuntimeConfigReloadAction {
    fn from_changed_fields(changed_fields: &[String]) -> Self {
        if changed_fields.is_empty() {
            return Self::Noop;
        }

        if changed_fields.iter().any(|field| {
            matches!(
                field.as_str(),
                "event_queue_capacity" | "paths" | "webchat_server"
            )
        }) {
            Self::RestartRuntime
        } else {
            Self::ReloadRuntime
        }
    }
}

pub fn validate_runtime_config_value(value: Value) -> Result<RuntimeConfig> {
    serde_json::from_value(value)
        .map_err(|err| AstrbotError::Pipeline(format!("validate runtime config: {err}")))
}

fn changed_runtime_fields(current: &RuntimeConfig, next: &RuntimeConfig) -> Vec<String> {
    let mut changed = Vec::new();

    push_changed(
        &mut changed,
        "event_queue_capacity",
        &current.event_queue_capacity,
        &next.event_queue_capacity,
    );
    push_changed(&mut changed, "paths", &current.paths, &next.paths);
    push_changed(
        &mut changed,
        "default_chat_provider_id",
        &current.default_chat_provider_id,
        &next.default_chat_provider_id,
    );
    push_changed(
        &mut changed,
        "chat_providers",
        &current.chat_providers,
        &next.chat_providers,
    );
    push_changed(
        &mut changed,
        "default_speech_to_text_provider_id",
        &current.default_speech_to_text_provider_id,
        &next.default_speech_to_text_provider_id,
    );
    push_changed(
        &mut changed,
        "speech_to_text_providers",
        &current.speech_to_text_providers,
        &next.speech_to_text_providers,
    );
    push_changed(
        &mut changed,
        "default_text_to_speech_provider_id",
        &current.default_text_to_speech_provider_id,
        &next.default_text_to_speech_provider_id,
    );
    push_changed(
        &mut changed,
        "text_to_speech_providers",
        &current.text_to_speech_providers,
        &next.text_to_speech_providers,
    );
    push_changed(
        &mut changed,
        "default_embedding_provider_id",
        &current.default_embedding_provider_id,
        &next.default_embedding_provider_id,
    );
    push_changed(
        &mut changed,
        "embedding_providers",
        &current.embedding_providers,
        &next.embedding_providers,
    );
    push_changed(
        &mut changed,
        "default_rerank_provider_id",
        &current.default_rerank_provider_id,
        &next.default_rerank_provider_id,
    );
    push_changed(
        &mut changed,
        "rerank_providers",
        &current.rerank_providers,
        &next.rerank_providers,
    );
    push_changed(
        &mut changed,
        "platforms",
        &current.platforms,
        &next.platforms,
    );
    push_changed(
        &mut changed,
        "wake_check",
        &current.wake_check,
        &next.wake_check,
    );
    push_changed(
        &mut changed,
        "whitelist_policy",
        &current.whitelist_policy,
        &next.whitelist_policy,
    );
    push_changed(
        &mut changed,
        "session_status",
        &current.session_status,
        &next.session_status,
    );
    push_changed(
        &mut changed,
        "rate_limit",
        &current.rate_limit,
        &next.rate_limit,
    );
    push_changed(
        &mut changed,
        "content_safety",
        &current.content_safety,
        &next.content_safety,
    );
    push_changed(
        &mut changed,
        "provider_fallback",
        &current.provider_fallback,
        &next.provider_fallback,
    );
    push_changed(
        &mut changed,
        "result_decorate",
        &current.result_decorate,
        &next.result_decorate,
    );
    push_changed(
        &mut changed,
        "state_policy",
        &current.state_policy,
        &next.state_policy,
    );
    push_changed(
        &mut changed,
        "webchat_server",
        &current.webchat_server,
        &next.webchat_server,
    );
    push_changed(
        &mut changed,
        "command_plugins",
        &current.command_plugins,
        &next.command_plugins,
    );

    changed
}

fn push_changed<T: PartialEq>(changed: &mut Vec<String>, field: &str, current: &T, next: &T) {
    if current != next {
        changed.push(field.to_string());
    }
}
