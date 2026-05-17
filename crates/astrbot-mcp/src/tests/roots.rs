use crate::{McpRoot, McpRootAlias, McpRootsCapabilityConfig, McpUri};

#[test]
fn roots_keep_aliases_and_uri_typed() {
    let defaults = McpRootsCapabilityConfig::enabled_for_default_paths();
    assert_eq!(defaults.paths, vec!["data".to_string(), "temp".to_string()]);
    assert!(
        McpRootAlias::all()
            .iter()
            .any(|alias| alias.as_str() == "knowledge_base")
    );

    let root = McpRoot::new(McpUri::new("file:///tmp").expect("uri")).named("temp");
    let json = serde_json::to_value(root).expect("root should serialize");

    assert_eq!(json["uri"], "file:///tmp");
    assert_eq!(json["name"], "temp");
}
