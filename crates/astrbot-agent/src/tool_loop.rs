use astrbot_core::ProviderRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolLoopStrategy {
    Disabled,
    ProviderToolCalls,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopPolicy {
    pub strategy: ToolLoopStrategy,
    pub max_steps: usize,
    pub tool_call_timeout_seconds: u64,
    pub schema_mode: String,
}

impl Default for ToolLoopPolicy {
    fn default() -> Self {
        Self {
            strategy: ToolLoopStrategy::Disabled,
            max_steps: 30,
            tool_call_timeout_seconds: 60,
            schema_mode: "full".to_string(),
        }
    }
}

impl ToolLoopPolicy {
    pub fn enabled(mut self) -> Self {
        self.strategy = ToolLoopStrategy::ProviderToolCalls;
        self
    }

    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }

    pub fn with_timeout_seconds(mut self, tool_call_timeout_seconds: u64) -> Self {
        self.tool_call_timeout_seconds = tool_call_timeout_seconds.max(1);
        self
    }

    pub fn with_schema_mode(mut self, schema_mode: impl Into<String>) -> Self {
        let schema_mode = schema_mode.into();
        if !schema_mode.trim().is_empty() {
            self.schema_mode = schema_mode;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolLoopState {
    Idle,
    Running,
    Done,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopStep {
    pub index: usize,
    pub tool_name: Option<String>,
}

impl ToolLoopStep {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            tool_name: None,
        }
    }

    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        let tool_name = tool_name.into();
        self.tool_name = (!tool_name.trim().is_empty()).then_some(tool_name);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopOutcome {
    pub final_request: ProviderRequest,
    pub steps: Vec<ToolLoopStep>,
    pub state: ToolLoopState,
}

impl ToolLoopOutcome {
    pub fn skipped(final_request: ProviderRequest) -> Self {
        Self {
            final_request,
            steps: Vec::new(),
            state: ToolLoopState::Done,
        }
    }
}
