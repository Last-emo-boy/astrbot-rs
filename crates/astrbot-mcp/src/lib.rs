pub mod bridge;
pub mod client;
pub mod config;
pub mod elicitation;
pub mod prompts;
pub mod resources;
pub mod roots;
pub mod sampling;
pub mod tools;
pub mod transport;
pub mod types;

pub use bridge::{McpBridgeCatalogBuilder, McpBridgeRegistration};
pub use client::{
    McpClientBoundary, McpClientLifecycle, McpClientSnapshot, McpClientState, McpConnectionReport,
    McpReconnectPolicy,
};
pub use config::{
    McpClientCapabilities, McpConfig, McpElicitationCapabilityConfig, McpSamplingCapabilityConfig,
    McpServerConfig, McpServerConfigs, McpTransport,
};
pub use elicitation::{
    McpElicitationAction, McpElicitationField, McpElicitationFieldType, McpElicitationRequest,
    McpElicitationResult, McpElicitationSchema, McpElicitationValue,
};
pub use prompts::{
    McpGetPromptRequest, McpGetPromptResult, McpPrompt, McpPromptArgument, McpPromptMessage,
    build_mcp_prompt_tool_names,
};
pub use resources::{
    McpReadResourceRequest, McpReadResourceResult, McpResource, McpResourceContent,
    McpResourceTemplate, build_mcp_resource_tool_names,
};
pub use roots::{McpRoot, McpRootAlias, McpRootsCapabilityConfig, McpRootsRequest};
pub use sampling::{
    McpModelHint, McpSamplingMessage, McpSamplingRequest, McpSamplingResult, McpSamplingRole,
};
pub use tools::{
    McpAnnotations, McpContentBlock, McpEmbeddedResource, McpTool, McpToolArguments,
    McpToolCallRequest, McpToolCallResult, McpToolResultStatus,
};
pub use transport::{
    McpJsonRpcFrame, McpProcessCommand, McpProcessState, McpProcessSupervisorPlan,
    McpReconnectDecision, McpStdoutJsonRpcParser, McpStdoutParseReport, McpTransportEndpoint,
    McpTransportRuntime, McpTransportSession, mcp_reconnect_decision,
};
pub use types::{
    McpCursor, McpError, McpJsonObject, McpJsonSchema, McpJsonValue, McpListPage, McpMimeType,
    McpResult, McpServerName, McpUri,
};

#[cfg(test)]
mod tests;
