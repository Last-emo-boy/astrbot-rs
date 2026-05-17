use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};

use crate::{PipelineContext, PipelineScheduler, PipelineStage};

mod builtins;
mod entry;
mod order;

use entry::RegisteredStage;

pub use order::{
    CONTENT_SAFETY_STAGE_ORDER, CONTENT_SAFETY_STAGE_TYPE, PLUGIN_STAGE_ORDER, PLUGIN_STAGE_TYPE,
    PREPROCESS_STAGE_ORDER, PREPROCESS_STAGE_TYPE, PROCESS_STAGE_ORDER, PROCESS_STAGE_TYPE,
    PROVIDER_STAGE_ORDER, PROVIDER_STAGE_TYPE, RATE_LIMIT_STAGE_ORDER, RATE_LIMIT_STAGE_TYPE,
    RESPOND_STAGE_ORDER, RESPOND_STAGE_TYPE, RESULT_DECORATE_STAGE_ORDER,
    RESULT_DECORATE_STAGE_TYPE, SESSION_STATUS_STAGE_ORDER, SESSION_STATUS_STAGE_TYPE,
    WAKE_STAGE_ORDER, WAKE_STAGE_TYPE, WHITELIST_STAGE_ORDER, WHITELIST_STAGE_TYPE,
};

#[derive(Default)]
pub struct PipelineStageRegistry {
    stages: HashMap<String, RegisteredStage>,
}

impl PipelineStageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_stages() -> Result<Self> {
        let mut registry = Self::new();
        builtins::register_builtin_stages(&mut registry)?;
        Ok(registry)
    }

    pub fn register_stage<F, S>(
        &mut self,
        stage_type: impl Into<String>,
        order: i32,
        factory: F,
    ) -> Result<()>
    where
        F: Fn() -> S + Send + Sync + 'static,
        S: PipelineStage + 'static,
    {
        let stage_type = stage_type.into();
        let stage_type = stage_type.trim();
        if stage_type.is_empty() {
            return Err(AstrbotError::Pipeline(
                "pipeline stage type must not be empty".to_string(),
            ));
        }

        if self.stages.contains_key(stage_type) {
            return Err(AstrbotError::Pipeline(format!(
                "pipeline stage type {stage_type} is already registered"
            )));
        }

        let stage_type = stage_type.to_string();
        self.stages.insert(
            stage_type.clone(),
            RegisteredStage::new(stage_type, order, factory),
        );
        Ok(())
    }

    pub fn has_stage(&self, stage_type: &str) -> bool {
        self.stages.contains_key(stage_type)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn ordered_stage_types(&self) -> Vec<String> {
        self.ordered_stages()
            .into_iter()
            .map(|stage| stage.stage_type.clone())
            .collect()
    }

    pub fn build_scheduler(&self, context: PipelineContext) -> PipelineScheduler {
        PipelineScheduler::from_registry(context, self)
    }

    pub(crate) fn build_ordered_stages(&self) -> Vec<Arc<dyn PipelineStage>> {
        self.ordered_stages()
            .into_iter()
            .map(|stage| (stage.factory)())
            .collect()
    }

    fn ordered_stages(&self) -> Vec<&RegisteredStage> {
        let mut stages = self.stages.values().collect::<Vec<_>>();
        stages.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.stage_type.cmp(&right.stage_type))
        });
        stages
    }
}

#[cfg(test)]
mod tests;
