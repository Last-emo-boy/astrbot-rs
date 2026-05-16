mod event;
pub mod extension;
pub mod filter;
mod handler;
pub mod loader;
pub mod manifest;
mod registry;
pub mod sandbox;
pub mod sdk;
pub mod tool;

pub use event::{PluginControl, PluginEventType};
pub use extension::{
    PluginPlatformExtension, PluginPlatformExtensionKind, PluginWebApiMethod, PluginWebApiRoute,
};
pub use filter::{
    AlwaysFilter, CommandFilter, EventFilter, MessageSessionKindFilter, PermissionFilter,
    PermissionLevel, PermissionScope, PlatformFilter, RegexFilter,
};
pub use handler::{HandlerMetadata, PluginHandler, RegisteredHandler};
pub use loader::{
    HotReloadDecision, InMemoryPluginStore, NoopDependencyInstaller, PluginDependency,
    PluginDependencyInstaller, PluginDependencyKind, PluginDependencyPlan, PluginFileChange,
    PluginFileChangeKind, PluginLifecycleAction, PluginLifecycleEvent, PluginLifecycleState,
    PluginLoadSource, PluginLoadSourceKind, PluginLoader, PluginMetadata, PluginRecord,
    PluginStateStore, plan_hot_reload,
};
pub use manifest::{PluginCapability, PluginManifest};
pub use registry::PluginRegistry;
pub use sandbox::{PluginPermission, SandboxProfile, SandboxRuntime, ToolCapability};
pub use sdk::{PLUGIN_SDK_VERSION, PluginContext, PluginModule, PluginTestHarness};
pub use tool::{
    BackgroundTaskPolicy, HandoffToolTarget, PluginToolDeclaration, PluginToolKind,
    SandboxedToolExecutor, ToolCapabilityDecision, ToolExecutionRequest, ToolExecutionResult,
    ToolExecutionStatus, ToolExecutor,
};

#[cfg(test)]
mod tests;
