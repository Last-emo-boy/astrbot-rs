use std::fmt;

use serde::{Deserialize, Serialize};

use super::{McpError, McpResult};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpServerName(String);

impl McpServerName {
    pub fn new(value: impl Into<String>) -> McpResult<Self> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(McpError::InvalidConfig(
                "server name cannot be empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for McpServerName {
    type Error = McpError;

    fn try_from(value: String) -> McpResult<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for McpServerName {
    type Error = McpError;

    fn try_from(value: &str) -> McpResult<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpUri(String);

impl McpUri {
    pub fn new(value: impl Into<String>) -> McpResult<Self> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(McpError::InvalidConfig("uri cannot be empty".to_string()));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for McpUri {
    type Error = McpError;

    fn try_from(value: String) -> McpResult<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for McpUri {
    type Error = McpError;

    fn try_from(value: &str) -> McpResult<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpMimeType(String);

impl McpMimeType {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_string();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{McpMimeType, McpServerName, McpUri};

    #[test]
    fn typed_names_trim_and_reject_empty_values() {
        let name = McpServerName::new(" docs ").expect("server name");
        let uri = McpUri::new(" file:///tmp ").expect("uri");

        assert_eq!(name.as_str(), "docs");
        assert_eq!(uri.as_str(), "file:///tmp");
        assert!(McpServerName::new(" ").is_err());
        assert!(McpUri::new("").is_err());
        assert_eq!(
            McpMimeType::new(" text/plain ")
                .expect("mime type")
                .as_str(),
            "text/plain"
        );
    }
}
