use std::collections::BTreeMap;

use astrbot_core::{MessageSender, MessageSession, MessageSessionKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformSessionScene {
    Direct,
    Group,
    Channel,
    Unknown(String),
}

impl PlatformSessionScene {
    pub fn from_session_kind(kind: MessageSessionKind) -> Self {
        match kind {
            MessageSessionKind::Direct => Self::Direct,
            MessageSessionKind::Group => Self::Group,
        }
    }

    pub fn default_target_kind(&self) -> PlatformRouteTargetKind {
        match self {
            Self::Direct => PlatformRouteTargetKind::UserId,
            Self::Group => PlatformRouteTargetKind::GroupId,
            Self::Channel => PlatformRouteTargetKind::ChannelId,
            Self::Unknown(value) => PlatformRouteTargetKind::Custom(value.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformRouteTargetKind {
    UserId,
    GroupId,
    ChannelId,
    OpenConversationId,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformReplyTarget {
    pub message_id: String,
    pub sender_id: Option<String>,
    pub thread_id: Option<String>,
}

impl PlatformReplyTarget {
    pub fn new(message_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            sender_id: None,
            thread_id: None,
        }
    }

    pub fn with_sender_id(mut self, sender_id: impl Into<String>) -> Self {
        self.sender_id = non_empty(sender_id);
        self
    }

    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = non_empty(thread_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformSenderBinding {
    pub sender: MessageSender,
    external_ids: BTreeMap<String, String>,
}

impl PlatformSenderBinding {
    pub fn new(sender: MessageSender) -> Self {
        Self {
            sender,
            external_ids: BTreeMap::new(),
        }
    }

    pub fn with_external_id(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let key = key.trim();
        if let Some(value) = non_empty(value)
            && !key.is_empty()
        {
            self.external_ids.insert(key.to_string(), value);
        }
        self
    }

    pub fn external_id(&self, key: &str) -> Option<&str> {
        self.external_ids.get(key.trim()).map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProactiveSendRequirement {
    None,
    RecentMessageId,
    SenderExternalId(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProactiveSendReadiness {
    Ready,
    MissingRecentMessageId,
    MissingSenderExternalId(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformOutboundRoute {
    pub platform_id: String,
    pub conversation_id: String,
    pub platform_session_id: String,
    pub scene: PlatformSessionScene,
    pub target_kind: PlatformRouteTargetKind,
    pub last_message_id: Option<String>,
    pub reply_target: Option<PlatformReplyTarget>,
    pub sender_binding: Option<PlatformSenderBinding>,
    pub readiness: ProactiveSendReadiness,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformOutboundRoutingState {
    pub session: MessageSession,
    pub platform_session_id: String,
    pub scene: PlatformSessionScene,
    pub last_inbound_message_id: Option<String>,
    pub last_outbound_message_id: Option<String>,
    pub reply_target: Option<PlatformReplyTarget>,
    pub sender_binding: Option<PlatformSenderBinding>,
}

impl PlatformOutboundRoutingState {
    pub fn from_session(
        session: MessageSession,
        platform_session_id: impl Into<String>,
        scene: PlatformSessionScene,
    ) -> Self {
        Self {
            session,
            platform_session_id: platform_session_id.into(),
            scene,
            last_inbound_message_id: None,
            last_outbound_message_id: None,
            reply_target: None,
            sender_binding: None,
        }
    }

    pub fn for_message_session(session: MessageSession) -> Self {
        let scene = PlatformSessionScene::from_session_kind(session.kind);
        let platform_session_id = session.conversation_id.clone();
        Self::from_session(session, platform_session_id, scene)
    }

    pub fn with_last_inbound_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.last_inbound_message_id = non_empty(message_id);
        self
    }

    pub fn with_last_outbound_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.last_outbound_message_id = non_empty(message_id);
        self
    }

    pub fn with_reply_target(mut self, reply_target: PlatformReplyTarget) -> Self {
        self.reply_target = Some(reply_target);
        self
    }

    pub fn with_sender_binding(mut self, sender_binding: PlatformSenderBinding) -> Self {
        self.sender_binding = Some(sender_binding);
        self
    }

    pub fn select_route(&self, requirement: ProactiveSendRequirement) -> PlatformOutboundRoute {
        let readiness = match &requirement {
            ProactiveSendRequirement::None => ProactiveSendReadiness::Ready,
            ProactiveSendRequirement::RecentMessageId => {
                if self.recent_message_id().is_some() {
                    ProactiveSendReadiness::Ready
                } else {
                    ProactiveSendReadiness::MissingRecentMessageId
                }
            }
            ProactiveSendRequirement::SenderExternalId(key) => {
                if self
                    .sender_binding
                    .as_ref()
                    .and_then(|binding| binding.external_id(key))
                    .is_some()
                {
                    ProactiveSendReadiness::Ready
                } else {
                    ProactiveSendReadiness::MissingSenderExternalId(key.clone())
                }
            }
        };

        PlatformOutboundRoute {
            platform_id: self.session.platform_id.clone(),
            conversation_id: self.session.conversation_id.clone(),
            platform_session_id: self.platform_session_id.clone(),
            scene: self.scene.clone(),
            target_kind: self.scene.default_target_kind(),
            last_message_id: self.recent_message_id().map(str::to_string),
            reply_target: self.reply_target.clone(),
            sender_binding: self.sender_binding.clone(),
            readiness,
        }
    }

    fn recent_message_id(&self) -> Option<&str> {
        self.last_outbound_message_id
            .as_deref()
            .or(self.last_inbound_message_id.as_deref())
    }
}

fn non_empty(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use astrbot_core::{MessageSender, MessageSession};

    use super::{
        PlatformOutboundRoutingState, PlatformRouteTargetKind, PlatformSenderBinding,
        PlatformSessionScene, ProactiveSendReadiness, ProactiveSendRequirement,
    };

    #[test]
    fn qq_official_style_route_requires_recent_message_id() {
        let state = PlatformOutboundRoutingState::from_session(
            MessageSession::group("qq-official", "group:g1"),
            "g1",
            PlatformSessionScene::Group,
        )
        .with_last_inbound_message_id("msg-1");

        let route = state.select_route(ProactiveSendRequirement::RecentMessageId);

        assert_eq!(route.readiness, ProactiveSendReadiness::Ready);
        assert_eq!(route.target_kind, PlatformRouteTargetKind::GroupId);
        assert_eq!(route.platform_session_id, "g1");
        assert_eq!(route.last_message_id.as_deref(), Some("msg-1"));
    }

    #[test]
    fn route_selection_reports_missing_recent_message_id() {
        let state = PlatformOutboundRoutingState::from_session(
            MessageSession::new("qq-official", "private:u1"),
            "u1",
            PlatformSessionScene::Direct,
        );

        let route = state.select_route(ProactiveSendRequirement::RecentMessageId);

        assert_eq!(
            route.readiness,
            ProactiveSendReadiness::MissingRecentMessageId
        );
    }

    #[test]
    fn dingtalk_style_sender_binding_exposes_staff_id() {
        let sender = MessageSender::new("user-1", Some("User".to_string()));
        let state = PlatformOutboundRoutingState::from_session(
            MessageSession::new("dingtalk", "private:user-1"),
            "user-1",
            PlatformSessionScene::Direct,
        )
        .with_sender_binding(
            PlatformSenderBinding::new(sender).with_external_id("dingtalk_staff_id", "staff-1"),
        );

        let route = state.select_route(ProactiveSendRequirement::SenderExternalId(
            "dingtalk_staff_id".to_string(),
        ));

        assert_eq!(route.readiness, ProactiveSendReadiness::Ready);
        assert_eq!(
            route
                .sender_binding
                .as_ref()
                .and_then(|binding| binding.external_id("dingtalk_staff_id")),
            Some("staff-1")
        );
    }
}
