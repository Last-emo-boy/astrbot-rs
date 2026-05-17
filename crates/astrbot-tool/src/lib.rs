mod activation;
mod catalog;
mod commands;
mod conflicts;
mod internal;
pub mod schema;
mod source;

pub use activation::ToolActivationPolicy;
pub use catalog::{ToolCatalog, ToolDescriptor};
pub use commands::{CommandDescriptor, CommandPermission, CommandType};
pub use conflicts::{
    CommandConflict, ToolConflict, detect_command_conflicts, detect_tool_conflicts,
};
pub use internal::{
    InternalToolProviderCatalog, InternalToolProviderDescriptor, InternalToolRegistration,
    builtin_internal_tool_catalog, builtin_internal_tool_registrations,
};
pub use schema::{ProviderToolSchemaFormat, ToolSchemaSerializer};
pub use source::{ToolSource, ToolSourceMetadata, ToolUserTogglePolicy};

#[cfg(test)]
mod tests;
