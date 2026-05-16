use std::sync::Arc;

use crate::PipelineStage;

pub(super) type StageFactory = Arc<dyn Fn() -> Arc<dyn PipelineStage> + Send + Sync>;

pub(super) struct RegisteredStage {
    pub(super) stage_type: String,
    pub(super) order: i32,
    pub(super) factory: StageFactory,
}

impl RegisteredStage {
    pub(super) fn new<F, S>(stage_type: String, order: i32, factory: F) -> Self
    where
        F: Fn() -> S + Send + Sync + 'static,
        S: PipelineStage + 'static,
    {
        let factory = Arc::new(move || Arc::new(factory()) as Arc<dyn PipelineStage>);
        Self {
            stage_type,
            order,
            factory,
        }
    }
}
