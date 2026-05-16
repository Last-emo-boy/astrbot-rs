use astrbot_core::{MessageComponent, MessageEvent, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage, WakeCheckConfig};

#[derive(Default)]
pub struct WakeCheckStage;

#[async_trait]
impl PipelineStage for WakeCheckStage {
    fn name(&self) -> &str {
        "wake"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        let config = ctx.wake_check();
        let bot_self_id = event.self_id().or(config.bot_self_id.as_deref());

        if should_ignore_bot_self_message(event, config, bot_self_id) {
            event.stop();
            return Ok(PipelineControl::Stop);
        }

        if let Some(prefix) = matched_wake_prefix(event, config)
            && (!event.session.is_group() || !starts_with_non_bot_mention(event, bot_self_id))
        {
            event.message.trim_plain_text_prefix(prefix);
            event.mark_wake(true);
            return Ok(PipelineControl::Continue);
        }

        if addressed_to_bot(event, config, bot_self_id)
            || (event.session.is_direct() && !config.direct_message_needs_wake_prefix)
        {
            event.mark_wake(true);
            return Ok(PipelineControl::Continue);
        }

        event.stop();
        Ok(PipelineControl::Stop)
    }
}

fn should_ignore_bot_self_message(
    event: &MessageEvent,
    config: &WakeCheckConfig,
    bot_self_id: Option<&str>,
) -> bool {
    config.ignore_bot_self_message && bot_self_id == Some(event.sender.id.as_str())
}

fn matched_wake_prefix<'a>(event: &MessageEvent, config: &'a WakeCheckConfig) -> Option<&'a str> {
    let text = event.message.plain_text();
    let text = text.trim_start();
    config
        .wake_prefixes
        .iter()
        .find(|prefix| text.starts_with(prefix.as_str()))
        .map(String::as_str)
}

fn starts_with_non_bot_mention(event: &MessageEvent, bot_self_id: Option<&str>) -> bool {
    let Some(MessageComponent::Mention { user_id }) = event.message.components().first() else {
        return false;
    };

    bot_self_id != Some(user_id.as_str())
}

fn addressed_to_bot(
    event: &MessageEvent,
    config: &WakeCheckConfig,
    bot_self_id: Option<&str>,
) -> bool {
    let mentions_bot = bot_self_id
        .map(|bot_self_id| {
            event.message.mentions_user(bot_self_id) || event.message.replies_to_user(bot_self_id)
        })
        .unwrap_or(false);
    let mentions_all = !config.ignore_at_all && event.message.mentions_all();

    mentions_bot || mentions_all
}
