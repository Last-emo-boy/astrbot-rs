use serde_json::json;

use crate::{
    InternalToolProviderCatalog, InternalToolProviderDescriptor, InternalToolRegistration,
    ToolCatalog, ToolSource, builtin_internal_tool_catalog,
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

#[test]
fn computer_use_provider_registers_inactive_tools_with_source_compatible_schemas() {
    let catalog = builtin_internal_tool_catalog();
    let provider = catalog
        .providers()
        .iter()
        .find(|provider| provider.provider_id == "computer_use")
        .expect("computer_use provider should exist");
    let registrations = provider.registrations();
    let names = registrations
        .iter()
        .map(|registration| registration.descriptor.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(registrations.len(), 7);
    for expected in [
        "astrbot_execute_shell",
        "astrbot_execute_ipython",
        "astrbot_upload_file",
        "astrbot_download_file",
        "astrbot_execute_browser",
        "astrbot_execute_browser_batch",
        "astrbot_run_browser_skill",
    ] {
        assert!(names.contains(&expected), "missing {expected}");
    }

    let shell = registrations
        .iter()
        .find(|registration| registration.descriptor.name == "astrbot_execute_shell")
        .expect("shell registration");
    assert!(!shell.descriptor.active);
    assert_eq!(shell.descriptor.parameters["required"][0], "command");
    assert!(shell.descriptor.parameters["properties"]["env"]["additionalProperties"].is_object());

    let browser_batch = registrations
        .iter()
        .find(|registration| registration.descriptor.name == "astrbot_execute_browser_batch")
        .expect("browser batch registration");
    assert_eq!(
        browser_batch.descriptor.parameters["properties"]["commands"]["items"]["type"],
        "string"
    );

    let skill = registrations
        .iter()
        .find(|registration| registration.descriptor.name == "astrbot_run_browser_skill")
        .expect("browser skill registration");
    assert_eq!(skill.descriptor.parameters["required"][0], "skill_key");
}

#[test]
fn t2i_provider_registers_render_tool_for_agent_use() {
    let catalog = builtin_internal_tool_catalog();
    let provider = catalog
        .providers()
        .iter()
        .find(|provider| provider.provider_id == "t2i")
        .expect("t2i provider should exist");
    let render = provider
        .registrations()
        .iter()
        .find(|registration| registration.descriptor.name == "astrbot_t2i_render")
        .expect("t2i render tool should be registered");

    assert_eq!(render.descriptor.parameters["required"][0], "prompt");
    assert_eq!(
        render.descriptor.parameters["properties"]["format"]["enum"][0],
        "png"
    );
}
