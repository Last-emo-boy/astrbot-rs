use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpJsonRpcId {
    Number(i64),
    String(String),
}

impl McpJsonRpcId {
    pub fn as_json(self) -> serde_json::Value {
        match self {
            Self::Number(value) => serde_json::Value::Number(value.into()),
            Self::String(value) => serde_json::Value::String(value),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpJsonRpcErrorObject {
    pub code: i64,
    pub message: String,
}

impl McpJsonRpcErrorObject {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub const MCP_JSONRPC_VERSION: &str = "2.0";
pub const MCP_JSONRPC_PARSE_ERROR: i64 = -32700;
pub const MCP_JSONRPC_INVALID_REQUEST: i64 = -32600;
pub const MCP_JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
pub const MCP_JSONRPC_INTERNAL_ERROR: i64 = -32603;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{MCP_JSONRPC_VERSION, McpJsonRpcErrorObject, McpJsonRpcId};

    #[test]
    fn protocol_primitives_stay_transport_independent() {
        assert_eq!(MCP_JSONRPC_VERSION, "2.0");
        assert_eq!(McpJsonRpcId::Number(7).as_json(), json!(7));
        assert_eq!(
            McpJsonRpcId::String("abc".to_string()).as_json(),
            json!("abc")
        );

        let error = McpJsonRpcErrorObject::new(-32601, "method not found");
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "method not found");
    }
}
