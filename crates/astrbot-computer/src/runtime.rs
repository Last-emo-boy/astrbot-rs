use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use astrbot_tool::{ToolCatalog, ToolSourceMetadata};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{BooterKind, BooterSession, ComputerComponent, ComputerRuntimeConfig};

pub const COMPUTER_USE_PROVIDER_ID: &str = "computer_use";
pub const EXECUTE_SHELL_TOOL: &str = "astrbot_execute_shell";
pub const EXECUTE_IPYTHON_TOOL: &str = "astrbot_execute_ipython";
pub const UPLOAD_FILE_TOOL: &str = "astrbot_upload_file";
pub const DOWNLOAD_FILE_TOOL: &str = "astrbot_download_file";
pub const EXECUTE_BROWSER_TOOL: &str = "astrbot_execute_browser";
pub const EXECUTE_BROWSER_BATCH_TOOL: &str = "astrbot_execute_browser_batch";
pub const RUN_BROWSER_SKILL_TOOL: &str = "astrbot_run_browser_skill";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputerUseRuntimeMode {
    None,
    Local,
    Sandbox(ComputerRuntimeConfig),
}

impl ComputerUseRuntimeMode {
    pub fn local() -> Self {
        Self::Local
    }

    pub fn sandbox(kind: BooterKind) -> Self {
        Self::Sandbox(ComputerRuntimeConfig::sandbox(kind))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputerUseSession {
    pub session_id: String,
    pub mode: ComputerUseRuntimeMode,
    pub admin: bool,
}

impl ComputerUseSession {
    pub fn new(session_id: impl Into<String>, mode: ComputerUseRuntimeMode) -> Self {
        Self {
            session_id: session_id.into(),
            mode,
            admin: false,
        }
    }

    pub fn local(session_id: impl Into<String>) -> Self {
        Self::new(session_id, ComputerUseRuntimeMode::Local)
    }

    pub fn none(session_id: impl Into<String>) -> Self {
        Self::new(session_id, ComputerUseRuntimeMode::None)
    }

    pub fn sandbox(session_id: impl Into<String>, config: ComputerRuntimeConfig) -> Self {
        Self::new(session_id, ComputerUseRuntimeMode::Sandbox(config))
    }

    pub fn with_admin(mut self, admin: bool) -> Self {
        self.admin = admin;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputerToolInvocation {
    pub tool_name: String,
    pub session_id: String,
    pub arguments: BTreeMap<String, Value>,
}

impl ComputerToolInvocation {
    pub fn new(
        tool_name: impl Into<String>,
        session_id: impl Into<String>,
        arguments: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            session_id: session_id.into(),
            arguments,
        }
    }

    pub fn string_arg(&self, key: &str) -> Option<String> {
        self.arguments
            .get(key)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    }

    pub fn bool_arg(&self, key: &str, default: bool) -> bool {
        self.arguments
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    pub fn integer_arg(&self, key: &str, default: i64) -> i64 {
        self.arguments
            .get(key)
            .and_then(Value::as_i64)
            .unwrap_or(default)
    }

    pub fn string_list_arg(&self, key: &str) -> Vec<String> {
        self.arguments
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerToolExecution {
    pub status: ComputerToolExecutionStatus,
    pub output: Value,
}

impl ComputerToolExecution {
    pub fn completed(output: Value) -> Self {
        Self {
            status: ComputerToolExecutionStatus::Completed,
            output,
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            status: ComputerToolExecutionStatus::Rejected,
            output: json!({ "error": message.into() }),
        }
    }

    pub fn output_text(&self) -> String {
        match &self.output {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerToolExecutionStatus {
    Completed,
    Rejected,
}

#[async_trait]
pub trait ComputerRuntimePort: Send + Sync {
    async fn execute_local(
        &self,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution>;

    async fn execute_sandbox(
        &self,
        session: BooterSession,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution>;
}

pub struct ComputerUseRuntime {
    port: Arc<dyn ComputerRuntimePort>,
}

impl ComputerUseRuntime {
    pub fn new(port: Arc<dyn ComputerRuntimePort>) -> Self {
        Self { port }
    }

    pub fn catalog_for_session(
        &self,
        base_catalog: &ToolCatalog,
        session: &ComputerUseSession,
        booter_session: Option<&BooterSession>,
    ) -> ToolCatalog {
        computer_catalog_for_session(base_catalog, session, booter_session)
    }

    pub async fn execute(
        &self,
        session: &ComputerUseSession,
        booter_session: Option<BooterSession>,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        if !session.admin {
            return Ok(ComputerToolExecution::rejected(format!(
                "{} requires administrator permission",
                invocation.tool_name
            )));
        }

        match &session.mode {
            ComputerUseRuntimeMode::None => Ok(ComputerToolExecution::rejected(
                "computer-use runtime is disabled",
            )),
            ComputerUseRuntimeMode::Local => {
                ensure_local_tool(&invocation.tool_name)?;
                self.port.execute_local(invocation).await
            }
            ComputerUseRuntimeMode::Sandbox(config) => {
                ensure_sandbox_tool(&invocation.tool_name)?;
                let session = booter_session.unwrap_or_else(|| {
                    BooterSession::new(session.session_id.clone(), config.clone())
                });
                let component = component_for_tool(&invocation.tool_name)?;
                if !session.supports(component) {
                    return Ok(ComputerToolExecution::rejected(format!(
                        "computer-use runtime does not support {:?} capability",
                        component
                    )));
                }
                self.port.execute_sandbox(session, invocation).await
            }
        }
    }
}

pub fn computer_catalog_for_session(
    base_catalog: &ToolCatalog,
    session: &ComputerUseSession,
    booter_session: Option<&BooterSession>,
) -> ToolCatalog {
    let active_names = active_computer_tool_names(session, booter_session);
    let active_names = active_names.iter().collect::<BTreeSet<_>>();
    let mut catalog = ToolCatalog::new();

    for tool in base_catalog.tools() {
        let mut tool = tool.clone();
        if is_computer_tool(&tool.name) {
            tool.active = active_names.contains(&tool.name);
            if tool.source.provider_id.as_deref() != Some(COMPUTER_USE_PROVIDER_ID) {
                tool.source =
                    ToolSourceMetadata::internal_provider(COMPUTER_USE_PROVIDER_ID, "AstrBot");
            }
        }
        catalog.add_tool(tool);
    }

    catalog
}

pub fn active_computer_tool_names(
    session: &ComputerUseSession,
    booter_session: Option<&BooterSession>,
) -> Vec<String> {
    match &session.mode {
        ComputerUseRuntimeMode::None => Vec::new(),
        ComputerUseRuntimeMode::Local => vec![
            EXECUTE_SHELL_TOOL.to_string(),
            EXECUTE_IPYTHON_TOOL.to_string(),
        ],
        ComputerUseRuntimeMode::Sandbox(config) => {
            let components = booter_session
                .map(|session| session.components.as_slice())
                .unwrap_or_else(|| config.components.as_slice());
            computer_tool_names_for_components(components)
        }
    }
}

pub fn computer_tool_names_for_components(components: &[ComputerComponent]) -> Vec<String> {
    let mut names = Vec::new();
    if components.contains(&ComputerComponent::Shell) {
        names.push(EXECUTE_SHELL_TOOL.to_string());
    }
    if components.contains(&ComputerComponent::Python) {
        names.push(EXECUTE_IPYTHON_TOOL.to_string());
    }
    if components.contains(&ComputerComponent::FileSystem) {
        names.push(UPLOAD_FILE_TOOL.to_string());
        names.push(DOWNLOAD_FILE_TOOL.to_string());
    }
    if components.contains(&ComputerComponent::Browser) {
        names.push(EXECUTE_BROWSER_TOOL.to_string());
        names.push(EXECUTE_BROWSER_BATCH_TOOL.to_string());
        names.push(RUN_BROWSER_SKILL_TOOL.to_string());
    }
    names.sort();
    names
}

pub fn is_computer_tool(name: &str) -> bool {
    matches!(
        name,
        EXECUTE_SHELL_TOOL
            | EXECUTE_IPYTHON_TOOL
            | UPLOAD_FILE_TOOL
            | DOWNLOAD_FILE_TOOL
            | EXECUTE_BROWSER_TOOL
            | EXECUTE_BROWSER_BATCH_TOOL
            | RUN_BROWSER_SKILL_TOOL
    )
}

fn ensure_local_tool(name: &str) -> Result<()> {
    match name {
        EXECUTE_SHELL_TOOL | EXECUTE_IPYTHON_TOOL => Ok(()),
        other => Err(AstrbotError::Pipeline(format!(
            "computer-use local runtime cannot execute tool {other}"
        ))),
    }
}

fn ensure_sandbox_tool(name: &str) -> Result<()> {
    if is_computer_tool(name) {
        Ok(())
    } else {
        Err(AstrbotError::Pipeline(format!(
            "computer-use runtime cannot execute tool {name}"
        )))
    }
}

fn component_for_tool(name: &str) -> Result<ComputerComponent> {
    match name {
        EXECUTE_SHELL_TOOL => Ok(ComputerComponent::Shell),
        EXECUTE_IPYTHON_TOOL => Ok(ComputerComponent::Python),
        UPLOAD_FILE_TOOL | DOWNLOAD_FILE_TOOL => Ok(ComputerComponent::FileSystem),
        EXECUTE_BROWSER_TOOL | EXECUTE_BROWSER_BATCH_TOOL | RUN_BROWSER_SKILL_TOOL => {
            Ok(ComputerComponent::Browser)
        }
        other => Err(AstrbotError::Pipeline(format!(
            "unknown computer-use tool {other}"
        ))),
    }
}

#[derive(Default)]
pub struct RecordingComputerRuntimePort {
    calls: RwLock<Vec<RecordedComputerRuntimeCall>>,
}

impl RecordingComputerRuntimePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calls(&self) -> Vec<RecordedComputerRuntimeCall> {
        self.calls
            .read()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordedComputerRuntimeCall {
    pub runtime: String,
    pub tool_name: String,
    pub session_id: String,
    pub arguments: BTreeMap<String, Value>,
}

#[async_trait]
impl ComputerRuntimePort for RecordingComputerRuntimePort {
    async fn execute_local(
        &self,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        self.calls
            .write()
            .map_err(lock_error)?
            .push(RecordedComputerRuntimeCall {
                runtime: "local".to_string(),
                tool_name: invocation.tool_name.clone(),
                session_id: invocation.session_id.clone(),
                arguments: invocation.arguments.clone(),
            });
        Ok(ComputerToolExecution::completed(json!({
            "runtime": "local",
            "tool": invocation.tool_name,
            "session_id": invocation.session_id,
            "arguments": object_from_btreemap(invocation.arguments),
        })))
    }

    async fn execute_sandbox(
        &self,
        session: BooterSession,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        self.calls
            .write()
            .map_err(lock_error)?
            .push(RecordedComputerRuntimeCall {
                runtime: format!("{:?}", session.config.kind).to_ascii_lowercase(),
                tool_name: invocation.tool_name.clone(),
                session_id: invocation.session_id.clone(),
                arguments: invocation.arguments.clone(),
            });
        Ok(ComputerToolExecution::completed(json!({
            "runtime": "sandbox",
            "kind": format!("{:?}", session.config.kind).to_ascii_lowercase(),
            "tool": invocation.tool_name,
            "session_id": invocation.session_id,
            "arguments": object_from_btreemap(invocation.arguments),
        })))
    }
}

fn object_from_btreemap(map: BTreeMap<String, Value>) -> Value {
    Value::Object(map.into_iter().collect::<Map<_, _>>())
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("computer runtime lock: {err}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use astrbot_tool::builtin_internal_tool_catalog;
    use serde_json::json;

    use super::{
        ComputerToolExecutionStatus, ComputerToolInvocation, ComputerUseRuntime,
        ComputerUseRuntimeMode, ComputerUseSession, DOWNLOAD_FILE_TOOL, EXECUTE_BROWSER_TOOL,
        EXECUTE_SHELL_TOOL, RUN_BROWSER_SKILL_TOOL, RecordingComputerRuntimePort,
        active_computer_tool_names, computer_catalog_for_session,
    };
    use crate::{BooterKind, BooterSession, ComputerComponent, ComputerRuntimeConfig};

    #[test]
    fn runtime_catalog_keeps_registration_inactive_until_runtime_selects_tools() {
        let base = builtin_internal_tool_catalog().into_tool_catalog();
        let none = computer_catalog_for_session(&base, &ComputerUseSession::none("s1"), None);

        assert!(
            !none
                .tool(EXECUTE_SHELL_TOOL)
                .expect("shell registered")
                .active
        );

        let local = computer_catalog_for_session(&base, &ComputerUseSession::local("s1"), None);
        assert!(local.tool(EXECUTE_SHELL_TOOL).expect("shell").active);
        assert!(!local.tool(DOWNLOAD_FILE_TOOL).expect("download").active);
    }

    #[test]
    fn sandbox_catalog_uses_booted_capabilities_when_available() {
        let base = builtin_internal_tool_catalog().into_tool_catalog();
        let session = ComputerUseSession::sandbox(
            "s1",
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo),
        );
        let booted = BooterSession::new(
            "s1",
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo)
                .with_components([ComputerComponent::Shell, ComputerComponent::Browser]),
        );
        let catalog = computer_catalog_for_session(&base, &session, Some(&booted));

        assert!(catalog.tool(EXECUTE_BROWSER_TOOL).expect("browser").active);
        assert!(catalog.tool(RUN_BROWSER_SKILL_TOOL).expect("skill").active);
        assert!(!catalog.tool(DOWNLOAD_FILE_TOOL).expect("download").active);
        assert_eq!(
            active_computer_tool_names(&session, Some(&booted)),
            vec![
                EXECUTE_BROWSER_TOOL.to_string(),
                "astrbot_execute_browser_batch".to_string(),
                EXECUTE_SHELL_TOOL.to_string(),
                RUN_BROWSER_SKILL_TOOL.to_string()
            ]
        );
    }

    #[tokio::test]
    async fn runtime_rejects_permission_runtime_and_capability_failures() {
        let port = Arc::new(RecordingComputerRuntimePort::new());
        let runtime = ComputerUseRuntime::new(port);
        let denied = runtime
            .execute(
                &ComputerUseSession::local("s1"),
                None,
                invocation(EXECUTE_SHELL_TOOL, "s1", json!({ "command": "pwd" })),
            )
            .await
            .expect("denied should be response");
        assert_eq!(denied.status, ComputerToolExecutionStatus::Rejected);
        assert!(denied.output_text().contains("administrator"));

        let disabled = runtime
            .execute(
                &ComputerUseSession::none("s1").with_admin(true),
                None,
                invocation(EXECUTE_SHELL_TOOL, "s1", json!({ "command": "pwd" })),
            )
            .await
            .expect("disabled should be response");
        assert!(disabled.output_text().contains("disabled"));

        let sandbox = ComputerUseSession::new(
            "s1",
            ComputerUseRuntimeMode::sandbox(BooterKind::ShipyardNeo),
        )
        .with_admin(true);
        let booted = BooterSession::new(
            "s1",
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo)
                .with_components([ComputerComponent::Shell]),
        );
        let missing = runtime
            .execute(
                &sandbox,
                Some(booted),
                invocation(
                    RUN_BROWSER_SKILL_TOOL,
                    "s1",
                    json!({ "skill_key": "login" }),
                ),
            )
            .await
            .expect("missing capability should be response");
        assert!(missing.output_text().contains("Browser"));
    }

    #[tokio::test]
    async fn runtime_executes_local_shell_sandbox_file_and_browser_skill_through_port() {
        let port = Arc::new(RecordingComputerRuntimePort::new());
        let runtime = ComputerUseRuntime::new(port.clone());

        let local = runtime
            .execute(
                &ComputerUseSession::local("local-1").with_admin(true),
                None,
                invocation(
                    EXECUTE_SHELL_TOOL,
                    "local-1",
                    json!({ "command": "echo ok" }),
                ),
            )
            .await
            .expect("local shell should execute");
        assert_eq!(local.status, ComputerToolExecutionStatus::Completed);

        let sandbox = ComputerUseSession::sandbox(
            "sandbox-1",
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo),
        )
        .with_admin(true);
        let booted = BooterSession::new(
            "sandbox-1",
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo)
                .with_components([ComputerComponent::FileSystem, ComputerComponent::Browser]),
        );
        runtime
            .execute(
                &sandbox,
                Some(booted.clone()),
                invocation(
                    DOWNLOAD_FILE_TOOL,
                    "sandbox-1",
                    json!({ "remote_path": "/workspace/out.png" }),
                ),
            )
            .await
            .expect("sandbox file transfer should execute");
        runtime
            .execute(
                &sandbox,
                Some(booted),
                invocation(
                    RUN_BROWSER_SKILL_TOOL,
                    "sandbox-1",
                    json!({ "skill_key": "login", "timeout": 90 }),
                ),
            )
            .await
            .expect("browser skill should execute");

        let calls = port.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].runtime, "local");
        assert_eq!(calls[1].tool_name, DOWNLOAD_FILE_TOOL);
        assert_eq!(calls[2].tool_name, RUN_BROWSER_SKILL_TOOL);
    }

    fn invocation(
        tool: &str,
        session_id: &str,
        value: serde_json::Value,
    ) -> ComputerToolInvocation {
        let arguments = value
            .as_object()
            .expect("arguments object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        ComputerToolInvocation::new(tool, session_id, arguments)
    }
}
