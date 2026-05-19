use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::types::{McpError, McpJsonSchema, McpJsonValue, McpResult};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpElicitationRequest {
    Form {
        message: String,
        requested_schema: McpElicitationSchema,
    },
    Url {
        message: String,
        url: String,
    },
}

impl McpElicitationRequest {
    pub fn form(message: impl Into<String>, requested_schema: McpElicitationSchema) -> Self {
        Self::Form {
            message: message.into(),
            requested_schema,
        }
    }

    pub fn url(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self::Url {
            message: message.into(),
            url: url.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationSchema {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, McpElicitationField>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
}

impl McpElicitationSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_field(mut self, name: impl Into<String>, field: McpElicitationField) -> Self {
        let name = name.into().trim().to_string();
        if !name.is_empty() {
            self.properties.insert(name, field);
        }
        self
    }

    pub fn require(mut self, name: impl Into<String>) -> Self {
        let name = name.into().trim().to_string();
        if !name.is_empty() && !self.required.contains(&name) {
            self.required.push(name);
        }
        self
    }

    pub fn into_json_schema(self) -> McpJsonSchema {
        let value = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        McpJsonSchema::from_json(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationField {
    #[serde(rename = "type")]
    pub field_type: McpElicitationFieldType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<McpElicitationValue>,
}

impl McpElicitationField {
    pub fn string() -> Self {
        Self {
            field_type: McpElicitationFieldType::String,
            description: None,
            enum_values: Vec::new(),
        }
    }

    pub fn integer() -> Self {
        Self {
            field_type: McpElicitationFieldType::Integer,
            description: None,
            enum_values: Vec::new(),
        }
    }

    pub fn boolean() -> Self {
        Self {
            field_type: McpElicitationFieldType::Boolean,
            description: None,
            enum_values: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn with_enum_value(mut self, value: impl Into<McpElicitationValue>) -> Self {
        self.enum_values.push(value.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpElicitationFieldType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpElicitationValue {
    String(String),
    Integer(i64),
    Number(f64),
    Boolean(bool),
    StringArray(Vec<String>),
    Null,
}

impl From<&str> for McpElicitationValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for McpElicitationValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for McpElicitationValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for McpElicitationValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<McpElicitationValue> for McpJsonValue {
    fn from(value: McpElicitationValue) -> Self {
        match value {
            McpElicitationValue::String(value) => Self::String(value),
            McpElicitationValue::Integer(value) => Self::Integer(value),
            McpElicitationValue::Number(value) => Self::Number(value),
            McpElicitationValue::Boolean(value) => Self::Bool(value),
            McpElicitationValue::StringArray(value) => {
                Self::Array(value.into_iter().map(McpJsonValue::String).collect())
            }
            McpElicitationValue::Null => Self::Null,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationResult {
    pub action: McpElicitationAction,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub content: BTreeMap<String, McpElicitationValue>,
}

impl McpElicitationResult {
    pub fn accept(content: BTreeMap<String, McpElicitationValue>) -> Self {
        Self {
            action: McpElicitationAction::Accept,
            content,
        }
    }

    pub fn decline() -> Self {
        Self {
            action: McpElicitationAction::Decline,
            content: BTreeMap::new(),
        }
    }

    pub fn cancel() -> Self {
        Self {
            action: McpElicitationAction::Cancel,
            content: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpElicitationAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpElicitationSession {
    pub session_id: String,
    pub unified_msg_origin: String,
    pub sender_id: String,
    pub prompt: String,
    pub timeout_seconds: u64,
}

#[derive(Debug)]
pub struct McpElicitationCoordinator {
    server_name: String,
    sessions: BTreeMap<String, PendingElicitationSession>,
    next_session_id: u64,
}

impl McpElicitationCoordinator {
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            sessions: BTreeMap::new(),
            next_session_id: 0,
        }
    }

    pub fn begin(
        &mut self,
        unified_msg_origin: impl Into<String>,
        sender_id: impl Into<String>,
        request: McpElicitationRequest,
        timeout_seconds: u64,
    ) -> McpResult<McpElicitationSession> {
        let unified_msg_origin =
            normalized_non_empty(unified_msg_origin, "unified message origin")?;
        let sender_id = normalized_non_empty(sender_id, "sender id")?;
        self.clear_expired_for(&unified_msg_origin);
        if self.sessions.contains_key(&unified_msg_origin) {
            return Err(McpError::Unsupported(format!(
                "An MCP elicitation is already active for unified message origin '{unified_msg_origin}'."
            )));
        }

        self.next_session_id += 1;
        let timeout_seconds = timeout_seconds.max(1);
        let prompt = build_elicitation_prompt(&self.server_name, &request);
        let session = McpElicitationSession {
            session_id: format!("elicitation-{}", self.next_session_id),
            unified_msg_origin: unified_msg_origin.clone(),
            sender_id: sender_id.clone(),
            prompt,
            timeout_seconds,
        };
        self.sessions.insert(
            unified_msg_origin,
            PendingElicitationSession {
                session: session.clone(),
                request,
                expires_at: Instant::now() + Duration::from_secs(timeout_seconds),
            },
        );
        Ok(session)
    }

    pub fn handle_reply(
        &mut self,
        unified_msg_origin: &str,
        sender_id: &str,
        reply_text: &str,
        llm_fallback_json: Option<&str>,
    ) -> McpResult<McpElicitationResult> {
        let key = unified_msg_origin.trim();
        let Some(pending) = self.sessions.get(key) else {
            return Err(McpError::Unsupported(
                "No active MCP elicitation is waiting for this unified message origin.".to_string(),
            ));
        };
        if pending.session.sender_id != sender_id.trim() {
            return Err(McpError::Unsupported(
                "MCP elicitation reply came from a different sender.".to_string(),
            ));
        }
        if Instant::now() >= pending.expires_at {
            self.sessions.remove(key);
            return Ok(McpElicitationResult::cancel());
        }

        let request = pending.request.clone();
        let result = match request {
            McpElicitationRequest::Form {
                requested_schema, ..
            } => match parse_form_reply(&requested_schema, reply_text) {
                Ok(result) => Ok(result),
                Err(err) => {
                    if let Some(fallback) = llm_fallback_json {
                        match parse_llm_fallback_json(&requested_schema, fallback) {
                            Some(Ok(result)) => Ok(result),
                            Some(Err(_)) | None => Err(err),
                        }
                    } else {
                        Err(err)
                    }
                }
            },
            McpElicitationRequest::Url { .. } => parse_url_reply(reply_text),
        }?;

        if matches!(
            result.action,
            McpElicitationAction::Accept
                | McpElicitationAction::Decline
                | McpElicitationAction::Cancel
        ) {
            self.sessions.remove(key);
        }
        Ok(result)
    }

    pub fn cancel_expired(&mut self, unified_msg_origin: &str) -> Option<McpElicitationResult> {
        let key = unified_msg_origin.trim();
        let expired = self
            .sessions
            .get(key)
            .is_some_and(|pending| Instant::now() >= pending.expires_at);
        if expired {
            self.sessions.remove(key);
            Some(McpElicitationResult::cancel())
        } else {
            None
        }
    }

    pub fn has_active_session(&self, unified_msg_origin: &str) -> bool {
        self.sessions.contains_key(unified_msg_origin.trim())
    }

    fn clear_expired_for(&mut self, unified_msg_origin: &str) {
        let expired = self
            .sessions
            .get(unified_msg_origin)
            .is_some_and(|pending| Instant::now() >= pending.expires_at);
        if expired {
            self.sessions.remove(unified_msg_origin);
        }
    }
}

#[derive(Debug)]
struct PendingElicitationSession {
    session: McpElicitationSession,
    request: McpElicitationRequest,
    expires_at: Instant,
}

pub fn parse_form_reply(
    requested_schema: &McpElicitationSchema,
    reply_text: &str,
) -> McpResult<McpElicitationResult> {
    if let Some(action) = parse_cancel_or_decline_action(reply_text) {
        return Ok(action_result(action));
    }
    let content = parse_form_content(requested_schema, reply_text)?;
    Ok(McpElicitationResult::accept(content))
}

pub fn parse_url_reply(reply_text: &str) -> McpResult<McpElicitationResult> {
    parse_url_action(reply_text)
        .map(action_result)
        .ok_or_else(|| {
            McpError::Unsupported(
                "Please reply `done`, `decline`, or `cancel` to continue this MCP request."
                    .to_string(),
            )
        })
}

pub fn parse_llm_fallback_json(
    requested_schema: &McpElicitationSchema,
    text: &str,
) -> Option<McpResult<McpElicitationResult>> {
    let text = strip_code_fence(text);
    if text.trim().is_empty() {
        return None;
    }
    let payload = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(payload)) => payload,
        Ok(_) => {
            return Some(Err(McpError::Protocol(
                "LLM fallback must return a JSON object.".to_string(),
            )));
        }
        Err(err) => {
            return Some(Err(McpError::Protocol(format!(
                "LLM fallback returned invalid JSON: {err}"
            ))));
        }
    };
    let payload = payload
        .into_iter()
        .map(|(key, value)| json_to_elicitation_value(value).map(|value| (key, value)))
        .collect::<McpResult<BTreeMap<_, _>>>();
    Some(payload.and_then(|payload| {
        coerce_form_payload(requested_schema, payload).map(McpElicitationResult::accept)
    }))
}

fn parse_form_content(
    requested_schema: &McpElicitationSchema,
    reply_text: &str,
) -> McpResult<BTreeMap<String, McpElicitationValue>> {
    if requested_schema.properties.is_empty() {
        return Ok(BTreeMap::new());
    }
    let reply_text = strip_code_fence(reply_text);
    let normalized = reply_text.trim();
    if normalized.is_empty() {
        return Err(McpError::Unsupported("The reply is empty.".to_string()));
    }

    let payload = if normalized.starts_with('{') {
        let value = serde_json::from_str::<serde_json::Value>(normalized).map_err(|err| {
            McpError::Protocol(format!("The JSON reply could not be parsed: {err}"))
        })?;
        let serde_json::Value::Object(object) = value else {
            return Err(McpError::Protocol(
                "The JSON reply must be an object.".to_string(),
            ));
        };
        object
            .into_iter()
            .map(|(key, value)| json_to_elicitation_value(value).map(|value| (key, value)))
            .collect::<McpResult<BTreeMap<_, _>>>()?
    } else if requested_schema.properties.len() == 1 {
        let field_name = requested_schema
            .properties
            .keys()
            .next()
            .expect("one field should exist")
            .clone();
        BTreeMap::from([(
            field_name,
            McpElicitationValue::String(normalized.to_string()),
        )])
    } else {
        let parsed = parse_key_value_lines(normalized, requested_schema);
        if parsed.is_empty() {
            parse_natural_language_reply(normalized, requested_schema)
        } else {
            parsed
        }
    };

    coerce_form_payload(requested_schema, payload)
}

fn coerce_form_payload(
    requested_schema: &McpElicitationSchema,
    payload: BTreeMap<String, McpElicitationValue>,
) -> McpResult<BTreeMap<String, McpElicitationValue>> {
    let normalized_keys = requested_schema
        .properties
        .keys()
        .map(|name| (name.to_ascii_lowercase(), name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut coerced = BTreeMap::new();
    for (raw_key, raw_value) in payload {
        let Some(field_name) = normalized_keys.get(&raw_key.trim().to_ascii_lowercase()) else {
            continue;
        };
        let field = requested_schema
            .properties
            .get(field_name)
            .expect("normalized field should exist");
        coerced.insert(
            field_name.clone(),
            coerce_value(field_name, raw_value, field)?,
        );
    }

    let missing = requested_schema
        .required
        .iter()
        .filter(|field| !coerced.contains_key(*field))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(McpError::Unsupported(format!(
            "Missing required field(s): {}",
            missing.join(", ")
        )));
    }
    Ok(coerced)
}

fn coerce_value(
    field_name: &str,
    raw_value: McpElicitationValue,
    field: &McpElicitationField,
) -> McpResult<McpElicitationValue> {
    let value = match field.field_type {
        McpElicitationFieldType::String => McpElicitationValue::String(match raw_value {
            McpElicitationValue::String(value) => value.trim().to_string(),
            other => value_to_string(other),
        }),
        McpElicitationFieldType::Integer => {
            let raw = value_to_string(raw_value);
            let value = raw.trim().parse::<i64>().map_err(|_| {
                McpError::Unsupported(format!("Field `{field_name}` must be an integer."))
            })?;
            McpElicitationValue::Integer(value)
        }
        McpElicitationFieldType::Number => {
            let raw = value_to_string(raw_value);
            let value = raw.trim().parse::<f64>().map_err(|_| {
                McpError::Unsupported(format!("Field `{field_name}` must be a number."))
            })?;
            McpElicitationValue::Number(value)
        }
        McpElicitationFieldType::Boolean => {
            McpElicitationValue::Boolean(coerce_boolean(field_name, raw_value)?)
        }
        McpElicitationFieldType::Array => McpElicitationValue::StringArray(match raw_value {
            McpElicitationValue::StringArray(values) => values,
            McpElicitationValue::String(value) => value
                .lines()
                .flat_map(|line| line.split(','))
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect(),
            other => vec![value_to_string(other)],
        }),
    };

    if !field.enum_values.is_empty() && !field.enum_values.contains(&value) {
        return Err(McpError::Unsupported(format!(
            "Field `{field_name}` must be one of: {}.",
            field
                .enum_values
                .iter()
                .map(|value| value_to_string(value.clone()))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(value)
}

fn coerce_boolean(field_name: &str, raw_value: McpElicitationValue) -> McpResult<bool> {
    if let McpElicitationValue::Boolean(value) = raw_value {
        return Ok(value);
    }
    let normalized = value_to_string(raw_value).trim().to_ascii_lowercase();
    match normalized.as_str() {
        "true" | "1" | "yes" | "y" | "on" | "是" | "好的" => Ok(true),
        "false" | "0" | "no" | "n" | "off" | "否" | "不是" => Ok(false),
        _ => Err(McpError::Unsupported(format!(
            "Field `{field_name}` must be a boolean."
        ))),
    }
}

fn parse_key_value_lines(
    reply_text: &str,
    requested_schema: &McpElicitationSchema,
) -> BTreeMap<String, McpElicitationValue> {
    let normalized_keys = requested_schema
        .properties
        .keys()
        .map(|name| (name.to_ascii_lowercase(), name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut parsed = BTreeMap::new();
    for line in reply_text.lines() {
        let Some((key, value)) = line.split_once(':').or_else(|| line.split_once('：')) else {
            continue;
        };
        let Some(field_name) = normalized_keys.get(&key.trim().to_ascii_lowercase()) else {
            continue;
        };
        parsed.insert(
            field_name.clone(),
            McpElicitationValue::String(value.trim().to_string()),
        );
    }
    parsed
}

fn parse_natural_language_reply(
    reply_text: &str,
    requested_schema: &McpElicitationSchema,
) -> BTreeMap<String, McpElicitationValue> {
    let lower = reply_text.to_ascii_lowercase();
    let mut parsed = BTreeMap::new();
    for (field_name, field) in &requested_schema.properties {
        let field_lower = field_name.to_ascii_lowercase();
        for separator in [":", "=", " is ", " 是 ", " 为 "] {
            let pattern = format!("{field_lower}{separator}");
            if let Some(start) = lower.find(&pattern) {
                let value_start = start + pattern.len();
                let value = reply_text[value_start..]
                    .split([',', '，', ';', '；', '。'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches(['`', '\'', '"']);
                if !value.is_empty() {
                    parsed.insert(
                        field_name.clone(),
                        McpElicitationValue::String(value.to_string()),
                    );
                }
            }
        }
        if parsed.contains_key(field_name) || field.enum_values.is_empty() {
            continue;
        }
        let matches = field
            .enum_values
            .iter()
            .filter(|value| lower.contains(&value_to_string((*value).clone()).to_ascii_lowercase()))
            .cloned()
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            parsed.insert(field_name.clone(), matches[0].clone());
        }
    }
    if parsed.is_empty() && requested_schema.required.len() == 1 {
        parsed.insert(
            requested_schema.required[0].clone(),
            McpElicitationValue::String(reply_text.trim().to_string()),
        );
    }
    parsed
}

fn parse_cancel_or_decline_action(reply_text: &str) -> Option<McpElicitationAction> {
    let normalized = reply_text.trim().to_ascii_lowercase();
    if CANCEL_KEYWORDS.contains(&normalized.as_str()) {
        Some(McpElicitationAction::Cancel)
    } else if DECLINE_KEYWORDS.contains(&normalized.as_str()) {
        Some(McpElicitationAction::Decline)
    } else {
        None
    }
}

fn parse_url_action(reply_text: &str) -> Option<McpElicitationAction> {
    let normalized = reply_text.trim().to_ascii_lowercase();
    if ACCEPT_KEYWORDS.contains(&normalized.as_str()) {
        Some(McpElicitationAction::Accept)
    } else if DECLINE_KEYWORDS.contains(&normalized.as_str()) {
        Some(McpElicitationAction::Decline)
    } else if CANCEL_KEYWORDS.contains(&normalized.as_str()) {
        Some(McpElicitationAction::Cancel)
    } else {
        None
    }
}

fn action_result(action: McpElicitationAction) -> McpElicitationResult {
    match action {
        McpElicitationAction::Accept => McpElicitationResult::accept(BTreeMap::new()),
        McpElicitationAction::Decline => McpElicitationResult::decline(),
        McpElicitationAction::Cancel => McpElicitationResult::cancel(),
    }
}

fn build_elicitation_prompt(server_name: &str, request: &McpElicitationRequest) -> String {
    match request {
        McpElicitationRequest::Form {
            message,
            requested_schema,
        } => {
            let mut lines = vec![format!(
                "MCP server `{server_name}` needs more information."
            )];
            if !message.trim().is_empty() {
                lines.push(message.trim().to_string());
            }
            if !requested_schema.properties.is_empty() {
                lines.push("Requested fields:".to_string());
                for (field_name, field) in &requested_schema.properties {
                    let suffix = if requested_schema.required.contains(field_name) {
                        " required"
                    } else {
                        " optional"
                    };
                    let field_type = format!("{:?}", field.field_type).to_ascii_lowercase();
                    if let Some(description) = &field.description {
                        lines.push(format!(
                            "- {field_name} ({field_type},{suffix}): {description}"
                        ));
                    } else {
                        lines.push(format!("- {field_name} ({field_type},{suffix})"));
                    }
                }
            }
            lines.push("Reply with JSON or `field: value` lines.".to_string());
            lines.push("Reply `decline` to refuse or `cancel` to stop.".to_string());
            lines.join("\n")
        }
        McpElicitationRequest::Url { message, url } => {
            let mut lines = vec![format!(
                "MCP server `{server_name}` needs an external confirmation step."
            )];
            if !message.trim().is_empty() {
                lines.push(message.trim().to_string());
            }
            lines.push(format!("URL: {url}"));
            lines.push(
                "Reply `done` after you finish, `decline` to refuse, or `cancel` to stop."
                    .to_string(),
            );
            lines.join("\n")
        }
    }
}

fn strip_code_fence(text: &str) -> String {
    let stripped = text.trim();
    if !stripped.starts_with("```") || !stripped.ends_with("```") {
        return stripped.to_string();
    }
    let lines = stripped.lines().collect::<Vec<_>>();
    if lines.len() <= 2 {
        stripped
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        lines[1..lines.len() - 1].join("\n").trim().to_string()
    }
}

fn normalized_non_empty(value: impl Into<String>, label: &str) -> McpResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() {
        Err(McpError::InvalidConfig(format!("{label} cannot be empty")))
    } else {
        Ok(value)
    }
}

fn json_to_elicitation_value(value: serde_json::Value) -> McpResult<McpElicitationValue> {
    match value {
        serde_json::Value::Null => Ok(McpElicitationValue::Null),
        serde_json::Value::Bool(value) => Ok(McpElicitationValue::Boolean(value)),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(McpElicitationValue::Integer(integer))
            } else if let Some(number) = value.as_f64() {
                Ok(McpElicitationValue::Number(number))
            } else {
                Err(McpError::Protocol(
                    "unsupported elicitation JSON number".to_string(),
                ))
            }
        }
        serde_json::Value::String(value) => Ok(McpElicitationValue::String(value)),
        serde_json::Value::Array(values) => Ok(McpElicitationValue::StringArray(
            values
                .into_iter()
                .map(|value| match value {
                    serde_json::Value::String(value) => value,
                    other => other.to_string(),
                })
                .collect(),
        )),
        serde_json::Value::Object(_) => Err(McpError::Protocol(
            "nested elicitation objects are not supported".to_string(),
        )),
    }
}

fn value_to_string(value: McpElicitationValue) -> String {
    match value {
        McpElicitationValue::String(value) => value,
        McpElicitationValue::Integer(value) => value.to_string(),
        McpElicitationValue::Number(value) => value.to_string(),
        McpElicitationValue::Boolean(value) => value.to_string(),
        McpElicitationValue::StringArray(values) => values.join(", "),
        McpElicitationValue::Null => String::new(),
    }
}

const ACCEPT_KEYWORDS: &[&str] = &["accept", "done", "ok", "okay", "yes", "完成", "同意"];
const DECLINE_KEYWORDS: &[&str] = &["decline", "reject", "refuse", "no", "拒绝", "不同意"];
const CANCEL_KEYWORDS: &[&str] = &["cancel", "stop", "退出", "取消"];
