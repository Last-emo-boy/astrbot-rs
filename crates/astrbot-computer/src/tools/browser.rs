use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    BooterSession, ComputerRuntimePort, ComputerToolExecution, ComputerToolInvocation,
    EXECUTE_BROWSER_BATCH_TOOL, EXECUTE_BROWSER_TOOL,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserCdpEndpoint {
    pub websocket_url: String,
}

impl BrowserCdpEndpoint {
    pub fn new(websocket_url: impl Into<String>) -> Self {
        Self {
            websocket_url: websocket_url.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserToolLimits {
    pub timeout_ms: u64,
    pub memory_limit_mb: u64,
    pub max_sessions: usize,
}

impl Default for BrowserToolLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            memory_limit_mb: 512,
            max_sessions: 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSession {
    pub session_id: String,
    pub endpoint: BrowserCdpEndpoint,
    pub generation: u64,
    pub crashed: bool,
}

impl BrowserSession {
    pub fn new(session_id: impl Into<String>, endpoint: BrowserCdpEndpoint) -> Self {
        Self {
            session_id: session_id.into(),
            endpoint,
            generation: 1,
            crashed: false,
        }
    }

    pub fn mark_crashed(mut self) -> Self {
        self.crashed = true;
        self
    }

    fn next_generation(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            endpoint: self.endpoint.clone(),
            generation: self.generation + 1,
            crashed: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { selector: String },
    TypeText { selector: String, text: String },
    Screenshot { full_page: bool },
    ExtractText { selector: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserCommand {
    pub action: BrowserAction,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl BrowserCommand {
    pub fn navigate(url: impl Into<String>) -> Self {
        Self {
            action: BrowserAction::Navigate { url: url.into() },
            timeout_ms: None,
        }
    }

    pub fn click(selector: impl Into<String>) -> Self {
        Self {
            action: BrowserAction::Click {
                selector: selector.into(),
            },
            timeout_ms: None,
        }
    }

    pub fn type_text(selector: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            action: BrowserAction::TypeText {
                selector: selector.into(),
                text: text.into(),
            },
            timeout_ms: None,
        }
    }

    pub fn screenshot() -> Self {
        Self {
            action: BrowserAction::Screenshot { full_page: true },
            timeout_ms: None,
        }
    }

    pub fn extract_text(selector: impl Into<String>) -> Self {
        Self {
            action: BrowserAction::ExtractText {
                selector: Some(selector.into()),
            },
            timeout_ms: None,
        }
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn parse(command: &str) -> Result<Self> {
        let command = command.trim();
        if command.is_empty() {
            return Err(AstrbotError::Pipeline(
                "browser command cannot be empty".to_string(),
            ));
        }
        if command.starts_with('{') {
            return serde_json::from_str(command).map_err(|err| {
                AstrbotError::Pipeline(format!("parse browser command JSON: {err}"))
            });
        }

        let (verb, rest) = split_once_whitespace(command);
        match verb {
            "navigate" | "goto" | "open" => {
                non_empty(rest, "browser navigate url").map(Self::navigate)
            }
            "click" => non_empty(rest, "browser click selector").map(Self::click),
            "type" | "type_text" => {
                let (selector, text) = split_once_whitespace(rest);
                let selector = non_empty(selector, "browser type selector")?;
                let text = non_empty(text, "browser type text")?;
                Ok(Self::type_text(selector, text))
            }
            "screenshot" => Ok(Self::screenshot()),
            "extract_text" | "text" => Ok(Self {
                action: BrowserAction::ExtractText {
                    selector: non_empty_option(rest),
                },
                timeout_ms: None,
            }),
            other => Err(AstrbotError::Pipeline(format!(
                "unknown browser command verb: {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrowserCommandResult {
    pub ok: bool,
    pub value: Value,
}

impl BrowserCommandResult {
    pub fn ok(value: Value) -> Self {
        Self { ok: true, value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserToolError {
    Crashed(String),
    Rejected(String),
}

impl BrowserToolError {
    fn into_core(self) -> AstrbotError {
        match self {
            Self::Crashed(message) | Self::Rejected(message) => AstrbotError::Pipeline(message),
        }
    }
}

#[async_trait]
pub trait BrowserToolPort: Send + Sync {
    async fn execute(
        &self,
        session: &BrowserSession,
        command: &BrowserCommand,
        limits: &BrowserToolLimits,
    ) -> std::result::Result<BrowserCommandResult, BrowserToolError>;
}

pub struct BrowserSessionPool {
    endpoint: BrowserCdpEndpoint,
    limits: BrowserToolLimits,
    port: Arc<dyn BrowserToolPort>,
    sessions: RwLock<BTreeMap<String, BrowserSession>>,
}

impl BrowserSessionPool {
    pub fn new(
        endpoint: BrowserCdpEndpoint,
        limits: BrowserToolLimits,
        port: Arc<dyn BrowserToolPort>,
    ) -> Self {
        Self {
            endpoint,
            limits,
            port,
            sessions: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn session(&self, session_id: &str) -> Result<BrowserSession> {
        let mut sessions = self.sessions.write().map_err(lock_error)?;
        Ok(self.session_locked(&mut sessions, session_id))
    }

    pub async fn execute(
        &self,
        session_id: &str,
        command: BrowserCommand,
    ) -> Result<BrowserCommandResult> {
        let session = self.session(session_id)?;
        match self.port.execute(&session, &command, &self.limits).await {
            Ok(result) => Ok(result),
            Err(BrowserToolError::Crashed(message)) => {
                self.replace_crashed_session(session_id)?;
                Err(AstrbotError::Pipeline(message))
            }
            Err(err) => Err(err.into_core()),
        }
    }

    pub fn mark_crashed(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().map_err(lock_error)?;
        if let Some(session) = sessions.get(session_id).cloned() {
            sessions.insert(session_id.to_string(), session.mark_crashed());
        }
        Ok(())
    }

    fn replace_crashed_session(&self, session_id: &str) -> Result<BrowserSession> {
        let mut sessions = self.sessions.write().map_err(lock_error)?;
        let next = sessions
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| BrowserSession::new(session_id, self.endpoint.clone()))
            .next_generation();
        sessions.insert(session_id.to_string(), next.clone());
        Ok(next)
    }

    fn session_locked(
        &self,
        sessions: &mut BTreeMap<String, BrowserSession>,
        session_id: &str,
    ) -> BrowserSession {
        if let Some(existing) = sessions.get(session_id)
            && !existing.crashed
        {
            return existing.clone();
        }
        let next = sessions
            .get(session_id)
            .map(BrowserSession::next_generation)
            .unwrap_or_else(|| BrowserSession::new(session_id, self.endpoint.clone()));
        sessions.insert(session_id.to_string(), next.clone());

        while sessions.len() > self.limits.max_sessions.max(1) {
            let Some(first_key) = sessions.keys().next().cloned() else {
                break;
            };
            if first_key == session_id && sessions.len() == 1 {
                break;
            }
            sessions.remove(&first_key);
        }

        next
    }
}

pub struct BrowserComputerRuntimePort {
    pool: Arc<BrowserSessionPool>,
}

impl BrowserComputerRuntimePort {
    pub fn new(pool: Arc<BrowserSessionPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ComputerRuntimePort for BrowserComputerRuntimePort {
    async fn execute_local(
        &self,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        Ok(ComputerToolExecution::rejected(format!(
            "{} requires a sandbox browser session",
            invocation.tool_name
        )))
    }

    async fn execute_sandbox(
        &self,
        session: BooterSession,
        invocation: ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        match invocation.tool_name.as_str() {
            EXECUTE_BROWSER_TOOL => self.execute_one(&session, &invocation).await,
            EXECUTE_BROWSER_BATCH_TOOL => self.execute_batch(&session, &invocation).await,
            other => Ok(ComputerToolExecution::rejected(format!(
                "browser runtime cannot execute tool {other}"
            ))),
        }
    }
}

impl BrowserComputerRuntimePort {
    async fn execute_one(
        &self,
        session: &BooterSession,
        invocation: &ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        let Some(command_text) = invocation.string_arg("cmd") else {
            return Ok(ComputerToolExecution::rejected("browser cmd is required"));
        };
        let command = BrowserCommand::parse(&command_text)?
            .with_timeout_ms(invocation_timeout_ms(invocation, 30));
        match self.pool.execute(&session.session_id, command).await {
            Ok(result) => Ok(ComputerToolExecution::completed(result.value)),
            Err(err) => Ok(ComputerToolExecution::rejected(err.to_string())),
        }
    }

    async fn execute_batch(
        &self,
        session: &BooterSession,
        invocation: &ComputerToolInvocation,
    ) -> Result<ComputerToolExecution> {
        let commands = invocation.string_list_arg("commands");
        if commands.is_empty() {
            return Ok(ComputerToolExecution::rejected(
                "browser batch commands are required",
            ));
        }

        let timeout_ms = invocation_timeout_ms(invocation, 60);
        let stop_on_error = invocation.bool_arg("stop_on_error", true);
        let mut outputs = Vec::new();
        for command_text in commands {
            let command = BrowserCommand::parse(&command_text)?.with_timeout_ms(timeout_ms);
            match self.pool.execute(&session.session_id, command).await {
                Ok(result) => outputs.push(result.value),
                Err(err) if stop_on_error => {
                    return Ok(ComputerToolExecution::rejected(err.to_string()));
                }
                Err(err) => outputs.push(json!({ "ok": false, "error": err.to_string() })),
            }
        }
        Ok(ComputerToolExecution::completed(
            json!({ "results": outputs }),
        ))
    }
}

#[derive(Default)]
pub struct FakeBrowserToolPort {
    calls: RwLock<Vec<(String, BrowserCommand)>>,
    failures: RwLock<VecDeque<BrowserToolError>>,
}

impl FakeBrowserToolPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_failure(&self, failure: BrowserToolError) -> Result<()> {
        self.failures
            .write()
            .map_err(lock_error)?
            .push_back(failure);
        Ok(())
    }

    pub fn calls(&self) -> Vec<(String, BrowserCommand)> {
        self.calls
            .read()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl BrowserToolPort for FakeBrowserToolPort {
    async fn execute(
        &self,
        session: &BrowserSession,
        command: &BrowserCommand,
        limits: &BrowserToolLimits,
    ) -> std::result::Result<BrowserCommandResult, BrowserToolError> {
        if let Some(failure) = self
            .failures
            .write()
            .map_err(|err| BrowserToolError::Rejected(err.to_string()))?
            .pop_front()
        {
            return Err(failure);
        }

        self.calls
            .write()
            .map_err(|err| BrowserToolError::Rejected(err.to_string()))?
            .push((session.session_id.clone(), command.clone()));

        let value = match &command.action {
            BrowserAction::Navigate { url } => json!({
                "action": "navigate",
                "url": url,
                "timeout_ms": command.timeout_ms.unwrap_or(limits.timeout_ms)
            }),
            BrowserAction::Click { selector } => json!({
                "action": "click",
                "selector": selector
            }),
            BrowserAction::TypeText { selector, text } => json!({
                "action": "type_text",
                "selector": selector,
                "text": text
            }),
            BrowserAction::Screenshot { full_page } => json!({
                "action": "screenshot",
                "mime": "image/png",
                "bytes": [137, 80, 78, 71],
                "full_page": full_page
            }),
            BrowserAction::ExtractText { selector } => json!({
                "action": "extract_text",
                "selector": selector,
                "text": "Example Domain"
            }),
        };
        Ok(BrowserCommandResult::ok(value))
    }
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("browser tool lock: {err}"))
}

fn invocation_timeout_ms(invocation: &ComputerToolInvocation, default_seconds: i64) -> u64 {
    let seconds = invocation.integer_arg("timeout", default_seconds).max(1) as u64;
    seconds.saturating_mul(1_000)
}

fn split_once_whitespace(value: &str) -> (&str, &str) {
    let value = value.trim();
    match value.find(char::is_whitespace) {
        Some(index) => (&value[..index], value[index..].trim()),
        None => (value, ""),
    }
}

fn non_empty(value: &str, label: &str) -> Result<String> {
    non_empty_option(value)
        .ok_or_else(|| AstrbotError::Pipeline(format!("{label} cannot be empty")))
}

fn non_empty_option(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::{
        BrowserCdpEndpoint, BrowserCommand, BrowserComputerRuntimePort, BrowserSessionPool,
        BrowserToolError, BrowserToolLimits, FakeBrowserToolPort,
    };
    use crate::{
        BooterKind, BooterSession, ComputerRuntimeConfig, ComputerRuntimePort,
        ComputerToolExecutionStatus, ComputerToolInvocation, EXECUTE_BROWSER_BATCH_TOOL,
        EXECUTE_BROWSER_TOOL,
    };

    #[tokio::test]
    async fn fake_browser_port_covers_screenshot_and_text_extraction() {
        let port = Arc::new(FakeBrowserToolPort::new());
        let pool = BrowserSessionPool::new(
            BrowserCdpEndpoint::new("ws://localhost:9222/devtools/browser/test"),
            BrowserToolLimits::default(),
            port.clone(),
        );

        let shot = pool
            .execute("s1", BrowserCommand::screenshot())
            .await
            .expect("screenshot should work");
        let text = pool
            .execute("s1", BrowserCommand::extract_text("main"))
            .await
            .expect("extract should work");

        assert_eq!(shot.value["mime"], "image/png");
        assert_eq!(text.value["text"], "Example Domain");
        assert_eq!(port.calls().len(), 2);
    }

    #[tokio::test]
    async fn crashed_browser_session_is_recreated_before_next_action() {
        let port = Arc::new(FakeBrowserToolPort::new());
        port.push_failure(BrowserToolError::Crashed("browser crashed".to_string()))
            .unwrap();
        let pool = BrowserSessionPool::new(
            BrowserCdpEndpoint::new("ws://localhost:9222/devtools/browser/test"),
            BrowserToolLimits::default(),
            port,
        );

        let first_generation = pool.session("s1").unwrap().generation;
        let err = pool
            .execute("s1", BrowserCommand::navigate("https://example.com"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("browser crashed"));

        let recovered = pool
            .execute("s1", BrowserCommand::navigate("https://example.com"))
            .await
            .expect("next action should use recreated session");
        assert_eq!(recovered.value["url"], "https://example.com");
        assert!(pool.session("s1").unwrap().generation > first_generation);
    }

    #[tokio::test]
    async fn browser_runtime_port_executes_single_and_batch_tool_invocations() {
        let browser_port = Arc::new(FakeBrowserToolPort::new());
        let pool = Arc::new(BrowserSessionPool::new(
            BrowserCdpEndpoint::new("ws://localhost:9222/devtools/browser/test"),
            BrowserToolLimits::default(),
            browser_port.clone(),
        ));
        let runtime = BrowserComputerRuntimePort::new(pool);
        let session = BooterSession::new(
            "conversation-1",
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo),
        );

        let single = runtime
            .execute_sandbox(
                session.clone(),
                invocation(
                    EXECUTE_BROWSER_TOOL,
                    serde_json::json!({
                        "cmd": "navigate https://example.com",
                        "timeout": 2
                    }),
                ),
            )
            .await
            .expect("single browser command should execute");
        assert_eq!(single.status, ComputerToolExecutionStatus::Completed);
        assert_eq!(single.output["url"], "https://example.com");
        assert_eq!(single.output["timeout_ms"], 2_000);

        let batch = runtime
            .execute_sandbox(
                session,
                invocation(
                    EXECUTE_BROWSER_BATCH_TOOL,
                    serde_json::json!({
                        "commands": ["click main", "extract_text main"],
                        "timeout": 3
                    }),
                ),
            )
            .await
            .expect("browser batch should execute");
        assert_eq!(batch.status, ComputerToolExecutionStatus::Completed);
        assert_eq!(batch.output["results"].as_array().unwrap().len(), 2);
        assert_eq!(browser_port.calls().len(), 3);
    }

    fn invocation(tool_name: &str, value: serde_json::Value) -> ComputerToolInvocation {
        let arguments = value
            .as_object()
            .expect("browser invocation arguments should be object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        ComputerToolInvocation::new(tool_name, "conversation-1", arguments)
    }
}
