use thiserror::Error;

use super::{
    MCP_JSONRPC_INTERNAL_ERROR, MCP_JSONRPC_INVALID_REQUEST, MCP_JSONRPC_METHOD_NOT_FOUND,
    MCP_JSONRPC_PARSE_ERROR, McpJsonRpcErrorObject,
};

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

impl McpError {
    pub fn to_json_rpc_error(&self) -> McpJsonRpcErrorObject {
        match self {
            Self::InvalidConfig(message) | Self::Unsupported(message) => {
                McpJsonRpcErrorObject::new(MCP_JSONRPC_INVALID_REQUEST, message)
            }
            Self::NotConnected(message) => {
                McpJsonRpcErrorObject::new(MCP_JSONRPC_METHOD_NOT_FOUND, message)
            }
            Self::Transport(message) => {
                McpJsonRpcErrorObject::new(MCP_JSONRPC_INTERNAL_ERROR, message)
            }
            Self::Protocol(message) => McpJsonRpcErrorObject::new(MCP_JSONRPC_PARSE_ERROR, message),
        }
    }
}
