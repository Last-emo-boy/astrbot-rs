use astrbot_core::MessageSession;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OneBotSessionKind {
    Private,
    Group,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneBotSession {
    platform_id: String,
    kind: OneBotSessionKind,
    session_id: String,
    message_id: Option<String>,
}

impl OneBotSession {
    pub fn private(platform_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            platform_id: platform_id.into(),
            kind: OneBotSessionKind::Private,
            session_id: user_id.into(),
            message_id: None,
        }
    }

    pub fn group(platform_id: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            platform_id: platform_id.into(),
            kind: OneBotSessionKind::Group,
            session_id: group_id.into(),
            message_id: None,
        }
    }

    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    pub fn kind(&self) -> &OneBotSessionKind {
        &self.kind
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn message_id(&self) -> Option<&str> {
        self.message_id.as_deref()
    }

    pub fn message_session(&self) -> MessageSession {
        match self.kind {
            OneBotSessionKind::Private => {
                MessageSession::new(self.platform_id.clone(), self.conversation_id())
            }
            OneBotSessionKind::Group => {
                MessageSession::group(self.platform_id.clone(), self.conversation_id())
            }
        }
    }

    pub fn conversation_id(&self) -> String {
        match self.kind {
            OneBotSessionKind::Private => format!("private:{}", self.session_id),
            OneBotSessionKind::Group => format!("group:{}", self.session_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OneBotSession, OneBotSessionKind};

    #[test]
    fn onebot_session_tracks_private_and_group_metadata() {
        let private = OneBotSession::private("onebot", "user-1").with_message_id("msg-1");
        let group = OneBotSession::group("onebot", "group-1");

        assert_eq!(private.kind(), &OneBotSessionKind::Private);
        assert_eq!(private.session_id(), "user-1");
        assert_eq!(private.message_id(), Some("msg-1"));
        assert_eq!(private.message_session().conversation_id, "private:user-1");

        assert_eq!(group.kind(), &OneBotSessionKind::Group);
        assert_eq!(group.message_session().conversation_id, "group:group-1");
        assert!(group.message_session().is_group());
    }
}
