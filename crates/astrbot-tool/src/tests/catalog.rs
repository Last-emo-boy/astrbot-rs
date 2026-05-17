use crate::{ToolActivationPolicy, ToolCatalog, ToolDescriptor, ToolSource};

use super::weather_tool;

#[test]
fn catalog_replaces_tools_and_applies_activation_policy_without_registry_mutation() {
    let mut catalog = ToolCatalog::new();
    catalog.add_tool(ToolDescriptor::new("weather").with_source(ToolSource::Plugin));
    catalog.add_tool(
        weather_tool()
            .with_description("Updated weather")
            .with_source(ToolSource::Mcp),
    );
    catalog.add_tool(ToolDescriptor::new("disabled").inactive());

    let active = catalog.active_tools(&ToolActivationPolicy::new().rename("weather", "forecast"));

    assert_eq!(catalog.tools().len(), 2);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "forecast");
    assert_eq!(active[0].source, ToolSource::Mcp);
}
