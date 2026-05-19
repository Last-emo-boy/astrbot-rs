use std::{
    collections::BTreeSet,
    sync::{Arc, RwLock},
};

use astrbot_agent::{HandoffToolSpec, ResolvedSubagent, SubagentConfig, SubagentConfigSource};
use astrbot_core::Result as AstrbotResult;
use astrbot_storage::SqliteJsonStore;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

const SUBAGENT_CONFIG_NAMESPACE: &str = "subagent_orchestrator";
const SUBAGENT_CONFIG_KEY: &str = "config";

#[derive(Clone, Debug)]
pub struct ManagementSubagentState {
    config: Arc<RwLock<ManagementSubagentConfig>>,
    executions: Arc<RwLock<Vec<ManagementSubagentExecutionRecord>>>,
    execution_bridge: Option<Arc<dyn ManagementSubagentExecutionBridge>>,
    store: Option<SqliteJsonStore>,
}

impl ManagementSubagentState {
    pub fn new(source: SubagentConfigSource) -> Self {
        Self::from_config(ManagementSubagentConfig::from_source(source))
    }

    pub fn from_config(config: ManagementSubagentConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            executions: Arc::new(RwLock::new(Vec::new())),
            execution_bridge: None,
            store: None,
        }
    }

    pub fn sqlite(
        store: SqliteJsonStore,
        default_config: ManagementSubagentConfig,
    ) -> AstrbotResult<Self> {
        let config = store
            .get_json(SUBAGENT_CONFIG_NAMESPACE, SUBAGENT_CONFIG_KEY)?
            .unwrap_or(default_config);
        Ok(Self::from_config(config).with_store(store))
    }

    pub fn with_execution_bridge(
        mut self,
        execution_bridge: Arc<dyn ManagementSubagentExecutionBridge>,
    ) -> Self {
        self.execution_bridge = Some(execution_bridge);
        self
    }

    pub fn with_store(mut self, store: SqliteJsonStore) -> Self {
        self.store = Some(store);
        self
    }

    fn catalog_response(
        &self,
    ) -> Result<ManagementSubagentCatalogResponse, ManagementSubagentError> {
        let config = self
            .config
            .read()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))?
            .clone();
        let executions = self
            .executions
            .read()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))?
            .clone();
        Ok(catalog_from_config(&config, executions))
    }

    fn source_config(&self) -> Result<ManagementSubagentConfig, ManagementSubagentError> {
        Ok(self
            .config
            .read()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))?
            .clone())
    }

    fn replace_config(
        &self,
        config: ManagementSubagentConfig,
    ) -> Result<ManagementSubagentCatalogResponse, ManagementSubagentError> {
        let config = normalize_config(config)?;
        if let Some(store) = &self.store {
            store
                .put_json(SUBAGENT_CONFIG_NAMESPACE, SUBAGENT_CONFIG_KEY, &config)
                .map_err(|error| ManagementSubagentError::Persistence(error.to_string()))?;
        }
        *self
            .config
            .write()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))? =
            config.clone();
        let executions = self
            .executions
            .read()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))?
            .clone();
        Ok(catalog_from_config(&config, executions))
    }

    fn replace_agents(
        &self,
        agents: Vec<SubagentConfig>,
    ) -> Result<ManagementSubagentCatalogResponse, ManagementSubagentError> {
        let mut config = self.source_config()?;
        config.agents = agents;
        self.replace_config(config)
    }

    fn execute(
        &self,
        request: ManagementSubagentExecuteRequest,
    ) -> Result<ManagementSubagentExecuteResponse, ManagementSubagentError> {
        let bridge = self
            .execution_bridge
            .as_ref()
            .ok_or(ManagementSubagentError::ExecutionUnavailable)?
            .clone();
        let config = self
            .config
            .read()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))?
            .clone();
        let agent_config = config
            .source()
            .enabled_agents()
            .into_iter()
            .find(|agent| agent.name == request.agent_name)
            .ok_or_else(|| ManagementSubagentError::NotFound(request.agent_name.clone()))?;
        let resolved = ResolvedSubagent::from_config(agent_config, None);
        let handoff = HandoffToolSpec::from_subagent(resolved.clone());
        let result = bridge
            .execute(&resolved, &request)
            .map_err(ManagementSubagentError::Execution)?;
        let mut executions = self
            .executions
            .write()
            .map_err(|error| ManagementSubagentError::StateLock(error.to_string()))?;
        let execution = ManagementSubagentExecutionRecord {
            run_id: format!("subagent-run-{}", executions.len() + 1),
            agent_name: resolved.name.clone(),
            handoff_tool: handoff.name,
            input: request.input,
            output: result.output,
            status: result.status,
            background: request.background,
            provider_id: resolved.provider_id,
            persona_id: resolved.persona_id,
            tools: resolved.tools.clone(),
            all_tools: resolved.tools.is_none(),
            context: request.context,
        };
        executions.push(execution.clone());
        let catalog = catalog_from_config(&config, executions.clone());

        Ok(ManagementSubagentExecuteResponse { execution, catalog })
    }
}

pub trait ManagementSubagentExecutionBridge: Send + Sync + std::fmt::Debug {
    fn execute(
        &self,
        subagent: &ResolvedSubagent,
        request: &ManagementSubagentExecuteRequest,
    ) -> Result<ManagementSubagentExecutionResult, String>;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSubagentExecutionResult {
    pub status: String,
    pub output: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSubagentConfig {
    #[serde(default, alias = "enable")]
    pub main_enable: bool,
    #[serde(default)]
    pub remove_main_duplicate_tools: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub router_system_prompt: String,
    #[serde(default)]
    pub agents: Vec<SubagentConfig>,
}

impl ManagementSubagentConfig {
    pub fn from_source(source: SubagentConfigSource) -> Self {
        Self {
            agents: source.agents,
            ..Self::default()
        }
    }

    fn source(&self) -> SubagentConfigSource {
        SubagentConfigSource::new(self.agents.clone())
    }
}

impl Default for ManagementSubagentConfig {
    fn default() -> Self {
        Self {
            main_enable: false,
            remove_main_duplicate_tools: false,
            router_system_prompt: default_router_system_prompt(),
            agents: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSubagentApplyRequest {
    #[serde(default)]
    pub agents: Vec<SubagentConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementSubagentApplyResponse {
    pub ok: bool,
    pub catalog: ManagementSubagentCatalogResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementSubagentCatalogResponse {
    pub main_enable: bool,
    pub remove_main_duplicate_tools: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub router_system_prompt: String,
    pub agents: Vec<ManagementSubagentDescriptor>,
    pub handoffs: Vec<ManagementSubagentHandoffDescriptor>,
    pub executions: Vec<ManagementSubagentExecutionRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSubagentDescriptor {
    pub name: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub public_description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    pub all_tools: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementSubagentHandoffDescriptor {
    pub tool_name: String,
    pub agent_name: String,
    pub description: String,
    pub parameters: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    pub all_tools: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementSubagentExecuteRequest {
    pub agent_name: String,
    pub input: String,
    #[serde(default)]
    pub context: Value,
    #[serde(default)]
    pub background: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementSubagentExecuteResponse {
    pub execution: ManagementSubagentExecutionRecord,
    pub catalog: ManagementSubagentCatalogResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementSubagentExecutionRecord {
    pub run_id: String,
    pub agent_name: String,
    pub handoff_tool: String,
    pub input: String,
    pub output: String,
    pub status: String,
    pub background: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    pub all_tools: bool,
    #[serde(default)]
    pub context: Value,
}

impl From<&SubagentConfig> for ManagementSubagentDescriptor {
    fn from(config: &SubagentConfig) -> Self {
        Self {
            name: config.name.clone(),
            enabled: config.enabled,
            persona_id: config.persona_id.clone(),
            system_prompt: config.system_prompt.clone(),
            public_description: config.public_description.clone(),
            provider_id: config.provider_id.clone(),
            tools: config.tools.clone(),
            all_tools: config.tools.is_none(),
        }
    }
}

impl From<HandoffToolSpec> for ManagementSubagentHandoffDescriptor {
    fn from(spec: HandoffToolSpec) -> Self {
        Self {
            tool_name: spec.name,
            agent_name: spec.agent_name,
            description: spec.description,
            parameters: spec.parameters,
            provider_id: spec.provider_id,
            persona_id: spec.persona_id,
            all_tools: spec.tools.is_none(),
            tools: spec.tools,
        }
    }
}

#[derive(Debug)]
enum ManagementSubagentError {
    StateLock(String),
    Persistence(String),
    InvalidConfig(String),
    NotFound(String),
    ExecutionUnavailable,
    Execution(String),
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementSubagentCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let subagents = state.subagents().ok_or_else(subagents_unavailable)?;
    subagents
        .catalog_response()
        .map(Json)
        .map_err(map_subagent_error)
}

pub async fn apply(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSubagentApplyRequest>,
) -> Result<Json<ManagementSubagentApplyResponse>, (StatusCode, Json<ErrorResponse>)> {
    let subagents = state.subagents().ok_or_else(subagents_unavailable)?;
    let catalog = subagents
        .replace_agents(request.agents)
        .map_err(map_subagent_error)?;
    Ok(Json(ManagementSubagentApplyResponse { ok: true, catalog }))
}

pub async fn execute(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSubagentExecuteRequest>,
) -> Result<Json<ManagementSubagentExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let subagents = state.subagents().ok_or_else(subagents_unavailable)?;
    subagents
        .execute(request)
        .map(Json)
        .map_err(map_subagent_error)
}

pub async fn source_config(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let subagents = state.subagents().ok_or_else(subagents_unavailable)?;
    let config = subagents.source_config().map_err(map_subagent_error)?;
    Ok(source_ok(json!(config), "ok"))
}

pub async fn source_update_config(
    State(state): State<ManagementApiState>,
    Json(config): Json<ManagementSubagentConfig>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let subagents = state.subagents().ok_or_else(subagents_unavailable)?;
    subagents
        .replace_config(config)
        .map_err(map_subagent_error)?;
    Ok(source_ok(
        json!(subagents.source_config().map_err(map_subagent_error)?),
        "保存成功",
    ))
}

pub async fn source_available_tools(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let tools = state
        .tools()
        .ok_or_else(tool_state_unavailable)?
        .source_descriptors()
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error }),
            )
        })?
        .into_iter()
        .filter(|tool| !tool.name.starts_with("transfer_to_"))
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description.unwrap_or_default(),
                "parameters": tool.parameters,
                "active": tool.active,
                "origin": tool.origin,
                "handler_module_path": tool.origin_name,
            })
        })
        .collect::<Vec<_>>();
    Ok(source_ok(json!(tools), "ok"))
}

fn catalog_from_config(
    config: &ManagementSubagentConfig,
    executions: Vec<ManagementSubagentExecutionRecord>,
) -> ManagementSubagentCatalogResponse {
    let source = config.source();
    let agents = source
        .agents
        .iter()
        .map(ManagementSubagentDescriptor::from)
        .collect();
    let handoffs = source
        .enabled_agents()
        .into_iter()
        .map(|config| ResolvedSubagent::from_config(config, None))
        .map(HandoffToolSpec::from_subagent)
        .map(ManagementSubagentHandoffDescriptor::from)
        .collect();

    ManagementSubagentCatalogResponse {
        main_enable: config.main_enable,
        remove_main_duplicate_tools: config.remove_main_duplicate_tools,
        router_system_prompt: config.router_system_prompt.clone(),
        agents,
        handoffs,
        executions,
    }
}

fn normalize_config(
    mut config: ManagementSubagentConfig,
) -> Result<ManagementSubagentConfig, ManagementSubagentError> {
    config.router_system_prompt = config.router_system_prompt.trim().to_string();
    if config.router_system_prompt.is_empty() {
        config.router_system_prompt = default_router_system_prompt();
    }
    config.agents = normalize_agents(config.agents)?;
    Ok(config)
}

fn normalize_agents(
    agents: Vec<SubagentConfig>,
) -> Result<Vec<SubagentConfig>, ManagementSubagentError> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(agents.len());
    for mut agent in agents {
        agent.name = agent.name.trim().to_string();
        if agent.name.is_empty() {
            return Err(ManagementSubagentError::InvalidConfig(
                "subagent name is required".to_string(),
            ));
        }
        if !valid_subagent_name(&agent.name) {
            return Err(ManagementSubagentError::InvalidConfig(format!(
                "invalid subagent name {}",
                agent.name
            )));
        }
        if !seen.insert(agent.name.clone()) {
            return Err(ManagementSubagentError::InvalidConfig(format!(
                "duplicate subagent name {}",
                agent.name
            )));
        }
        agent.persona_id = trim_option(agent.persona_id);
        agent.provider_id = trim_option(agent.provider_id);
        agent.system_prompt = agent.system_prompt.trim().to_string();
        agent.public_description = agent.public_description.trim().to_string();
        agent.tools = agent.tools.map(normalize_tools);
        normalized.push(agent);
    }
    normalized.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(normalized)
}

fn trim_option(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn normalize_tools(tools: Vec<String>) -> Vec<String> {
    let mut tools = tools
        .into_iter()
        .map(|tool| tool.trim().to_string())
        .filter(|tool| !tool.is_empty())
        .collect::<Vec<_>>();
    tools.sort();
    tools.dedup();
    tools
}

fn valid_subagent_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
        && name.len() <= 64
}

fn default_router_system_prompt() -> String {
    "You are a task router. Your job is to chat naturally, recognize user intent, and delegate work to the most suitable subagent using transfer_to_* tools. Do not try to use domain tools yourself. If no subagent fits, respond directly.".to_string()
}

fn source_ok(data: Value, message: impl Into<String>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": message.into(),
        "data": data,
    }))
}

fn subagents_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "subagent management state is not configured".to_string(),
        }),
    )
}

fn tool_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "tool management state is not configured".to_string(),
        }),
    )
}

fn map_subagent_error(error: ManagementSubagentError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        ManagementSubagentError::StateLock(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("subagent management state lock: {message}"),
        ),
        ManagementSubagentError::Persistence(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("subagent persistence: {message}"),
        ),
        ManagementSubagentError::InvalidConfig(message) => (StatusCode::BAD_REQUEST, message),
        ManagementSubagentError::NotFound(agent_name) => (
            StatusCode::NOT_FOUND,
            format!("subagent {agent_name} is not enabled"),
        ),
        ManagementSubagentError::ExecutionUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "subagent execution bridge is not configured".to_string(),
        ),
        ManagementSubagentError::Execution(message) => (StatusCode::BAD_REQUEST, message),
    };

    (status, Json(ErrorResponse { error: message }))
}
