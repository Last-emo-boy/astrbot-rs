use astrbot_core::Result;

use crate::stages::{
    ContentSafetyCheckStage, ProcessStage, RateLimitStage, RespondStage, ResultDecorateStage,
    SessionStatusCheckStage, WakeCheckStage, WhitelistCheckStage,
};

use super::{
    CONTENT_SAFETY_STAGE_ORDER, CONTENT_SAFETY_STAGE_TYPE, PROCESS_STAGE_ORDER, PROCESS_STAGE_TYPE,
    PipelineStageRegistry, RATE_LIMIT_STAGE_ORDER, RATE_LIMIT_STAGE_TYPE, RESPOND_STAGE_ORDER,
    RESPOND_STAGE_TYPE, RESULT_DECORATE_STAGE_ORDER, RESULT_DECORATE_STAGE_TYPE,
    SESSION_STATUS_STAGE_ORDER, SESSION_STATUS_STAGE_TYPE, WAKE_STAGE_ORDER, WAKE_STAGE_TYPE,
    WHITELIST_STAGE_ORDER, WHITELIST_STAGE_TYPE,
};

pub(super) fn register_builtin_stages(registry: &mut PipelineStageRegistry) -> Result<()> {
    registry.register_stage(WAKE_STAGE_TYPE, WAKE_STAGE_ORDER, WakeCheckStage::default)?;
    registry.register_stage(
        WHITELIST_STAGE_TYPE,
        WHITELIST_STAGE_ORDER,
        WhitelistCheckStage::default,
    )?;
    registry.register_stage(
        SESSION_STATUS_STAGE_TYPE,
        SESSION_STATUS_STAGE_ORDER,
        SessionStatusCheckStage::default,
    )?;
    registry.register_stage(
        RATE_LIMIT_STAGE_TYPE,
        RATE_LIMIT_STAGE_ORDER,
        RateLimitStage::default,
    )?;
    registry.register_stage(
        CONTENT_SAFETY_STAGE_TYPE,
        CONTENT_SAFETY_STAGE_ORDER,
        ContentSafetyCheckStage::default,
    )?;
    registry.register_stage(
        PROCESS_STAGE_TYPE,
        PROCESS_STAGE_ORDER,
        ProcessStage::default,
    )?;
    registry.register_stage(
        RESULT_DECORATE_STAGE_TYPE,
        RESULT_DECORATE_STAGE_ORDER,
        ResultDecorateStage::default,
    )?;
    registry.register_stage(
        RESPOND_STAGE_TYPE,
        RESPOND_STAGE_ORDER,
        RespondStage::default,
    )?;
    Ok(())
}
