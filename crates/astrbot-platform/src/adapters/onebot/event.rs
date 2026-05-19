use std::sync::Arc;

use astrbot_core::{MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink};

use crate::{PlatformGroupIdentityInput, PlatformIdentityNormalizer};

pub(super) fn build_onebot_event(
    event_id: String,
    platform_id: String,
    platform_name: String,
    session: MessageSession,
    sender_id: impl Into<String>,
    message: MessageChain,
    sink: Arc<dyn MessageSink>,
) -> MessageEvent {
    let sender = MessageSender::new(sender_id, None);
    let group = if session.is_group() {
        Some(PlatformGroupIdentityInput::new(
            session
                .conversation_id
                .strip_prefix("group:")
                .unwrap_or(&session.conversation_id),
        ))
    } else {
        None
    };
    let identity = PlatformIdentityNormalizer::normalize_identity(
        sender.id.clone(),
        sender.display_name.clone(),
        group,
    );
    MessageEvent::new(
        event_id,
        platform_id,
        platform_name,
        session,
        sender,
        message,
        sink,
    )
    .with_identity(identity)
}
