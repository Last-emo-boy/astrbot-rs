use crate::{
    CONTENT_SAFETY_STAGE_TYPE, PREPROCESS_STAGE_TYPE, PROCESS_STAGE_TYPE, PipelineContext,
    PipelineStageRegistry, RATE_LIMIT_STAGE_TYPE, RESPOND_STAGE_TYPE, RESULT_DECORATE_STAGE_TYPE,
    SESSION_STATUS_STAGE_TYPE, WAKE_STAGE_TYPE, WHITELIST_STAGE_TYPE,
};

#[test]
fn builtin_registry_matches_current_pipeline_order() {
    let registry = PipelineStageRegistry::with_builtin_stages().expect("builtins should register");

    assert_eq!(
        registry.ordered_stage_types(),
        vec![
            WAKE_STAGE_TYPE.to_string(),
            PREPROCESS_STAGE_TYPE.to_string(),
            WHITELIST_STAGE_TYPE.to_string(),
            SESSION_STATUS_STAGE_TYPE.to_string(),
            RATE_LIMIT_STAGE_TYPE.to_string(),
            CONTENT_SAFETY_STAGE_TYPE.to_string(),
            PROCESS_STAGE_TYPE.to_string(),
            RESULT_DECORATE_STAGE_TYPE.to_string(),
            RESPOND_STAGE_TYPE.to_string(),
        ]
    );

    let scheduler = registry.build_scheduler(PipelineContext::new());
    assert_eq!(scheduler.stage_count(), 9);
    assert_eq!(
        scheduler.stage_names(),
        vec![
            WAKE_STAGE_TYPE.to_string(),
            PREPROCESS_STAGE_TYPE.to_string(),
            WHITELIST_STAGE_TYPE.to_string(),
            SESSION_STATUS_STAGE_TYPE.to_string(),
            RATE_LIMIT_STAGE_TYPE.to_string(),
            CONTENT_SAFETY_STAGE_TYPE.to_string(),
            PROCESS_STAGE_TYPE.to_string(),
            RESULT_DECORATE_STAGE_TYPE.to_string(),
            RESPOND_STAGE_TYPE.to_string(),
        ]
    );
}
