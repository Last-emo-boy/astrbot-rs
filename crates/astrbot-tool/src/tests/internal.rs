use serde_json::json;

use crate::{
    InternalToolProviderCatalog, InternalToolProviderDescriptor, InternalToolRegistration,
    ToolCatalog, ToolSource,
};

#[test]
fn internal_provider_catalog_emits_registration_descriptors() {
    let catalog = InternalToolProviderCatalog::new(vec![
        InternalToolProviderDescriptor::new("knowledge_base", "astrbot.core.tools.kb_query")
            .with_registration(InternalToolRegistration::new(
                "knowledge_base",
                "astr_kb_search",
                "Query knowledge base",
                json!({"type": "object"}),
            )),
    ]);
    let mut tool_catalog = ToolCatalog::new();

    catalog.extend_tool_catalog(&mut tool_catalog);

    let tool = tool_catalog
        .tool("astr_kb_search")
        .expect("internal tool should be registered");
    assert_eq!(tool.source, ToolSource::Internal);
    assert_eq!(tool.source.provider_id.as_deref(), Some("knowledge_base"));
    assert!(!tool.source.allows_user_toggle());
}
