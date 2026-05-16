use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageSessionKind {
    #[default]
    Direct,
    Group,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSession {
    pub platform_id: String,
    pub conversation_id: String,
    #[serde(default)]
    pub kind: MessageSessionKind,
}

impl MessageSession {
    pub fn new(platform_id: impl Into<String>, conversation_id: impl Into<String>) -> Self {
        Self {
            platform_id: platform_id.into(),
            conversation_id: conversation_id.into(),
            kind: MessageSessionKind::Direct,
        }
    }

    pub fn group(platform_id: impl Into<String>, conversation_id: impl Into<String>) -> Self {
        Self {
            platform_id: platform_id.into(),
            conversation_id: conversation_id.into(),
            kind: MessageSessionKind::Group,
        }
    }

    pub fn with_kind(mut self, kind: MessageSessionKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn is_direct(&self) -> bool {
        self.kind == MessageSessionKind::Direct
    }

    pub fn is_group(&self) -> bool {
        self.kind == MessageSessionKind::Group
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSender {
    pub id: String,
    pub display_name: Option<String>,
}

impl MessageSender {
    pub fn new(id: impl Into<String>, display_name: Option<String>) -> Self {
        Self {
            id: id.into(),
            display_name,
        }
    }
}
