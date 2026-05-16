use std::sync::Arc;

use crate::{MessageEvent, Result};

use super::EventExecutor;

pub trait EventRouter: Send + Sync {
    fn route(&self, event: &MessageEvent) -> Result<Arc<dyn EventExecutor>>;
}

#[derive(Clone)]
pub struct SingleExecutorRouter {
    executor: Arc<dyn EventExecutor>,
}

impl SingleExecutorRouter {
    pub fn new(executor: Arc<dyn EventExecutor>) -> Self {
        Self { executor }
    }
}

impl EventRouter for SingleExecutorRouter {
    fn route(&self, _event: &MessageEvent) -> Result<Arc<dyn EventExecutor>> {
        Ok(self.executor.clone())
    }
}
