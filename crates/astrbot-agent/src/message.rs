use astrbot_core::{ProviderContentPart, ProviderContextMessage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl AgentMessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolCall {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

impl AgentToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: None,
        }
    }

    pub fn with_arguments(mut self, arguments: impl Into<String>) -> Self {
        self.arguments = non_empty_option(arguments);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolCallPart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments_part: Option<String>,
}

impl AgentToolCallPart {
    pub fn new(arguments_part: impl Into<String>) -> Self {
        Self {
            arguments_part: non_empty_option(arguments_part),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: AgentMessageRole,
    #[serde(default)]
    pub parts: Vec<ProviderContentPart>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<AgentToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_save: bool,
}

impl AgentMessage {
    pub fn new(role: AgentMessageRole, parts: Vec<ProviderContentPart>) -> Self {
        Self {
            role,
            parts,
            tool_calls: Vec::new(),
            tool_call_id: None,
            no_save: false,
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self::text(AgentMessageRole::System, text)
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self::text(AgentMessageRole::User, text)
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::text(AgentMessageRole::Assistant, text)
    }

    pub fn tool(tool_call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::text(AgentMessageRole::Tool, text).with_tool_call_id(tool_call_id)
    }

    pub fn assistant_tool_call(tool_call: AgentToolCall) -> Self {
        Self {
            role: AgentMessageRole::Assistant,
            parts: Vec::new(),
            tool_calls: vec![tool_call],
            tool_call_id: None,
            no_save: false,
        }
    }

    pub fn text(role: AgentMessageRole, text: impl Into<String>) -> Self {
        Self::new(role, vec![ProviderContentPart::text(text)])
    }

    pub fn with_part(mut self, part: ProviderContentPart) -> Self {
        self.parts.push(part);
        self
    }

    pub fn with_tool_call(mut self, tool_call: AgentToolCall) -> Self {
        self.tool_calls.push(tool_call);
        self
    }

    pub fn with_tool_call_id(mut self, tool_call_id: impl Into<String>) -> Self {
        self.tool_call_id = non_empty_option(tool_call_id);
        self
    }

    pub fn mark_no_save(mut self) -> Self {
        self.no_save = true;
        self
    }

    pub fn is_valid(&self) -> bool {
        self.role == AgentMessageRole::Assistant && !self.tool_calls.is_empty()
            || !self.parts.is_empty()
    }
}

impl From<ProviderContextMessage> for AgentMessage {
    fn from(message: ProviderContextMessage) -> Self {
        let role = match message.role.as_str() {
            "system" => AgentMessageRole::System,
            "assistant" => AgentMessageRole::Assistant,
            "tool" => AgentMessageRole::Tool,
            _ => AgentMessageRole::User,
        };
        Self::new(role, message.parts)
    }
}

impl From<AgentMessage> for ProviderContextMessage {
    fn from(message: AgentMessage) -> Self {
        Self::new(message.role.as_str(), message.parts)
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}

fn is_false(value: &bool) -> bool {
    !*value
}
