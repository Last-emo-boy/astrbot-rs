use astrbot_tool::{ToolReferenceItem, ToolReferenceSet};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebChatReferenceItem {
    pub index: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
}

impl From<ToolReferenceItem> for WebChatReferenceItem {
    fn from(item: ToolReferenceItem) -> Self {
        Self {
            index: item.index,
            url: item.url,
            title: item.title,
            snippet: item.snippet,
            favicon: item.favicon,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebChatReferenceResponse {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used: Vec<WebChatReferenceItem>,
}

impl From<ToolReferenceSet> for WebChatReferenceResponse {
    fn from(refs: ToolReferenceSet) -> Self {
        Self {
            used: refs.used.into_iter().map(Into::into).collect(),
        }
    }
}
