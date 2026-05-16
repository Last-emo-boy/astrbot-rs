use astrbot_core::Result;

use crate::{PipelineContext, PipelineScheduler, PipelineStageRegistry};

pub struct DefaultPipelineBuilder {
    registry: PipelineStageRegistry,
}

impl DefaultPipelineBuilder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            registry: PipelineStageRegistry::with_builtin_stages()?,
        })
    }

    pub fn from_registry(registry: PipelineStageRegistry) -> Self {
        Self { registry }
    }

    pub fn stage_types(&self) -> Vec<String> {
        self.registry.ordered_stage_types()
    }

    pub fn build(&self, context: PipelineContext) -> Result<PipelineScheduler> {
        let scheduler = self.registry.build_scheduler(context);
        scheduler.initialize()?;
        Ok(scheduler)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use astrbot_core::{MessageEvent, Result};
    use async_trait::async_trait;

    use super::*;
    use crate::{
        CONTENT_SAFETY_STAGE_TYPE, PROCESS_STAGE_TYPE, PipelineControl, PipelineStage,
        PipelineStageRegistry, RATE_LIMIT_STAGE_TYPE, RESPOND_STAGE_TYPE,
        RESULT_DECORATE_STAGE_TYPE, SESSION_STATUS_STAGE_TYPE, WAKE_STAGE_TYPE,
        WHITELIST_STAGE_TYPE,
    };

    struct InitRecordingStage {
        name: &'static str,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl PipelineStage for InitRecordingStage {
        fn name(&self) -> &str {
            self.name
        }

        fn initialize(&self, _ctx: &PipelineContext) -> Result<()> {
            self.calls
                .lock()
                .expect("init calls should lock")
                .push(self.name);
            Ok(())
        }

        async fn handle(
            &self,
            _event: &mut MessageEvent,
            _ctx: &PipelineContext,
        ) -> Result<PipelineControl> {
            Ok(PipelineControl::Continue)
        }
    }

    #[test]
    fn default_builder_uses_builtin_stage_order() {
        let builder = DefaultPipelineBuilder::new().expect("builder should create");

        assert_eq!(
            builder.stage_types(),
            vec![
                WAKE_STAGE_TYPE.to_string(),
                WHITELIST_STAGE_TYPE.to_string(),
                SESSION_STATUS_STAGE_TYPE.to_string(),
                RATE_LIMIT_STAGE_TYPE.to_string(),
                CONTENT_SAFETY_STAGE_TYPE.to_string(),
                PROCESS_STAGE_TYPE.to_string(),
                RESULT_DECORATE_STAGE_TYPE.to_string(),
                RESPOND_STAGE_TYPE.to_string(),
            ]
        );
        let scheduler = builder
            .build(PipelineContext::new())
            .expect("scheduler should build");
        assert_eq!(
            scheduler.stage_names(),
            vec![
                WAKE_STAGE_TYPE.to_string(),
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

    #[test]
    fn default_builder_initializes_scheduler_before_returning() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let early_calls = calls.clone();
        let late_calls = calls.clone();
        let mut registry = PipelineStageRegistry::new();
        registry
            .register_stage("late", 20, move || InitRecordingStage {
                name: "late",
                calls: late_calls.clone(),
            })
            .expect("late registration should work");
        registry
            .register_stage("early", 10, move || InitRecordingStage {
                name: "early",
                calls: early_calls.clone(),
            })
            .expect("early registration should work");

        let builder = DefaultPipelineBuilder::from_registry(registry);
        let scheduler = builder
            .build(PipelineContext::new())
            .expect("scheduler should build");

        assert_eq!(
            *calls.lock().expect("init calls should lock"),
            vec!["early", "late"]
        );
        assert_eq!(
            scheduler.stage_names(),
            vec!["early".to_string(), "late".to_string()]
        );
    }
}
