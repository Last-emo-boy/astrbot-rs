use super::{PlatformMediaKind, PlatformMediaReference};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichPlatformEvent {
    pub platform_type: String,
    pub event_id: String,
    pub session_id: String,
    pub sender_id: String,
    parts: Vec<RichEventPart>,
}

impl RichPlatformEvent {
    pub fn new(
        platform_type: impl Into<String>,
        event_id: impl Into<String>,
        session_id: impl Into<String>,
        sender_id: impl Into<String>,
    ) -> Self {
        Self {
            platform_type: platform_type.into(),
            event_id: event_id.into(),
            session_id: session_id.into(),
            sender_id: sender_id.into(),
            parts: Vec::new(),
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        if !text.is_empty() {
            self.parts.push(RichEventPart::Text(text));
        }
        self
    }

    pub fn with_part(mut self, part: RichEventPart) -> Self {
        self.parts.push(part);
        self
    }

    pub fn parts(&self) -> &[RichEventPart] {
        &self.parts
    }

    pub fn plain_text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                RichEventPart::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn normalized(mut self) -> Self {
        self.parts = merge_adjacent_text_parts(self.parts);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichEventPart {
    Text(String),
    Mention {
        user_id: String,
        display_name: Option<String>,
    },
    Media(RichEventMedia),
    Reaction(RichEventReaction),
    Thread(RichEventThread),
    Raw {
        kind: String,
        summary: String,
    },
}

impl RichEventPart {
    pub fn mention(user_id: impl Into<String>) -> Self {
        Self::Mention {
            user_id: user_id.into(),
            display_name: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        if let Self::Mention {
            display_name: known,
            ..
        } = &mut self
        {
            let display_name = display_name.into();
            *known = (!display_name.trim().is_empty()).then_some(display_name);
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichEventMedia {
    pub reference: PlatformMediaReference,
    pub caption: Option<String>,
    pub duration_ms: Option<u64>,
    pub platform_download_id: Option<String>,
}

impl RichEventMedia {
    pub fn reference(reference: PlatformMediaReference) -> Self {
        Self {
            reference,
            caption: None,
            duration_ms: None,
            platform_download_id: None,
        }
    }

    pub fn download_code(kind: PlatformMediaKind, platform_download_id: impl Into<String>) -> Self {
        let platform_download_id = platform_download_id.into();
        Self {
            reference: PlatformMediaReference::media_id(kind, platform_download_id.clone()),
            caption: None,
            duration_ms: None,
            platform_download_id: Some(platform_download_id),
        }
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        let caption = caption.into();
        self.caption = (!caption.trim().is_empty()).then_some(caption);
        self
    }

    pub fn with_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_platform_download_id(mut self, platform_download_id: impl Into<String>) -> Self {
        let platform_download_id = platform_download_id.into();
        self.platform_download_id =
            (!platform_download_id.trim().is_empty()).then_some(platform_download_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichEventReaction {
    pub message_id: String,
    pub emoji: String,
    pub custom_emoji_id: Option<String>,
    pub big: bool,
}

impl RichEventReaction {
    pub fn new(message_id: impl Into<String>, emoji: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            emoji: emoji.into(),
            custom_emoji_id: None,
            big: false,
        }
    }

    pub fn custom_emoji(mut self, custom_emoji_id: impl Into<String>) -> Self {
        let custom_emoji_id = custom_emoji_id.into();
        self.custom_emoji_id = (!custom_emoji_id.trim().is_empty()).then_some(custom_emoji_id);
        self
    }

    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichEventThread {
    pub root_message_id: Option<String>,
    pub reply_to_message_id: Option<String>,
    pub thread_id: Option<String>,
}

impl RichEventThread {
    pub fn new() -> Self {
        Self {
            root_message_id: None,
            reply_to_message_id: None,
            thread_id: None,
        }
    }

    pub fn with_root_message_id(mut self, root_message_id: impl Into<String>) -> Self {
        let root_message_id = root_message_id.into();
        self.root_message_id = (!root_message_id.trim().is_empty()).then_some(root_message_id);
        self
    }

    pub fn with_reply_to_message_id(mut self, reply_to_message_id: impl Into<String>) -> Self {
        let reply_to_message_id = reply_to_message_id.into();
        self.reply_to_message_id =
            (!reply_to_message_id.trim().is_empty()).then_some(reply_to_message_id);
        self
    }

    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        let thread_id = thread_id.into();
        self.thread_id = (!thread_id.trim().is_empty()).then_some(thread_id);
        self
    }
}

impl Default for RichEventThread {
    fn default() -> Self {
        Self::new()
    }
}

fn merge_adjacent_text_parts(parts: Vec<RichEventPart>) -> Vec<RichEventPart> {
    let mut normalized = Vec::new();
    for part in parts {
        match (normalized.last_mut(), part) {
            (Some(RichEventPart::Text(existing)), RichEventPart::Text(next)) => {
                existing.push_str(&next);
            }
            (_, part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        RichEventMedia, RichEventPart, RichEventReaction, RichEventThread, RichPlatformEvent,
    };
    use crate::{PlatformMediaKind, PlatformMediaReference};

    #[test]
    fn rich_event_normalization_preserves_media_reaction_and_thread_metadata() {
        let event = RichPlatformEvent::new("telegram", "evt-1", "chat#42", "user")
            .with_text("hello ")
            .with_text("world")
            .with_part(RichEventPart::Thread(
                RichEventThread::new()
                    .with_thread_id("42")
                    .with_reply_to_message_id("msg-0"),
            ))
            .with_part(RichEventPart::Reaction(
                RichEventReaction::new("msg-1", "thumbs_up").big(),
            ))
            .with_part(RichEventPart::Media(
                RichEventMedia::reference(PlatformMediaReference::url(
                    PlatformMediaKind::Image,
                    "https://example.test/image.png",
                ))
                .with_caption("image"),
            ))
            .normalized();

        assert_eq!(event.plain_text(), "hello world");
        assert_eq!(event.parts().len(), 4);
        assert!(matches!(event.parts()[1], RichEventPart::Thread(_)));
        assert!(matches!(event.parts()[2], RichEventPart::Reaction(_)));
        assert!(matches!(event.parts()[3], RichEventPart::Media(_)));
    }

    #[test]
    fn platform_download_codes_stay_outside_core_message_event_types() {
        let media = RichEventMedia::download_code(PlatformMediaKind::Audio, "download-code")
            .with_duration_ms(1200);

        assert_eq!(media.reference.media_id.as_deref(), Some("download-code"));
        assert_eq!(media.platform_download_id.as_deref(), Some("download-code"));
        assert_eq!(media.duration_ms, Some(1200));
    }
}
