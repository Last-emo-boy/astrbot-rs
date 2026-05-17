use crate::{
    McpClientCapabilities, McpElicitationCapabilityConfig, McpRootsCapabilityConfig,
    McpSamplingCapabilityConfig, McpServerConfig, McpTransport, config,
};

#[test]
fn server_config_normalizes_http_transport_and_capabilities() {
    let config = McpServerConfig::default()
        .with_client_capabilities(McpClientCapabilities {
            elicitation: McpElicitationCapabilityConfig {
                enabled: true,
                timeout_seconds: 0,
            },
            sampling: McpSamplingCapabilityConfig { enabled: true },
            roots: McpRootsCapabilityConfig {
                enabled: true,
                paths: vec!["data".to_string(), "".to_string(), "temp".to_string()],
            },
        })
        .with_arg("--serve");

    let normalized = McpServerConfig {
        url: Some(" https://example.invalid/mcp ".to_string()),
        ..config
    }
    .normalize();

    assert_eq!(normalized.transport, McpTransport::Sse);
    assert_eq!(
        normalized.client_capabilities.elicitation.timeout_seconds,
        config::DEFAULT_MCP_ELICITATION_TIMEOUT_SECONDS
    );
    assert_eq!(
        normalized.client_capabilities.roots.paths,
        vec!["data".to_string(), "temp".to_string()]
    );
    assert!(
        normalized
            .client_capabilities
            .supports_interactive_requests()
    );
}
