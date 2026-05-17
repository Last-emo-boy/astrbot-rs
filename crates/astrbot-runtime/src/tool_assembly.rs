use astrbot_tool::{
    InternalToolRegistration, ToolCatalog, builtin_internal_tool_catalog,
    builtin_internal_tool_registrations,
};

#[derive(Clone, Debug, Default)]
pub struct RuntimeInternalToolAssembly;

impl RuntimeInternalToolAssembly {
    pub fn registrations(&self) -> Vec<InternalToolRegistration> {
        runtime_internal_tool_registrations()
    }

    pub fn catalog(&self) -> ToolCatalog {
        runtime_internal_tool_catalog()
    }
}

pub fn runtime_internal_tool_registrations() -> Vec<InternalToolRegistration> {
    builtin_internal_tool_registrations()
}

pub fn runtime_internal_tool_catalog() -> ToolCatalog {
    builtin_internal_tool_catalog().into_tool_catalog()
}
