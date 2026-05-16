use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage, WhitelistPolicyConfig};

#[derive(Default)]
pub struct WhitelistCheckStage;

#[async_trait]
impl PipelineStage for WhitelistCheckStage {
    fn name(&self) -> &str {
        "whitelist"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        let config = ctx.whitelist_policy();
        if !config.enabled || config.allowed_ids.is_empty() || platform_bypasses(event, config) {
            return Ok(PipelineControl::Continue);
        }

        if admin_bypasses(event, config) || session_allowed(event, config) {
            return Ok(PipelineControl::Continue);
        }

        event.stop();
        Ok(PipelineControl::Stop)
    }
}

fn platform_bypasses(event: &MessageEvent, config: &WhitelistPolicyConfig) -> bool {
    config
        .bypass_platform_ids
        .iter()
        .any(|platform_id| platform_id == &event.platform_id)
}

fn admin_bypasses(event: &MessageEvent, config: &WhitelistPolicyConfig) -> bool {
    let sender_is_admin = config
        .admin_user_ids
        .iter()
        .any(|admin_id| admin_id == &event.sender.id);
    if !sender_is_admin {
        return false;
    }

    (event.session.is_group() && config.ignore_admin_on_group)
        || (event.session.is_direct() && config.ignore_admin_on_direct)
}

fn session_allowed(event: &MessageEvent, config: &WhitelistPolicyConfig) -> bool {
    config.allowed_ids.iter().any(|allowed_id| {
        allowed_id == &event.session.conversation_id || allowed_id == &event.sender.id
    })
}
