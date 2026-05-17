use std::sync::Arc;

use astrbot_core::{MessageChain, MessageEvent, MessageSender, MessageSession};

use crate::PlatformIdentityNormalizer;
use crate::RecordingSink;

pub(super) fn build_webchat_event(
    event_id: String,
    platform_id: String,
    platform_name: String,
    conversation_id: impl Into<String>,
    sender_id: impl Into<String>,
    message: MessageChain,
    sink: Arc<RecordingSink>,
) -> MessageEvent {
    let sender = MessageSender::new(sender_id, None);
    let identity = PlatformIdentityNormalizer::normalize_direct_event(&sender);
    MessageEvent::new(
        event_id,
        platform_id.clone(),
        platform_name,
        MessageSession::new(platform_id, conversation_id),
        sender,
        message,
        sink,
    )
    .with_identity(identity)
}
