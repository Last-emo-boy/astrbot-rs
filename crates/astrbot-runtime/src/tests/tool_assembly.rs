use astrbot_tool::ToolSource;

use crate::{RuntimeInternalToolAssembly, runtime_internal_tool_catalog};

#[test]
fn runtime_assembles_internal_tool_catalog_without_dashboard_state() {
    let catalog = runtime_internal_tool_catalog();
    let names = catalog
        .tools()
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"astr_kb_search"));
    assert!(names.contains(&"send_message_to_user"));
    assert!(names.contains(&"web_search"));
    assert!(names.contains(&"fetch_url"));
    assert!(names.contains(&"web_search_tavily"));
    assert!(names.contains(&"tavily_extract_web_page"));
    assert!(names.contains(&"web_search_bocha"));
    assert!(
        catalog
            .tools()
            .iter()
            .all(|tool| tool.source == ToolSource::Internal)
    );
    assert!(
        catalog
            .tools()
            .iter()
            .all(|tool| !tool.source.allows_user_toggle())
    );

    let assembly = RuntimeInternalToolAssembly;
    assert_eq!(assembly.registrations().len(), catalog.tools().len());
}
