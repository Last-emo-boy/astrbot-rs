use std::sync::Arc;

use super::{CompositeProviderRequestDecorator, ProviderRequestDecorator};

#[derive(Default)]
pub struct AgentRequestDecoratorComposer {
    decorators: Vec<Arc<dyn ProviderRequestDecorator>>,
}

impl AgentRequestDecoratorComposer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decorator(mut self, decorator: Arc<dyn ProviderRequestDecorator>) -> Self {
        self.decorators.push(decorator);
        self
    }

    pub fn build(self) -> Arc<dyn ProviderRequestDecorator> {
        let mut composite = CompositeProviderRequestDecorator::new();
        for decorator in self.decorators {
            composite = composite.with_decorator(decorator);
        }
        Arc::new(composite)
    }
}
