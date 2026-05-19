use std::sync::Arc;

use astrbot_core::{EventExecutor, MessageEvent, Result};
use async_trait::async_trait;

use crate::{
    PipelineContext, PipelineControl, PipelineStage, PipelineStageRegistry, RESPOND_STAGE_TYPE,
    RESULT_DECORATE_STAGE_TYPE,
};

pub struct PipelineScheduler {
    ctx: PipelineContext,
    stages: Vec<Arc<dyn PipelineStage>>,
}

impl PipelineScheduler {
    pub fn new(ctx: PipelineContext) -> Self {
        Self {
            ctx,
            stages: Vec::new(),
        }
    }

    pub fn with_stage(mut self, stage: impl PipelineStage + 'static) -> Self {
        self.stages.push(Arc::new(stage));
        self
    }

    pub fn from_registry(ctx: PipelineContext, registry: &PipelineStageRegistry) -> Self {
        Self {
            ctx,
            stages: registry.build_ordered_stages(),
        }
    }

    pub fn initialize(&self) -> Result<()> {
        for stage in &self.stages {
            stage.initialize(&self.ctx)?;
        }
        Ok(())
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn stage_names(&self) -> Vec<String> {
        self.stages
            .iter()
            .map(|stage| stage.name().to_string())
            .collect()
    }
}

#[async_trait]
impl EventExecutor for PipelineScheduler {
    async fn execute(&self, mut event: MessageEvent) -> Result<()> {
        for stage in &self.stages {
            let control = stage.handle(&mut event, &self.ctx).await?;
            if control == PipelineControl::Stop {
                break;
            }
            if event.is_stopped() && !is_result_delivery_stage(stage.name()) {
                break;
            }
        }
        Ok(())
    }
}

fn is_result_delivery_stage(stage_name: &str) -> bool {
    matches!(stage_name, RESULT_DECORATE_STAGE_TYPE | RESPOND_STAGE_TYPE)
}
