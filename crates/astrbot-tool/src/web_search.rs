use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ToolCatalog, ToolDescriptor, ToolSourceMetadata};

pub const WEB_SEARCH_TOOL: &str = "web_search";
pub const FETCH_URL_TOOL: &str = "fetch_url";
pub const WEB_SEARCH_TAVILY_TOOL: &str = "web_search_tavily";
pub const TAVILY_EXTRACT_TOOL: &str = "tavily_extract_web_page";
pub const WEB_SEARCH_BOCHA_TOOL: &str = "web_search_bocha";
pub const BAIDU_AI_SEARCH_TOOL: &str = "AIsearch";

pub const WEB_SEARCH_TOOL_NAMES: &[&str] = &[
    WEB_SEARCH_TOOL,
    FETCH_URL_TOOL,
    WEB_SEARCH_TAVILY_TOOL,
    TAVILY_EXTRACT_TOOL,
    WEB_SEARCH_BOCHA_TOOL,
    BAIDU_AI_SEARCH_TOOL,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchProvider {
    #[default]
    Default,
    Tavily,
    Bocha,
    BaiduAiSearch,
}

impl WebSearchProvider {
    pub fn from_provider_name(provider: &str) -> Self {
        match provider.trim() {
            "tavily" => Self::Tavily,
            "bocha" => Self::Bocha,
            "baidu_ai_search" => Self::BaiduAiSearch,
            _ => Self::Default,
        }
    }

    pub fn as_provider_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Tavily => "tavily",
            Self::Bocha => "bocha",
            Self::BaiduAiSearch => "baidu_ai_search",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchSessionConfig {
    pub enabled: bool,
    #[serde(default)]
    pub provider: WebSearchProvider,
    #[serde(default)]
    pub web_search_link: bool,
    #[serde(default)]
    pub tavily_keys: Vec<String>,
    #[serde(default)]
    pub bocha_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baidu_app_builder_key: Option<String>,
}

impl WebSearchSessionConfig {
    pub fn enabled(provider: WebSearchProvider) -> Self {
        Self {
            enabled: true,
            provider,
            ..Self::default()
        }
    }

    pub fn from_provider_settings(settings: &Value) -> Self {
        Self {
            enabled: bool_field(settings, "web_search"),
            provider: string_field(settings, "websearch_provider")
                .map(|provider| WebSearchProvider::from_provider_name(&provider))
                .unwrap_or_default(),
            web_search_link: bool_field(settings, "web_search_link"),
            tavily_keys: key_list_field(settings, "websearch_tavily_key"),
            bocha_keys: key_list_field(settings, "websearch_bocha_key"),
            baidu_app_builder_key: string_field(settings, "websearch_baidu_app_builder_key"),
        }
    }

    pub fn with_web_search_link(mut self, web_search_link: bool) -> Self {
        self.web_search_link = web_search_link;
        self
    }

    pub fn with_tavily_keys<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tavily_keys = normalize_keys(keys);
        self
    }

    pub fn with_bocha_keys<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bocha_keys = normalize_keys(keys);
        self
    }

    pub fn with_baidu_app_builder_key(mut self, key: impl Into<String>) -> Self {
        self.baidu_app_builder_key = non_empty(key.into());
        self
    }

    pub fn baidu_ai_search_mcp_server(&self) -> Option<BaiduAiSearchMcpServerConfig> {
        self.baidu_app_builder_key
            .as_deref()
            .map(BaiduAiSearchMcpServerConfig::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaiduAiSearchMcpServerConfig {
    pub name: String,
    pub transport: String,
    pub url: String,
    pub timeout_seconds: u64,
}

impl BaiduAiSearchMcpServerConfig {
    pub fn new(api_key: impl AsRef<str>) -> Self {
        let api_key = api_key.as_ref();
        Self {
            name: "baidu_ai_search".to_string(),
            transport: "sse".to_string(),
            url: format!("http://appbuilder.baidu.com/v2/ai_search/mcp/sse?api_key={api_key}"),
            timeout_seconds: 600,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebSearchToolSelection {
    selected_tool_names: BTreeSet<String>,
    baidu_mcp_server: Option<BaiduAiSearchMcpServerConfig>,
}

impl WebSearchToolSelection {
    pub fn from_config(config: &WebSearchSessionConfig) -> Self {
        let mut selected = BTreeSet::new();
        let mut baidu_mcp_server = None;

        if config.enabled {
            match config.provider {
                WebSearchProvider::Default => {
                    selected.insert(WEB_SEARCH_TOOL.to_string());
                    selected.insert(FETCH_URL_TOOL.to_string());
                }
                WebSearchProvider::Tavily => {
                    selected.insert(WEB_SEARCH_TAVILY_TOOL.to_string());
                    selected.insert(TAVILY_EXTRACT_TOOL.to_string());
                }
                WebSearchProvider::Bocha => {
                    selected.insert(WEB_SEARCH_BOCHA_TOOL.to_string());
                }
                WebSearchProvider::BaiduAiSearch => {
                    if let Some(config) = config.baidu_ai_search_mcp_server() {
                        selected.insert(BAIDU_AI_SEARCH_TOOL.to_string());
                        baidu_mcp_server = Some(config);
                    }
                }
            }
        }

        Self {
            selected_tool_names: selected,
            baidu_mcp_server,
        }
    }

    pub fn selected_tool_names(&self) -> Vec<&str> {
        self.selected_tool_names
            .iter()
            .map(String::as_str)
            .collect()
    }

    pub fn baidu_mcp_server(&self) -> Option<&BaiduAiSearchMcpServerConfig> {
        self.baidu_mcp_server.as_ref()
    }

    pub fn selects(&self, tool_name: &str) -> bool {
        self.selected_tool_names.contains(tool_name)
    }

    pub fn apply_to_catalog(&self, catalog: &ToolCatalog) -> ToolCatalog {
        let mut filtered = ToolCatalog::new();
        for tool in catalog.tools() {
            if is_web_search_tool_name(&tool.name) && !self.selects(&tool.name) {
                continue;
            }
            filtered.add_tool(tool.clone());
        }
        if self.selects(BAIDU_AI_SEARCH_TOOL) && filtered.tool(BAIDU_AI_SEARCH_TOOL).is_none() {
            filtered.add_tool(
                ToolDescriptor::new(BAIDU_AI_SEARCH_TOOL)
                    .with_description("Search the web through the Baidu AI Search MCP server.")
                    .with_source_metadata(ToolSourceMetadata::mcp("baidu_ai_search")),
            );
        }
        filtered
    }
}

pub fn is_web_search_tool_name(tool_name: &str) -> bool {
    WEB_SEARCH_TOOL_NAMES.contains(&tool_name)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
}

impl WebSearchResult {
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        snippet: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            snippet: snippet.into(),
            favicon: None,
        }
    }

    pub fn with_favicon(mut self, favicon: impl Into<String>) -> Self {
        self.favicon = non_empty(favicon.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebExtractedPage {
    pub url: String,
    pub raw_content: String,
}

impl WebExtractedPage {
    pub fn new(url: impl Into<String>, raw_content: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            raw_content: raw_content.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TavilySearchRequest {
    pub query: String,
    pub max_results: usize,
    pub search_depth: String,
    pub topic: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

impl TavilySearchRequest {
    pub fn new(query: impl Into<String>, max_results: usize) -> Self {
        Self {
            query: query.into(),
            max_results,
            search_depth: "basic".to_string(),
            topic: "general".to_string(),
            days: None,
            time_range: None,
            start_date: None,
            end_date: None,
        }
    }

    pub fn into_payload(self) -> Value {
        let mut payload = json!({
            "query": self.query,
            "max_results": self.max_results,
            "include_favicon": true,
            "search_depth": self.search_depth,
            "topic": self.topic
        });
        if let Value::Object(map) = &mut payload {
            if let Some(days) = self.days {
                map.insert("days".to_string(), json!(days));
            }
            if let Some(time_range) = self.time_range {
                map.insert("time_range".to_string(), json!(time_range));
            }
            if let Some(start_date) = self.start_date {
                map.insert("start_date".to_string(), json!(start_date));
            }
            if let Some(end_date) = self.end_date {
                map.insert("end_date".to_string(), json!(end_date));
            }
        }
        payload
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TavilyExtractRequest {
    pub url: String,
    pub extract_depth: String,
}

impl TavilyExtractRequest {
    pub fn new(url: impl Into<String>, extract_depth: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            extract_depth: extract_depth.into(),
        }
    }

    pub fn into_payload(self) -> Value {
        json!({
            "urls": [self.url],
            "extract_depth": self.extract_depth
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BochaSearchRequest {
    pub query: String,
    pub count: usize,
    pub freshness: String,
    pub summary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<String>,
}

impl BochaSearchRequest {
    pub fn new(query: impl Into<String>, count: usize) -> Self {
        Self {
            query: query.into(),
            count,
            freshness: "noLimit".to_string(),
            summary: false,
            include: None,
            exclude: None,
        }
    }

    pub fn into_payload(self) -> Value {
        let mut payload = json!({
            "query": self.query,
            "count": self.count,
            "freshness": self.freshness,
            "summary": self.summary
        });
        if let Value::Object(map) = &mut payload {
            if let Some(include) = self.include {
                map.insert("include".to_string(), json!(include));
            }
            if let Some(exclude) = self.exclude {
                map.insert("exclude".to_string(), json!(exclude));
            }
        }
        payload
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchFaviconMetadata {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub favicons_by_url: BTreeMap<String, String>,
}

impl WebSearchFaviconMetadata {
    pub fn insert(&mut self, url: impl Into<String>, favicon: impl Into<String>) {
        let url = url.into();
        let favicon = favicon.into();
        if !url.trim().is_empty() && !favicon.trim().is_empty() {
            self.favicons_by_url.insert(url, favicon);
        }
    }
}

pub fn shape_indexed_web_search_results(
    results: &[WebSearchResult],
    ref_prefix: &str,
) -> (String, WebSearchFaviconMetadata) {
    let mut metadata = WebSearchFaviconMetadata::default();
    let shaped = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            if let Some(favicon) = &result.favicon {
                metadata.insert(result.url.clone(), favicon.clone());
            }
            json!({
                "title": result.title,
                "url": result.url,
                "snippet": result.snippet,
                "index": format!("{}.{}", ref_prefix, index + 1)
            })
        })
        .collect::<Vec<_>>();
    (json!({ "results": shaped }).to_string(), metadata)
}

pub fn web_search_tool_descriptors() -> Vec<ToolDescriptor> {
    WEB_SEARCH_TOOL_NAMES
        .iter()
        .map(|name| ToolDescriptor::new(*name))
        .collect()
}

fn bool_field(settings: &Value, key: &str) -> bool {
    settings.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn string_field(settings: &Value, key: &str) -> Option<String> {
    settings
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn key_list_field(settings: &Value, key: &str) -> Vec<String> {
    match settings.get(key) {
        Some(Value::String(value)) => normalize_keys([value.clone()]),
        Some(Value::Array(values)) => normalize_keys(values.iter().filter_map(Value::as_str)),
        _ => Vec::new(),
    }
}

fn normalize_keys<I, S>(keys: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    keys.into_iter()
        .map(Into::into)
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect()
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}
