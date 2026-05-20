mod activation;
mod catalog;
mod commands;
mod conflicts;
mod internal;
mod reference;
pub mod schema;
mod source;
mod web_search;

pub use activation::ToolActivationPolicy;
pub use catalog::{ToolCatalog, ToolDescriptor};
pub use commands::{
    BUILTIN_COMMAND_PLUGIN_NAME, CommandDescriptor, CommandPermission, CommandType,
    builtin_command_descriptors,
};
pub use conflicts::{
    CommandConflict, ToolConflict, detect_command_conflicts, detect_tool_conflicts,
};
pub use internal::{
    InternalToolProviderCatalog, InternalToolProviderDescriptor, InternalToolRegistration,
    T2I_RENDER_TOOL, builtin_internal_tool_catalog, builtin_internal_tool_registrations,
};
pub use reference::{
    ToolCallReferencePayload, ToolReferenceExtractor, ToolReferenceItem, ToolReferenceSet,
    ToolReferenceSource,
};
pub use schema::{ProviderToolSchemaFormat, ToolSchemaSerializer};
pub use source::{ToolSource, ToolSourceMetadata, ToolUserTogglePolicy};
pub use web_search::{
    BAIDU_AI_SEARCH_TOOL, BaiduAiSearchMcpServerConfig, BochaSearchRequest, FETCH_URL_TOOL,
    TAVILY_EXTRACT_TOOL, TavilyExtractRequest, TavilySearchRequest, WEB_SEARCH_BOCHA_TOOL,
    WEB_SEARCH_TAVILY_TOOL, WEB_SEARCH_TOOL, WEB_SEARCH_TOOL_NAMES, WebExtractedPage,
    WebSearchFaviconMetadata, WebSearchProvider, WebSearchResult, WebSearchSessionConfig,
    WebSearchToolSelection, is_web_search_tool_name, shape_indexed_web_search_results,
    web_search_tool_descriptors,
};

#[cfg(test)]
mod tests;
