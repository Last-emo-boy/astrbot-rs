use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageComponent {
    Plain {
        text: String,
    },
    Image {
        url: String,
    },
    Record {
        url: String,
    },
    Video {
        url: String,
    },
    File {
        name: String,
        url: String,
    },
    Mention {
        user_id: String,
    },
    MentionAll,
    Reply {
        message_id: String,
        selected_text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sender_id: Option<String>,
    },
}

impl MessageComponent {
    pub fn plain(text: impl Into<String>) -> Self {
        Self::Plain { text: text.into() }
    }

    pub fn image(url: impl Into<String>) -> Self {
        Self::Image { url: url.into() }
    }

    pub fn record(url: impl Into<String>) -> Self {
        Self::Record { url: url.into() }
    }

    pub fn video(url: impl Into<String>) -> Self {
        Self::Video { url: url.into() }
    }

    pub fn file(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self::File {
            name: name.into(),
            url: url.into(),
        }
    }

    pub fn mention(user_id: impl Into<String>) -> Self {
        Self::Mention {
            user_id: user_id.into(),
        }
    }

    pub fn mention_all() -> Self {
        Self::MentionAll
    }

    pub fn reply(message_id: impl Into<String>, selected_text: impl Into<String>) -> Self {
        Self::Reply {
            message_id: message_id.into(),
            selected_text: selected_text.into(),
            sender_id: None,
        }
    }

    pub fn reply_to_sender(
        message_id: impl Into<String>,
        selected_text: impl Into<String>,
        sender_id: impl Into<String>,
    ) -> Self {
        Self::Reply {
            message_id: message_id.into(),
            selected_text: selected_text.into(),
            sender_id: Some(sender_id.into()),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Plain { text } => text.trim().is_empty(),
            Self::Image { url }
            | Self::Record { url }
            | Self::Video { url }
            | Self::File { url, .. } => url.trim().is_empty(),
            Self::Mention { .. } | Self::MentionAll | Self::Reply { .. } => true,
        }
    }

    pub fn has_sendable_content(&self) -> bool {
        match self {
            Self::Plain { text } => !text.trim().is_empty(),
            Self::Image { url }
            | Self::Record { url }
            | Self::Video { url }
            | Self::File { url, .. } => !url.trim().is_empty(),
            Self::Mention { .. } | Self::MentionAll | Self::Reply { .. } => false,
        }
    }

    pub fn is_valid_send_component(&self) -> bool {
        match self {
            Self::Plain { .. }
            | Self::Image { .. }
            | Self::Record { .. }
            | Self::Video { .. }
            | Self::File { .. } => self.has_sendable_content(),
            Self::Mention { user_id } => !user_id.trim().is_empty(),
            Self::MentionAll => true,
            Self::Reply { message_id, .. } => !message_id.trim().is_empty(),
        }
    }
}
