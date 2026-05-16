mod config;
mod orchestrator;
mod tool_bridge;

pub use config::{ResolvedSubagent, SubagentConfig, SubagentConfigSource, SubagentPersonaProfile};
pub use orchestrator::{
    HandoffRegistration, HandoffToolSpec, InMemoryHandoffRegistry, StaticSubagentResolver,
    SubagentOrchestrator, SubagentResolver,
};
pub use tool_bridge::HandoffToolBridge;
