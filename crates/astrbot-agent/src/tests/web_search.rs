use std::{collections::BTreeMap, sync::Arc};

use astrbot_core::AstrbotError;
use astrbot_tool::{
    FETCH_URL_TOOL, TAVILY_EXTRACT_TOOL, WEB_SEARCH_BOCHA_TOOL, WEB_SEARCH_TAVILY_TOOL,
    WEB_SEARCH_TOOL, WebExtractedPage, WebSearchProvider, WebSearchResult, WebSearchSessionConfig,
    builtin_internal_tool_catalog,
};
use serde_json::json;

use crate::{
    AgentToolExecutionRequest, AgentToolExecutor, FixtureWebSearchClient, WebSearchToolExecutor,
};

fn request(tool_name: &str, arguments: serde_json::Value) -> AgentToolExecutionRequest {
    let descriptor = builtin_internal_tool_catalog()
        .into_tool_catalog()
        .tool(tool_name)
        .expect("tool descriptor")
        .clone();
    let arguments = arguments
        .as_object()
        .expect("object arguments")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    AgentToolExecutionRequest::new(descriptor, "call-1", "session-1", arguments, "{}")
}

#[tokio::test]
async fn web_search_executor_runs_default_search_and_fetch_url() {
    let client = Arc::new(
        FixtureWebSearchClient::new()
            .with_default_results(vec![WebSearchResult::new(
                "Rust",
                "https://www.rust-lang.org",
                "Rust language",
            )])
            .with_fetched_url("https://www.rust-lang.org", "<main>Rust language</main>"),
    );
    let executor = WebSearchToolExecutor::new(
        WebSearchSessionConfig::enabled(WebSearchProvider::Default).with_web_search_link(true),
        client.clone(),
    );

    let search = executor
        .execute(request(
            WEB_SEARCH_TOOL,
            json!({"query": "rust", "max_results": 3}),
        ))
        .await
        .expect("default search");
    assert!(search.into_text().contains("https://www.rust-lang.org"));

    let fetched = executor
        .execute(request(
            FETCH_URL_TOOL,
            json!({"url": "https://www.rust-lang.org"}),
        ))
        .await
        .expect("fetch url");
    assert_eq!(fetched.into_text(), "<main>Rust language</main>");

    let captured = client.captured();
    assert_eq!(captured[0].payload["max_results"], 3);
    assert_eq!(captured[1].provider, "fetch_url");
}

#[tokio::test]
async fn web_search_executor_rotates_tavily_keys_shapes_json_and_records_favicons() {
    let client = Arc::new(FixtureWebSearchClient::new());
    client.push_tavily_result(Ok(vec![
        WebSearchResult::new("AstrBot", "https://astrbot.app", "AstrBot docs")
            .with_favicon("https://astrbot.app/favicon.ico"),
    ]));
    client.push_tavily_result(Ok(vec![WebSearchResult::new(
        "Rust",
        "https://www.rust-lang.org",
        "Rust language",
    )]));
    let executor = WebSearchToolExecutor::new(
        WebSearchSessionConfig::enabled(WebSearchProvider::Tavily)
            .with_tavily_keys(["key-a", "key-b"]),
        client.clone(),
    );

    let first = executor
        .execute(request(
            WEB_SEARCH_TAVILY_TOOL,
            json!({
                "query": "astrbot",
                "max_results": 7,
                "search_depth": "invalid",
                "topic": "news",
                "days": 2,
                "time_range": "week"
            }),
        ))
        .await
        .expect("first tavily search")
        .into_text();
    let second = executor
        .execute(request(WEB_SEARCH_TAVILY_TOOL, json!({"query": "rust"})))
        .await
        .expect("second tavily search")
        .into_text();

    let first_json: serde_json::Value = serde_json::from_str(&first).expect("json result");
    let second_json: serde_json::Value = serde_json::from_str(&second).expect("json result");
    assert_eq!(first_json["results"][0]["index"], "0001.1");
    assert_eq!(second_json["results"][0]["index"], "0002.1");
    assert_eq!(
        executor
            .metadata()
            .favicons
            .favicons_by_url
            .get("https://astrbot.app")
            .map(String::as_str),
        Some("https://astrbot.app/favicon.ico")
    );

    let captured = client.captured();
    assert_eq!(captured[0].key.as_deref(), Some("key-a"));
    assert_eq!(captured[1].key.as_deref(), Some("key-b"));
    assert_eq!(captured[0].payload["search_depth"], "basic");
    assert_eq!(captured[0].payload["topic"], "news");
    assert_eq!(captured[0].payload["days"], 2);
}

#[tokio::test]
async fn web_search_executor_handles_tavily_extract_no_key_and_http_errors() {
    let client = Arc::new(FixtureWebSearchClient::new());
    let no_key_executor = WebSearchToolExecutor::new(
        WebSearchSessionConfig::enabled(WebSearchProvider::Tavily),
        client.clone(),
    );
    let no_key = no_key_executor
        .execute(request(WEB_SEARCH_TAVILY_TOOL, json!({"query": "rust"})))
        .await
        .expect_err("missing key should fail");
    assert!(no_key.to_string().contains("Tavily API key"));

    client.push_tavily_extract_result(Ok(vec![WebExtractedPage::new(
        "https://astrbot.app",
        "content",
    )]));
    let executor = WebSearchToolExecutor::new(
        WebSearchSessionConfig::enabled(WebSearchProvider::Tavily).with_tavily_keys(["key-a"]),
        client.clone(),
    );
    let extracted = executor
        .execute(request(
            TAVILY_EXTRACT_TOOL,
            json!({"url": "https://astrbot.app", "extract_depth": "advanced"}),
        ))
        .await
        .expect("extract")
        .into_text();
    assert!(extracted.contains("URL: https://astrbot.app"));
    assert!(extracted.contains("Content: content"));

    client.push_tavily_result(Err(AstrbotError::Provider(
        "Tavily web search failed: denied, status: 401".to_string(),
    )));
    let error = executor
        .execute(request(WEB_SEARCH_TAVILY_TOOL, json!({"query": "fail"})))
        .await
        .expect_err("http error");
    assert!(error.to_string().contains("status: 401"));
}

#[tokio::test]
async fn web_search_executor_runs_bocha_empty_and_json_paths() {
    let client = Arc::new(FixtureWebSearchClient::new());
    client.push_bocha_result(Ok(Vec::new()));
    client.push_bocha_result(Ok(vec![
        WebSearchResult::new("Bocha", "https://bocha.example", "snippet")
            .with_favicon("https://bocha.example/icon.png"),
    ]));
    let executor = WebSearchToolExecutor::new(
        WebSearchSessionConfig::enabled(WebSearchProvider::Bocha).with_bocha_keys(["bocha-key"]),
        client.clone(),
    );

    let empty = executor
        .execute(request(WEB_SEARCH_BOCHA_TOOL, json!({"query": "none"})))
        .await
        .expect("empty bocha")
        .into_text();
    assert_eq!(
        empty,
        "Error: BoCha web searcher does not return any results."
    );

    let result = executor
        .execute(request(
            WEB_SEARCH_BOCHA_TOOL,
            json!({
                "query": "bocha",
                "freshness": "oneWeek",
                "summary": true,
                "include": "example.com",
                "exclude": "blocked.com",
                "count": 5
            }),
        ))
        .await
        .expect("bocha result")
        .into_text();
    let result_json: serde_json::Value = serde_json::from_str(&result).expect("json result");
    assert_eq!(result_json["results"][0]["index"], "0001.1");

    let captured = client.captured();
    assert_eq!(captured[1].key.as_deref(), Some("bocha-key"));
    assert_eq!(captured[1].payload["freshness"], "oneWeek");
    assert_eq!(captured[1].payload["summary"], true);
    assert_eq!(
        executor
            .metadata()
            .favicons
            .favicons_by_url
            .get("https://bocha.example")
            .map(String::as_str),
        Some("https://bocha.example/icon.png")
    );
}
