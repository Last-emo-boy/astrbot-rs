use astrbot_core::{AstrbotError, Result};
use astrbot_platform::{PlatformManager, SentMessage};

pub(super) async fn emit_mock_text_on(
    platform_manager: &PlatformManager,
    platform_id: &str,
    event_id: impl Into<String>,
    conversation_id: impl Into<String>,
    sender_id: impl Into<String>,
    text: impl Into<String>,
) -> Result<()> {
    let platform = platform_manager.mock_platform(platform_id).ok_or_else(|| {
        AstrbotError::Platform(format!("mock platform {platform_id} is not configured"))
    })?;
    platform
        .emit_text(event_id, conversation_id, sender_id, text)
        .await
}

pub(super) async fn sent_messages_for(
    platform_manager: &PlatformManager,
    platform_id: &str,
) -> Vec<SentMessage> {
    let Some(sink) = platform_manager.recording_sink(platform_id) else {
        return Vec::new();
    };
    sink.messages().await
}
