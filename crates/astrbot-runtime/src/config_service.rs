use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config_io::{read_runtime_config, write_runtime_config};
use crate::config_route::{UmopConfigRoute, UmopConfigRouteStore, UmopConfigRouter};
use crate::runtime_config_schema;
use crate::{REDACTED_SECRET, RUNTIME_CONFIG_SCHEMA_VERSION, RuntimeConfig, RuntimeConfigSchema};

pub const DEFAULT_ABCONF_ID: &str = "default";

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
        self.preview_update_value_for_conf(DEFAULT_ABCONF_ID, candidate)
    }

    pub fn preview_update_value_for_conf(
        &self,
        conf_id: &str,
        candidate: Value,
    ) -> Result<RuntimeConfigUpdatePreview> {
        let current = self.read_config_for_conf(conf_id)?;
        let mut next = validate_runtime_config_value(candidate)?;
        preserve_redacted_secrets(&current, &mut next);
        Ok(RuntimeConfigUpdatePreview::new(current, next))
    }

    pub fn save_update_value(&self, candidate: Value) -> Result<RuntimeConfigUpdatePreview> {
        self.save_update_value_for_conf(DEFAULT_ABCONF_ID, candidate)
    }

    pub fn save_update_value_for_conf(
        &self,
        conf_id: &str,
        candidate: Value,
    ) -> Result<RuntimeConfigUpdatePreview> {
        let preview = self.preview_update_value_for_conf(conf_id, candidate)?;
        if preview.plan.write_required {
            if conf_id == DEFAULT_ABCONF_ID || conf_id.trim().is_empty() {
                write_runtime_config(&self.config_path, &preview.config)?;
            } else {
                self.write_abconf_config(conf_id, &preview.config)?;
            }
        }
        Ok(preview)
    }

    pub fn read_config_for_conf(&self, conf_id: &str) -> Result<RuntimeConfig> {
        if conf_id == DEFAULT_ABCONF_ID || conf_id.trim().is_empty() {
            return self.read_config();
        }
        let record = self.get_abconf(conf_id)?.ok_or_else(|| {
            AstrbotError::Pipeline(format!("runtime config {conf_id} does not exist"))
        })?;
        validate_runtime_config_value(record.config)
    }

    pub fn list_abconfs(&self) -> Result<Vec<RuntimeAbconfDescriptor>> {
        let dir = self.abconf_dir();
        if !dir.exists() {
            return Ok(vec![RuntimeAbconfDescriptor::default_config()]);
        }
        let mut records = fs::read_dir(&dir)
            .map_err(|err| AstrbotError::Pipeline(format!("list abconf dir: {err}")))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| self.read_abconf_path(&entry.path()).ok().flatten())
            .map(RuntimeAbconfDescriptor::from)
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.id.cmp(&right.id));
        records.push(RuntimeAbconfDescriptor::default_config());
        Ok(records)
    }

    pub fn create_abconf(
        &self,
        name: impl Into<Option<String>>,
        config: Option<Value>,
    ) -> Result<RuntimeAbconfRecord> {
        let name = non_empty_string(name.into()).unwrap_or_else(|| "Untitled ABConf".to_string());
        let config = match config {
            Some(Value::Null) | None => {
                serde_json::to_value(RuntimeConfig::default()).map_err(|err| {
                    AstrbotError::Pipeline(format!("serialize default config: {err}"))
                })?
            }
            Some(config) => config,
        };
        validate_runtime_config_value(config.clone())?;
        let record = RuntimeAbconfRecord {
            id: new_abconf_id(),
            name,
            config,
        };
        self.write_abconf(&record)?;
        Ok(record)
    }

    pub fn get_abconf(&self, id: &str) -> Result<Option<RuntimeAbconfRecord>> {
        if id.trim().is_empty() || id.trim() == DEFAULT_ABCONF_ID {
            return Ok(Some(RuntimeAbconfRecord {
                id: DEFAULT_ABCONF_ID.to_string(),
                name: DEFAULT_ABCONF_ID.to_string(),
                config: serde_json::to_value(self.read_config()?).map_err(|err| {
                    AstrbotError::Pipeline(format!("serialize default runtime config: {err}"))
                })?,
            }));
        }
        let id = sanitize_abconf_id(id)?;
        self.read_abconf_path(&self.abconf_path(&id))
    }

    pub fn update_abconf_info(
        &self,
        id: &str,
        name: impl Into<Option<String>>,
    ) -> Result<Option<RuntimeAbconfRecord>> {
        if id == DEFAULT_ABCONF_ID {
            return Err(AstrbotError::Pipeline(
                "default runtime config metadata cannot be updated".to_string(),
            ));
        }
        let Some(mut record) = self.get_abconf(id)? else {
            return Ok(None);
        };
        if let Some(name) = non_empty_string(name.into()) {
            record.name = name;
            self.write_abconf(&record)?;
        }
        Ok(Some(record))
    }

    pub fn delete_abconf(&self, id: &str) -> Result<bool> {
        if id == DEFAULT_ABCONF_ID {
            return Err(AstrbotError::Pipeline(
                "default runtime config cannot be deleted".to_string(),
            ));
        }
        let id = sanitize_abconf_id(id)?;
        let path = self.abconf_path(&id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(path)
            .map_err(|err| AstrbotError::Pipeline(format!("delete abconf {id}: {err}")))?;
        Ok(true)
    }

    pub fn read_umop_config_router(&self) -> Result<UmopConfigRouter> {
        self.umop_route_store().load()
    }

    pub fn save_umop_config_routes(&self, routes: &[UmopConfigRoute]) -> Result<()> {
        let router = UmopConfigRouter::new(routes.to_vec())?;
        self.umop_route_store().save(&router)
    }

    pub fn umop_route_store(&self) -> UmopConfigRouteStore {
        UmopConfigRouteStore::new(self.umop_routes_path())
    }

    fn abconf_dir(&self) -> PathBuf {
        self.sidecar_path("abconf")
    }

    fn abconf_path(&self, id: &str) -> PathBuf {
        self.abconf_dir().join(format!("{id}.json"))
    }

    fn read_abconf_path(&self, path: &Path) -> Result<Option<RuntimeAbconfRecord>> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(path)
            .map_err(|err| AstrbotError::Pipeline(format!("read abconf: {err}")))?;
        serde_json::from_str(&raw)
            .map(Some)
            .map_err(|err| AstrbotError::Pipeline(format!("parse abconf: {err}")))
    }

    fn write_abconf(&self, record: &RuntimeAbconfRecord) -> Result<()> {
        let id = sanitize_abconf_id(&record.id)?;
        let dir = self.abconf_dir();
        fs::create_dir_all(&dir)
            .map_err(|err| AstrbotError::Pipeline(format!("create abconf dir: {err}")))?;
        let payload = serde_json::to_string_pretty(record)
            .map_err(|err| AstrbotError::Pipeline(format!("serialize abconf: {err}")))?;
        fs::write(dir.join(format!("{id}.json")), payload)
            .map_err(|err| AstrbotError::Pipeline(format!("write abconf {id}: {err}")))
    }

    fn write_abconf_config(&self, id: &str, config: &RuntimeConfig) -> Result<()> {
        let Some(mut record) = self.get_abconf(id)? else {
            return Err(AstrbotError::Pipeline(format!(
                "runtime config {id} does not exist"
            )));
        };
        record.config = serde_json::to_value(config)
            .map_err(|err| AstrbotError::Pipeline(format!("serialize abconf config: {err}")))?;
        self.write_abconf(&record)
    }

    fn umop_routes_path(&self) -> PathBuf {
        self.sidecar_path("umop_config_routes.json")
    }

    fn sidecar_path(&self, suffix: &str) -> PathBuf {
        let parent = self
            .config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let stem = self
            .config_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or("runtime-config");
        parent.join(format!("{stem}.{suffix}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeAbconfRecord {
    pub id: String,
    pub name: String,
    pub config: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeAbconfDescriptor {
    pub id: String,
    pub name: String,
}

impl RuntimeAbconfDescriptor {
    pub fn default_config() -> Self {
        Self {
            id: DEFAULT_ABCONF_ID.to_string(),
            name: DEFAULT_ABCONF_ID.to_string(),
        }
    }
}

impl From<RuntimeAbconfRecord> for RuntimeAbconfDescriptor {
    fn from(record: RuntimeAbconfRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
        }
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

fn preserve_redacted_secrets(current: &RuntimeConfig, next: &mut RuntimeConfig) {
    for provider in &mut next.chat_providers {
        if let Some(existing) = current
            .chat_providers
            .iter()
            .find(|existing| existing.id == provider.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut provider.api_key);
        }
    }
    for source in &mut next.provider_sources {
        if let Some(existing) = current
            .provider_sources
            .iter()
            .find(|existing| existing.id == source.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut source.api_key);
        }
    }
    for provider in &mut next.speech_to_text_providers {
        if let Some(existing) = current
            .speech_to_text_providers
            .iter()
            .find(|existing| existing.id == provider.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut provider.api_key);
        }
    }
    for provider in &mut next.text_to_speech_providers {
        if let Some(existing) = current
            .text_to_speech_providers
            .iter()
            .find(|existing| existing.id == provider.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut provider.api_key);
        }
    }
    for provider in &mut next.embedding_providers {
        if let Some(existing) = current
            .embedding_providers
            .iter()
            .find(|existing| existing.id == provider.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut provider.api_key);
        }
    }
    for provider in &mut next.rerank_providers {
        if let Some(existing) = current
            .rerank_providers
            .iter()
            .find(|existing| existing.id == provider.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut provider.api_key);
        }
    }
    for runner in &mut next.external_agent_runners {
        if let Some(existing) = current
            .external_agent_runners
            .iter()
            .find(|existing| existing.id == runner.id)
        {
            preserve_redacted_optional_secret(&existing.api_key, &mut runner.api_key);
        }
    }
    for platform in &mut next.platforms {
        if let Some(existing) = current
            .platforms
            .iter()
            .find(|existing| existing.id == platform.id)
        {
            for (key, value) in &mut platform.secrets {
                if value == REDACTED_SECRET {
                    if let Some(existing_value) = existing.secrets.get(key) {
                        *value = existing_value.clone();
                    }
                }
            }
        }
    }

    if next.dashboard_auth.password == REDACTED_SECRET {
        next.dashboard_auth.password = current.dashboard_auth.password.clone();
    }
    if next.dashboard_auth.jwt_secret == REDACTED_SECRET {
        next.dashboard_auth.jwt_secret = current.dashboard_auth.jwt_secret.clone();
    }
}

fn preserve_redacted_optional_secret(current: &Option<String>, next: &mut Option<String>) {
    if next.as_deref() == Some(REDACTED_SECRET) {
        *next = current.clone();
    }
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
        "provider_sources",
        &current.provider_sources,
        &next.provider_sources,
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
        "external_agent_runners",
        &current.external_agent_runners,
        &next.external_agent_runners,
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
        "dashboard_auth",
        &current.dashboard_auth,
        &next.dashboard_auth,
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

fn non_empty_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn sanitize_abconf_id(id: &str) -> Result<String> {
    let id = id.trim();
    if id.is_empty() {
        return Err(AstrbotError::Pipeline("abconf id is required".to_string()));
    }
    let valid = id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    if !valid {
        return Err(AstrbotError::Pipeline(
            "abconf id may only contain ASCII letters, numbers, '-' and '_'".to_string(),
        ));
    }
    Ok(id.to_string())
}

fn new_abconf_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("abconf-{nanos}")
}
