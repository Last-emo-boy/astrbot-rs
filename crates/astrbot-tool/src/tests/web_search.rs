use serde_json::json;

use crate::{
    BAIDU_AI_SEARCH_TOOL, FETCH_URL_TOOL, ToolDescriptor, WEB_SEARCH_BOCHA_TOOL,
    WEB_SEARCH_TAVILY_TOOL, WEB_SEARCH_TOOL, WebSearchProvider, WebSearchResult,
    WebSearchSessionConfig, WebSearchToolSelection, shape_indexed_web_search_results,
};

#[test]
fn web_search_config_parses_source_provider_settings_and_legacy_keys() {
    let config = WebSearchSessionConfig::from_provider_settings(&json!({
        "web_search": true,
        "websearch_provider": "tavily",
        "web_search_link": true,
        "websearch_tavily_key": "legacy-key",
        "websearch_bocha_key": ["bocha-a", "", "bocha-b"],
        "websearch_baidu_app_builder_key": "baidu-key"
    }));

    assert!(config.enabled);
    assert_eq!(config.provider, WebSearchProvider::Tavily);
    assert!(config.web_search_link);
    assert_eq!(config.tavily_keys, vec!["legacy-key"]);
    assert_eq!(config.bocha_keys, vec!["bocha-a", "bocha-b"]);
    assert_eq!(config.baidu_app_builder_key.as_deref(), Some("baidu-key"));
}

#[test]
fn web_search_selection_matches_source_add_remove_provider_modes() {
    let default = WebSearchToolSelection::from_config(&WebSearchSessionConfig::enabled(
        WebSearchProvider::Default,
    ));
    assert_eq!(
        default.selected_tool_names(),
        vec![FETCH_URL_TOOL, WEB_SEARCH_TOOL]
    );

    let tavily = WebSearchToolSelection::from_config(&WebSearchSessionConfig::enabled(
        WebSearchProvider::Tavily,
    ));
    assert_eq!(
        tavily.selected_tool_names(),
        vec!["tavily_extract_web_page", WEB_SEARCH_TAVILY_TOOL]
    );

    let bocha = WebSearchToolSelection::from_config(&WebSearchSessionConfig::enabled(
        WebSearchProvider::Bocha,
    ));
    assert_eq!(bocha.selected_tool_names(), vec![WEB_SEARCH_BOCHA_TOOL]);

    let disabled = WebSearchToolSelection::from_config(&WebSearchSessionConfig::default());
    assert!(disabled.selected_tool_names().is_empty());
}

#[test]
fn web_search_selection_models_baidu_ai_search_mcp_server() {
    let missing_key = WebSearchToolSelection::from_config(&WebSearchSessionConfig::enabled(
        WebSearchProvider::BaiduAiSearch,
    ));
    assert!(missing_key.selected_tool_names().is_empty());
    assert!(missing_key.baidu_mcp_server().is_none());

    let with_key = WebSearchToolSelection::from_config(
        &WebSearchSessionConfig::enabled(WebSearchProvider::BaiduAiSearch)
            .with_baidu_app_builder_key("app-key"),
    );
    assert_eq!(with_key.selected_tool_names(), vec![BAIDU_AI_SEARCH_TOOL]);
    let mcp = with_key.baidu_mcp_server().expect("mcp config");
    assert_eq!(mcp.name, "baidu_ai_search");
    assert_eq!(mcp.transport, "sse");
    assert!(mcp.url.ends_with("api_key=app-key"));
    assert_eq!(mcp.timeout_seconds, 600);
}

#[test]
fn web_search_selection_filters_catalog_without_touching_other_tools() {
    let mut catalog = crate::builtin_internal_tool_catalog().into_tool_catalog();
    catalog.add_tool(ToolDescriptor::new("weather"));

    let filtered = WebSearchToolSelection::from_config(&WebSearchSessionConfig::enabled(
        WebSearchProvider::Default,
    ))
    .apply_to_catalog(&catalog);
    let names = filtered
        .tools()
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&WEB_SEARCH_TOOL));
    assert!(names.contains(&FETCH_URL_TOOL));
    assert!(!names.contains(&WEB_SEARCH_TAVILY_TOOL));
    assert!(!names.contains(&WEB_SEARCH_BOCHA_TOOL));
    assert!(names.contains(&"weather"));
}

#[test]
fn web_search_result_shape_preserves_indices_and_favicon_metadata() {
    let (json, metadata) = shape_indexed_web_search_results(
        &[
            WebSearchResult::new("Rust", "https://www.rust-lang.org", "Rust language")
                .with_favicon("https://www.rust-lang.org/favicon.ico"),
            WebSearchResult::new("AstrBot", "https://astrbot.app", "AstrBot docs"),
        ],
        "abcd",
    );
    let value: serde_json::Value = serde_json::from_str(&json).expect("json result");

    assert_eq!(value["results"][0]["index"], "abcd.1");
    assert_eq!(value["results"][1]["index"], "abcd.2");
    assert_eq!(
        metadata
            .favicons_by_url
            .get("https://www.rust-lang.org")
            .map(String::as_str),
        Some("https://www.rust-lang.org/favicon.ico")
    );
}
