pub mod dependency;
mod event;
pub mod extension;
pub mod filter;
mod handler;
pub mod loader;
pub mod manifest;
pub mod market;
mod registry;
pub mod sandbox;
pub mod sdk;
pub mod tool;

pub use dependency::{
    DependencyConflictKind, DependencyConflictReport, DependencyErrorRedactor,
    DependencyInstallOutcome, DependencyInstallRequest, DependencyInstallStatus,
    NoopDependencyInstaller, PackagePreferencePolicy, PluginDependency, PluginDependencyInstaller,
    PluginDependencyKind, PluginDependencyPlan, PluginDependencyPlanInstaller,
    PluginImportEnvironment, PluginRuntimeKind, RecordingDependencyInstaller,
    RuntimeImportBehavior,
};
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
    HotReloadDecision, InMemoryPluginStore, PluginFileChange, PluginFileChangeKind,
    PluginLifecycleAction, PluginLifecycleEvent, PluginLifecycleState, PluginLoadSource,
    PluginLoadSourceKind, PluginLoader, PluginMetadata, PluginRecord, PluginStateStore,
    plan_hot_reload,
};
pub use manifest::{PluginCapability, PluginManifest};
pub use market::{
    PluginCompatibility, PluginDocument, PluginDocumentFormat, PluginInstallPlan,
    PluginInstallSource, PluginMarketAction, PluginMarketCache, PluginMarketEntry,
    PluginMarketOperationPlan, PluginPackageDescriptor, PluginRegistrySource, PluginUninstallPlan,
    PluginUpdatePlan,
};
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
