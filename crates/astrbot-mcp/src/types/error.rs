use thiserror::Error;

pub type McpResult<T> = std::result::Result<T, McpError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum McpError {
    #[error("invalid MCP config: {0}")]
    InvalidConfig(String),

    #[error("MCP client is not connected: {0}")]
    NotConnected(String),

    #[error("unsupported MCP request: {0}")]
    Unsupported(String),

    #[error("MCP transport error: {0}")]
    Transport(String),

    #[error("MCP protocol error: {0}")]
    Protocol(String),
}
