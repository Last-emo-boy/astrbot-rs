use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_SUPPORTED_REFERENCE_TOOLS: &[&str] = &["web_search_tavily", "web_search_bocha"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReferenceItem {
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

impl ToolReferenceItem {
    pub fn new(index: impl Into<String>) -> Self {
        Self {
            index: index.into(),
            url: None,
            title: None,
            snippet: None,
            favicon: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        let url = url.into();
        self.url = (!url.trim().is_empty()).then_some(url);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.title = (!title.trim().is_empty()).then_some(title);
        self
    }

    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        let snippet = snippet.into();
        self.snippet = (!snippet.trim().is_empty()).then_some(snippet);
        self
    }

    pub fn with_favicon(mut self, favicon: impl Into<String>) -> Self {
        let favicon = favicon.into();
        self.favicon = (!favicon.trim().is_empty()).then_some(favicon);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReferenceSet {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used: Vec<ToolReferenceItem>,
}

impl ToolReferenceSet {
    pub fn new(used: Vec<ToolReferenceItem>) -> Self {
        Self { used }
    }

    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallReferencePayload {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub result: String,
}

impl ToolCallReferencePayload {
    pub fn new(name: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: None,
            result: result.into(),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        let id = id.into();
        self.id = (!id.trim().is_empty()).then_some(id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReferenceSource {
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

impl ToolReferenceSource {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            call_id: None,
        }
    }

    pub fn with_call_id(mut self, call_id: impl Into<String>) -> Self {
        let call_id = call_id.into();
        self.call_id = (!call_id.trim().is_empty()).then_some(call_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolReferenceExtractor {
    supported_tools: BTreeSet<String>,
    favicons_by_url: BTreeMap<String, String>,
}

impl Default for ToolReferenceExtractor {
    fn default() -> Self {
        Self::new(DEFAULT_SUPPORTED_REFERENCE_TOOLS.iter().copied())
    }
}

impl ToolReferenceExtractor {
    pub fn new(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            supported_tools: tools
                .into_iter()
                .map(Into::into)
                .filter(|tool| !tool.trim().is_empty())
                .collect(),
            favicons_by_url: BTreeMap::new(),
        }
    }

    pub fn with_favicon(mut self, url: impl Into<String>, favicon: impl Into<String>) -> Self {
        let url = url.into();
        let favicon = favicon.into();
        if !url.trim().is_empty() && !favicon.trim().is_empty() {
            self.favicons_by_url.insert(url, favicon);
        }
        self
    }

    pub fn with_favicons(
        mut self,
        favicons_by_url: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (url, favicon) in favicons_by_url {
            self = self.with_favicon(url, favicon);
        }
        self
    }

    pub fn supports(&self, tool_name: &str) -> bool {
        self.supported_tools.contains(tool_name)
    }

    pub fn extract_from_tool_calls(
        &self,
        response_text: &str,
        tool_calls: &[ToolCallReferencePayload],
    ) -> ToolReferenceSet {
        let candidates = self.reference_candidates(tool_calls);
        if candidates.is_empty() {
            return ToolReferenceSet::default();
        }

        let used = extract_ref_indices(response_text)
            .into_iter()
            .filter_map(|index| candidates.get(&index).cloned())
            .collect();
        ToolReferenceSet::new(used)
    }

    fn reference_candidates(
        &self,
        tool_calls: &[ToolCallReferencePayload],
    ) -> BTreeMap<String, ToolReferenceItem> {
        let mut references = BTreeMap::new();
        for call in tool_calls {
            if !self.supports(&call.name) || call.result.trim().is_empty() {
                continue;
            }

            let Ok(result) = serde_json::from_str::<Value>(&call.result) else {
                continue;
            };
            let Some(results) = result.get("results").and_then(Value::as_array) else {
                continue;
            };

            for item in results {
                let Some(index) = string_field(item, "index") else {
                    continue;
                };
                let mut reference = ToolReferenceItem::new(index.clone());
                if let Some(url) = string_field(item, "url") {
                    reference = reference.with_url(url.clone());
                    if let Some(favicon) = self.favicons_by_url.get(&url) {
                        reference = reference.with_favicon(favicon.clone());
                    }
                }
                if let Some(title) = string_field(item, "title") {
                    reference = reference.with_title(title);
                }
                if let Some(snippet) = string_field(item, "snippet") {
                    reference = reference.with_snippet(snippet);
                }
                references.insert(index, reference);
            }
        }
        references
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_ref_indices(text: &str) -> Vec<String> {
    let mut indices = Vec::new();
    let mut seen = BTreeSet::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<ref>") {
        let after_start = &remaining[start + "<ref>".len()..];
        let Some(end) = after_start.find("</ref>") else {
            break;
        };
        let index = after_start[..end].trim();
        if !index.is_empty() && seen.insert(index.to_string()) {
            indices.push(index.to_string());
        }
        remaining = &after_start[end + "</ref>".len()..];
    }

    indices
}
