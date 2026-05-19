use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use astrbot_core::{AstrbotError, Result};
use astrbot_tool::{
    BochaSearchRequest, FETCH_URL_TOOL, TAVILY_EXTRACT_TOOL, TavilyExtractRequest,
    TavilySearchRequest, WEB_SEARCH_BOCHA_TOOL, WEB_SEARCH_TAVILY_TOOL, WEB_SEARCH_TOOL,
    WebExtractedPage, WebSearchFaviconMetadata, WebSearchResult, WebSearchSessionConfig,
    shape_indexed_web_search_results,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::tool_loop::{AgentToolExecutionRequest, AgentToolExecutionResult, AgentToolExecutor};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebSearchToolExecutionMetadata {
    pub favicons: WebSearchFaviconMetadata,
}

#[async_trait]
pub trait WebSearchClient: Send + Sync {
    async fn default_search(&self, query: &str, max_results: usize)
    -> Result<Vec<WebSearchResult>>;

    async fn fetch_url(&self, url: &str) -> Result<String>;

    async fn tavily_search(&self, key: &str, payload: Value) -> Result<Vec<WebSearchResult>>;

    async fn tavily_extract(&self, key: &str, payload: Value) -> Result<Vec<WebExtractedPage>>;

    async fn bocha_search(&self, key: &str, payload: Value) -> Result<Vec<WebSearchResult>>;
}

pub struct ReqwestWebSearchClient {
    client: reqwest::Client,
}

impl ReqwestWebSearchClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("astrbot-rs-web-searcher/0.1")
                .build()
                .map_err(|err| AstrbotError::Provider(format!("web search client: {err}")))?,
        })
    }
}

#[async_trait]
impl WebSearchClient for ReqwestWebSearchClient {
    async fn default_search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<WebSearchResult>> {
        let encoded_query = encode_query(query);
        let mut last_error = None;
        for base_url in ["https://www.bing.com", "https://cn.bing.com"] {
            let url = format!("{base_url}/search?q={encoded_query}");
            match self.client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    let html = response.text().await.map_err(|err| {
                        AstrbotError::Provider(format!("default web search body failed: {err}"))
                    })?;
                    let results = parse_bing_results(&html, max_results);
                    if !results.is_empty() {
                        return Ok(results);
                    }
                }
                Ok(response) => {
                    let status = response.status();
                    let reason = response.text().await.unwrap_or_default();
                    last_error = Some(format!(
                        "default web search failed: {reason}, status: {}",
                        status.as_u16()
                    ));
                }
                Err(err) => {
                    last_error = Some(format!("default web search failed: {err}"));
                }
            }
        }
        Err(AstrbotError::Provider(last_error.unwrap_or_else(|| {
            "default web search returned no results".to_string()
        })))
    }

    async fn fetch_url(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("fetch_url failed: {err}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let reason = response.text().await.unwrap_or_default();
            return Err(AstrbotError::Provider(format!(
                "fetch_url failed: {reason}, status: {}",
                status.as_u16()
            )));
        }
        let html = response
            .text()
            .await
            .map_err(|err| AstrbotError::Provider(format!("fetch_url body failed: {err}")))?;
        Ok(strip_html_text(&html))
    }

    async fn tavily_search(&self, key: &str, payload: Value) -> Result<Vec<WebSearchResult>> {
        let response = self
            .client
            .post("https://api.tavily.com/search")
            .bearer_auth(key)
            .json(&payload)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Tavily web search failed: {err}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let reason = response.text().await.unwrap_or_default();
            return Err(AstrbotError::Provider(format!(
                "Tavily web search failed: {reason}, status: {}",
                status.as_u16()
            )));
        }
        let data = response
            .json::<Value>()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Tavily response JSON failed: {err}")))?;
        Ok(data
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|item| {
                WebSearchResult::new(
                    value_string(item, "title"),
                    value_string(item, "url"),
                    value_string(item, "content"),
                )
                .with_favicon(value_string(item, "favicon"))
            })
            .collect())
    }

    async fn tavily_extract(&self, key: &str, payload: Value) -> Result<Vec<WebExtractedPage>> {
        let response = self
            .client
            .post("https://api.tavily.com/extract")
            .bearer_auth(key)
            .json(&payload)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Tavily web search failed: {err}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let reason = response.text().await.unwrap_or_default();
            return Err(AstrbotError::Provider(format!(
                "Tavily web search failed: {reason}, status: {}",
                status.as_u16()
            )));
        }
        let data = response
            .json::<Value>()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Tavily extract JSON failed: {err}")))?;
        Ok(data
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|item| {
                WebExtractedPage::new(value_string(item, "url"), value_string(item, "raw_content"))
            })
            .collect())
    }

    async fn bocha_search(&self, key: &str, payload: Value) -> Result<Vec<WebSearchResult>> {
        let response = self
            .client
            .post("https://api.bochaai.com/v1/web-search")
            .bearer_auth(key)
            .json(&payload)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("BoCha web search failed: {err}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let reason = response.text().await.unwrap_or_default();
            return Err(AstrbotError::Provider(format!(
                "BoCha web search failed: {reason}, status: {}",
                status.as_u16()
            )));
        }
        let data = response
            .json::<Value>()
            .await
            .map_err(|err| AstrbotError::Provider(format!("BoCha response JSON failed: {err}")))?;
        Ok(data
            .pointer("/data/webPages/value")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|item| {
                WebSearchResult::new(
                    value_string(item, "name"),
                    value_string(item, "url"),
                    value_string(item, "snippet"),
                )
                .with_favicon(value_string(item, "siteIcon"))
            })
            .collect())
    }
}

pub struct WebSearchToolExecutor {
    config: WebSearchSessionConfig,
    client: Arc<dyn WebSearchClient>,
    tavily_keys: Mutex<RotatingKeys>,
    bocha_keys: Mutex<RotatingKeys>,
    ref_counter: Mutex<u64>,
    metadata: Mutex<WebSearchToolExecutionMetadata>,
}

impl WebSearchToolExecutor {
    pub fn new(config: WebSearchSessionConfig, client: Arc<dyn WebSearchClient>) -> Self {
        Self {
            tavily_keys: Mutex::new(RotatingKeys::new(config.tavily_keys.clone())),
            bocha_keys: Mutex::new(RotatingKeys::new(config.bocha_keys.clone())),
            config,
            client,
            ref_counter: Mutex::new(0),
            metadata: Mutex::new(WebSearchToolExecutionMetadata::default()),
        }
    }

    pub fn with_reqwest(config: WebSearchSessionConfig) -> Result<Self> {
        Ok(Self::new(config, Arc::new(ReqwestWebSearchClient::new()?)))
    }

    pub fn metadata(&self) -> WebSearchToolExecutionMetadata {
        self.metadata
            .lock()
            .expect("web search metadata should lock")
            .clone()
    }

    fn next_tavily_key(&self) -> Result<String> {
        self.tavily_keys
            .lock()
            .expect("tavily key ring should lock")
            .next_key("Tavily")
    }

    fn next_bocha_key(&self) -> Result<String> {
        self.bocha_keys
            .lock()
            .expect("bocha key ring should lock")
            .next_key("BoCha")
    }

    fn next_ref_prefix(&self) -> String {
        let mut counter = self.ref_counter.lock().expect("ref counter should lock");
        *counter += 1;
        format!("{:04x}", *counter)
    }

    fn merge_favicons(&self, metadata: WebSearchFaviconMetadata) {
        let mut current = self
            .metadata
            .lock()
            .expect("web search metadata should lock");
        for (url, favicon) in metadata.favicons_by_url {
            current.favicons.insert(url, favicon);
        }
    }
}

#[async_trait]
impl AgentToolExecutor for WebSearchToolExecutor {
    async fn execute(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        match request.descriptor.name.as_str() {
            WEB_SEARCH_TOOL => self.execute_default_search(&request).await,
            FETCH_URL_TOOL => self.execute_fetch_url(&request).await,
            WEB_SEARCH_TAVILY_TOOL => self.execute_tavily_search(&request).await,
            TAVILY_EXTRACT_TOOL => self.execute_tavily_extract(&request).await,
            WEB_SEARCH_BOCHA_TOOL => self.execute_bocha_search(&request).await,
            other => Err(AstrbotError::Pipeline(format!(
                "web search executor cannot handle tool {other}"
            ))),
        }
    }
}

impl WebSearchToolExecutor {
    async fn execute_default_search(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let query = required_string(request, "query")?;
        let max_results = usize_argument(request, "max_results", 5);
        let results = self.client.default_search(&query, max_results).await?;
        if results.is_empty() {
            return Ok(AgentToolExecutionResult::completed(
                "Error: web searcher does not return any results.",
            ));
        }

        let mut text = String::new();
        for (index, result) in results.iter().enumerate() {
            text.push_str(&format!(
                "{}. {} {}\n{}\n\n",
                index + 1,
                result.title,
                if self.config.web_search_link {
                    result.url.as_str()
                } else {
                    ""
                },
                result.snippet
            ));
        }
        if self.config.web_search_link {
            text.push_str("\n\n针对问题，请根据上面的结果分点总结，并且在结尾处附上对应内容的参考链接（如有）。");
        }
        Ok(AgentToolExecutionResult::completed(text))
    }

    async fn execute_fetch_url(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let url = required_string(request, "url")?;
        let text = self.client.fetch_url(&url).await?;
        Ok(AgentToolExecutionResult::completed(text))
    }

    async fn execute_tavily_search(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let key = self.next_tavily_key()?;
        let query = required_string(request, "query")?;
        let mut tavily = TavilySearchRequest::new(query, usize_argument(request, "max_results", 7));
        tavily.search_depth =
            enum_string_argument(request, "search_depth", "basic", &["basic", "advanced"]);
        tavily.topic = enum_string_argument(request, "topic", "general", &["general", "news"]);
        if tavily.topic == "news" {
            tavily.days = Some(u64_argument(request, "days", 3));
        }
        tavily.time_range =
            optional_enum_string_argument(request, "time_range", &["day", "week", "month", "year"]);
        tavily.start_date = optional_string_argument(request, "start_date");
        tavily.end_date = optional_string_argument(request, "end_date");

        let results = self
            .client
            .tavily_search(&key, tavily.into_payload())
            .await?;
        if results.is_empty() {
            return Ok(AgentToolExecutionResult::completed(
                "Error: Tavily web searcher does not return any results.",
            ));
        }
        let (text, metadata) = shape_indexed_web_search_results(&results, &self.next_ref_prefix());
        self.merge_favicons(metadata);
        Ok(AgentToolExecutionResult::completed(text))
    }

    async fn execute_tavily_extract(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let key = self.next_tavily_key()?;
        let url = required_string(request, "url")?;
        let extract_depth =
            enum_string_argument(request, "extract_depth", "basic", &["basic", "advanced"]);
        let pages = self
            .client
            .tavily_extract(
                &key,
                TavilyExtractRequest::new(url, extract_depth).into_payload(),
            )
            .await?;
        if pages.is_empty() {
            return Err(AstrbotError::Provider(
                "Error: Tavily web searcher does not return any results.".to_string(),
            ));
        }
        let text = pages
            .into_iter()
            .map(|page| format!("URL: {}\nContent: {}", page.url, page.raw_content))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(AgentToolExecutionResult::completed(text))
    }

    async fn execute_bocha_search(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let key = self.next_bocha_key()?;
        let query = required_string(request, "query")?;
        let mut bocha = BochaSearchRequest::new(query, usize_argument(request, "count", 10));
        bocha.freshness =
            optional_string_argument(request, "freshness").unwrap_or_else(|| "noLimit".to_string());
        bocha.summary = bool_argument(request, "summary", false);
        bocha.include = optional_string_argument(request, "include");
        bocha.exclude = optional_string_argument(request, "exclude");

        let results = self.client.bocha_search(&key, bocha.into_payload()).await?;
        if results.is_empty() {
            return Ok(AgentToolExecutionResult::completed(
                "Error: BoCha web searcher does not return any results.",
            ));
        }
        let (text, metadata) = shape_indexed_web_search_results(&results, &self.next_ref_prefix());
        self.merge_favicons(metadata);
        Ok(AgentToolExecutionResult::completed(text))
    }
}

#[derive(Clone, Debug, Default)]
struct RotatingKeys {
    keys: Vec<String>,
    index: usize,
}

impl RotatingKeys {
    fn new(keys: Vec<String>) -> Self {
        Self { keys, index: 0 }
    }

    fn next_key(&mut self, provider: &str) -> Result<String> {
        if self.keys.is_empty() {
            return Err(AstrbotError::Provider(format!(
                "Error: {provider} API key is not configured in AstrBot."
            )));
        }
        let key = self.keys[self.index].clone();
        self.index = (self.index + 1) % self.keys.len();
        Ok(key)
    }
}

#[derive(Clone, Debug, Default)]
pub struct FixtureWebSearchClient {
    default_results: Arc<Mutex<Vec<WebSearchResult>>>,
    fetched_urls: Arc<Mutex<BTreeMap<String, String>>>,
    tavily_results: Arc<Mutex<VecDeque<Result<Vec<WebSearchResult>>>>>,
    tavily_extract_results: Arc<Mutex<VecDeque<Result<Vec<WebExtractedPage>>>>>,
    bocha_results: Arc<Mutex<VecDeque<Result<Vec<WebSearchResult>>>>>,
    captured: Arc<Mutex<Vec<FixtureWebSearchRequest>>>,
}

impl FixtureWebSearchClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_results(self, results: Vec<WebSearchResult>) -> Self {
        *self
            .default_results
            .lock()
            .expect("fixture default results should lock") = results;
        self
    }

    pub fn with_fetched_url(self, url: impl Into<String>, text: impl Into<String>) -> Self {
        self.fetched_urls
            .lock()
            .expect("fixture fetched urls should lock")
            .insert(url.into(), text.into());
        self
    }

    pub fn push_tavily_result(&self, result: Result<Vec<WebSearchResult>>) {
        self.tavily_results
            .lock()
            .expect("fixture tavily results should lock")
            .push_back(result);
    }

    pub fn push_tavily_extract_result(&self, result: Result<Vec<WebExtractedPage>>) {
        self.tavily_extract_results
            .lock()
            .expect("fixture tavily extract results should lock")
            .push_back(result);
    }

    pub fn push_bocha_result(&self, result: Result<Vec<WebSearchResult>>) {
        self.bocha_results
            .lock()
            .expect("fixture bocha results should lock")
            .push_back(result);
    }

    pub fn captured(&self) -> Vec<FixtureWebSearchRequest> {
        self.captured
            .lock()
            .expect("fixture captured requests should lock")
            .clone()
    }

    fn capture(&self, request: FixtureWebSearchRequest) {
        self.captured
            .lock()
            .expect("fixture captured requests should lock")
            .push(request);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixtureWebSearchRequest {
    pub provider: String,
    pub key: Option<String>,
    pub payload: Value,
}

#[async_trait]
impl WebSearchClient for FixtureWebSearchClient {
    async fn default_search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<WebSearchResult>> {
        self.capture(FixtureWebSearchRequest {
            provider: "default".to_string(),
            key: None,
            payload: serde_json::json!({"query": query, "max_results": max_results}),
        });
        Ok(self
            .default_results
            .lock()
            .expect("fixture default results should lock")
            .clone())
    }

    async fn fetch_url(&self, url: &str) -> Result<String> {
        self.capture(FixtureWebSearchRequest {
            provider: "fetch_url".to_string(),
            key: None,
            payload: serde_json::json!({"url": url}),
        });
        self.fetched_urls
            .lock()
            .expect("fixture fetched urls should lock")
            .get(url)
            .cloned()
            .ok_or_else(|| AstrbotError::Provider(format!("fetch_url failed for {url}")))
    }

    async fn tavily_search(&self, key: &str, payload: Value) -> Result<Vec<WebSearchResult>> {
        self.capture(FixtureWebSearchRequest {
            provider: "tavily".to_string(),
            key: Some(key.to_string()),
            payload,
        });
        self.tavily_results
            .lock()
            .expect("fixture tavily results should lock")
            .pop_front()
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    async fn tavily_extract(&self, key: &str, payload: Value) -> Result<Vec<WebExtractedPage>> {
        self.capture(FixtureWebSearchRequest {
            provider: "tavily_extract".to_string(),
            key: Some(key.to_string()),
            payload,
        });
        self.tavily_extract_results
            .lock()
            .expect("fixture tavily extract results should lock")
            .pop_front()
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    async fn bocha_search(&self, key: &str, payload: Value) -> Result<Vec<WebSearchResult>> {
        self.capture(FixtureWebSearchRequest {
            provider: "bocha".to_string(),
            key: Some(key.to_string()),
            payload,
        });
        self.bocha_results
            .lock()
            .expect("fixture bocha results should lock")
            .pop_front()
            .unwrap_or_else(|| Ok(Vec::new()))
    }
}

fn required_string(request: &AgentToolExecutionRequest, name: &str) -> Result<String> {
    optional_string_argument(request, name)
        .ok_or_else(|| AstrbotError::Pipeline(format!("{name} must be a non-empty string")))
}

fn optional_string_argument(request: &AgentToolExecutionRequest, name: &str) -> Option<String> {
    request
        .argument(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn enum_string_argument(
    request: &AgentToolExecutionRequest,
    name: &str,
    default: &str,
    allowed: &[&str],
) -> String {
    optional_enum_string_argument(request, name, allowed).unwrap_or_else(|| default.to_string())
}

fn optional_enum_string_argument(
    request: &AgentToolExecutionRequest,
    name: &str,
    allowed: &[&str],
) -> Option<String> {
    optional_string_argument(request, name).filter(|value| allowed.contains(&value.as_str()))
}

fn usize_argument(request: &AgentToolExecutionRequest, name: &str, default: usize) -> usize {
    request
        .argument(name)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn u64_argument(request: &AgentToolExecutionRequest, name: &str, default: u64) -> u64 {
    request
        .argument(name)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn bool_argument(request: &AgentToolExecutionRequest, name: &str, default: bool) -> bool {
    request
        .argument(name)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn value_string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn strip_html_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_bing_results(html: &str, max_results: usize) -> Vec<WebSearchResult> {
    let mut results = Vec::new();
    let mut remaining = html;
    while results.len() < max_results {
        let Some(start) = remaining.find("b_algo") else {
            break;
        };
        remaining = &remaining[start..];
        let block_end = remaining.find("</li>").unwrap_or(remaining.len());
        let block = &remaining[..block_end];
        if let Some((title, url)) = parse_anchor(block) {
            let snippet = parse_snippet(block);
            results.push(WebSearchResult::new(title, url, snippet));
        }
        remaining = &remaining[block_end.min(remaining.len())..];
        if remaining.starts_with("</li>") {
            remaining = &remaining["</li>".len()..];
        }
    }
    results
}

fn parse_anchor(block: &str) -> Option<(String, String)> {
    let h2_start = block.find("<h2")?;
    let h2 = &block[h2_start..];
    let href_start = h2.find("href=\"")? + "href=\"".len();
    let after_href = &h2[href_start..];
    let href_end = after_href.find('"')?;
    let url = html_unescape(&after_href[..href_end]);
    let anchor_close = after_href[href_end..].find('>')? + href_end + 1;
    let after_anchor = &after_href[anchor_close..];
    let title_end = after_anchor.find("</a>")?;
    let title = strip_html_text(&html_unescape(&after_anchor[..title_end]));
    (!title.is_empty() && !url.is_empty()).then_some((title, url))
}

fn parse_snippet(block: &str) -> String {
    let Some(p_start) = block.find("<p") else {
        return String::new();
    };
    let p = &block[p_start..];
    let Some(content_start) = p.find('>') else {
        return String::new();
    };
    let after_start = &p[content_start + 1..];
    let Some(content_end) = after_start.find("</p>") else {
        return String::new();
    };
    strip_html_text(&html_unescape(&after_start[..content_end]))
}

fn encode_query(query: &str) -> String {
    query
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod web_search_unit_tests {
    use super::*;

    #[test]
    fn parses_bing_result_blocks_for_default_search() {
        let html = r#"
            <ol id="b_results">
              <li class="b_algo"><h2><a href="https://astrbot.app?a=1&amp;b=2">AstrBot <strong>Docs</strong></a></h2><p>Rust chatbot runtime.</p></li>
              <li class="b_algo"><h2><a href="https://example.test">Ignored</a></h2><p>Second.</p></li>
            </ol>
        "#;

        let results = parse_bing_results(html, 1);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "AstrBot Docs");
        assert_eq!(results[0].url, "https://astrbot.app?a=1&b=2");
        assert_eq!(results[0].snippet, "Rust chatbot runtime.");
    }

    #[test]
    fn encodes_default_search_query_for_bing() {
        assert_eq!(encode_query("rust bot"), "rust+bot");
        assert_eq!(encode_query("中文"), "%E4%B8%AD%E6%96%87");
    }
}
