use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use astrbot_core::{AstrbotError, MessageEvent, Result};
use astrbot_tool::{ToolActivationPolicy, ToolCatalog};
use async_trait::async_trait;

use crate::{
    HandlerMetadata, HotReloadDecision, PLUGIN_SDK_VERSION, PluginCapability, PluginControl,
    PluginDependency, PluginDependencyKind, PluginDependencyPlan, PluginEventType,
    PluginFileChange, PluginFileChangeKind, PluginHandler, PluginLifecycleAction,
    PluginLifecycleState, PluginLoadSource, PluginLoadSourceKind, PluginLoader, PluginManifest,
    PluginModule, PluginPermission, PluginPlatformExtension, PluginPlatformExtensionKind,
    PluginRegistry, PluginRuntimeKind, PluginStateStore, PluginToolDeclaration, PluginWebApiMethod,
    PluginWebApiRoute, RecordingDependencyInstaller, RegisteredHandler, plan_hot_reload,
};

static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(0);

struct RecordingHandler {
    calls: Arc<AtomicUsize>,
    terminates: Arc<AtomicUsize>,
    control: PluginControl,
}

#[async_trait]
impl PluginHandler for RecordingHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.control)
    }

    async fn terminate(&self) -> Result<()> {
        self.terminates.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct TestModule {
    manifest: PluginManifest,
    runtime_requirement: Option<String>,
    dependency_plan: PluginDependencyPlan,
    handler_name: Option<String>,
    tool_name: Option<String>,
    load_count: Arc<AtomicUsize>,
    unload_count: Arc<AtomicUsize>,
    handler_calls: Arc<AtomicUsize>,
    handler_terminates: Arc<AtomicUsize>,
    fail_load: bool,
}

impl TestModule {
    fn new(plugin_id: &str) -> Self {
        Self {
            manifest: PluginManifest::new(plugin_id, "0.1.0")
                .with_capability(PluginCapability::EventHandler)
                .with_capability(PluginCapability::LlmTool)
                .with_capability(PluginCapability::WebApi)
                .with_capability(PluginCapability::PlatformAccess),
            runtime_requirement: Some(">=0.0.0".to_string()),
            dependency_plan: PluginDependencyPlan::new(plugin_id),
            handler_name: Some("message".to_string()),
            tool_name: Some("weather".to_string()),
            load_count: Arc::new(AtomicUsize::new(0)),
            unload_count: Arc::new(AtomicUsize::new(0)),
            handler_calls: Arc::new(AtomicUsize::new(0)),
            handler_terminates: Arc::new(AtomicUsize::new(0)),
            fail_load: false,
        }
    }

    fn with_dependency(mut self, name: &str) -> Self {
        self.dependency_plan = self.dependency_plan.with_dependency(
            PluginDependency::new(PluginDependencyKind::PythonPackage, name)
                .with_version_req(">=1.0"),
        );
        self
    }

    fn with_tool(mut self, name: &str) -> Self {
        self.tool_name = Some(name.to_string());
        self
    }

    fn failing_load(mut self) -> Self {
        self.fail_load = true;
        self
    }
}

#[async_trait]
impl PluginModule for TestModule {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn runtime_version_requirement(&self) -> Option<&str> {
        self.runtime_requirement.as_deref()
    }

    fn dependency_plan(&self, plugin_id: &str) -> PluginDependencyPlan {
        let mut plan = PluginDependencyPlan::new(plugin_id);
        for dependency in self.dependency_plan.dependencies() {
            plan = plan.with_dependency(dependency.clone());
        }
        plan
    }

    fn handlers(&self, _ctx: &crate::PluginContext) -> Vec<RegisteredHandler> {
        self.handler_name
            .as_ref()
            .map(|handler_name| {
                RegisteredHandler::new(
                    HandlerMetadata::new(
                        self.manifest.name.clone(),
                        handler_name,
                        PluginEventType::AdapterMessage,
                    )
                    .with_priority(10),
                    Arc::new(RecordingHandler {
                        calls: self.handler_calls.clone(),
                        terminates: self.handler_terminates.clone(),
                        control: PluginControl::Stop,
                    }),
                )
            })
            .into_iter()
            .collect()
    }

    fn tools(&self, _ctx: &crate::PluginContext) -> Vec<PluginToolDeclaration> {
        self.tool_name
            .as_ref()
            .map(|name| PluginToolDeclaration::local(name).with_description("tool from plugin"))
            .into_iter()
            .collect()
    }

    fn web_routes(&self, _ctx: &crate::PluginContext) -> Vec<PluginWebApiRoute> {
        vec![
            PluginWebApiRoute::new(&self.manifest.name, "/api/plugin/test")
                .with_method(PluginWebApiMethod::Post),
        ]
    }

    fn platform_extensions(&self, _ctx: &crate::PluginContext) -> Vec<PluginPlatformExtension> {
        vec![PluginPlatformExtension::new(
            &self.manifest.name,
            "bridge",
            PluginPlatformExtensionKind::MessageBridge,
            "webchat",
        )]
    }

    async fn on_load(&self, _ctx: &crate::PluginContext) -> Result<()> {
        if self.fail_load {
            return Err(AstrbotError::Pipeline("module load failed".to_string()));
        }
        self.load_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn on_unload(&self, _ctx: &crate::PluginContext) -> Result<()> {
        self.unload_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn plugin_loader_discovers_and_activates_manifest_metadata() {
    let manifest = PluginManifest::new("My Fancy Plugin", "0.1.0")
        .with_permission(PluginPermission::RegisterWebApi)
        .with_capability(PluginCapability::WebApi);
    let mut loader = PluginLoader::new();

    let metadata = loader
        .discover_manifest(PluginLoadSource::native_rust("My Fancy Plugin"), manifest)
        .expect("manifest should be discovered")
        .with_supported_platform("webchat")
        .with_runtime_version(PLUGIN_SDK_VERSION);

    assert_eq!(metadata.plugin_id(), "my_fancy_plugin");
    assert_eq!(metadata.supported_platforms(), &["webchat".to_string()]);
    assert_eq!(metadata.runtime_version(), Some(PLUGIN_SDK_VERSION));

    let loaded = loader
        .mark_loaded("my_fancy_plugin")
        .expect("plugin should transition to loaded");
    assert_eq!(loaded.previous, PluginLifecycleState::Discovered);
    assert_eq!(loaded.next, PluginLifecycleState::Loaded);

    let activated = loader
        .activate("my_fancy_plugin")
        .expect("plugin should transition to active");
    assert_eq!(activated.action, PluginLifecycleAction::Activate);
    assert_eq!(activated.next, PluginLifecycleState::Active);

    let context = loader
        .context_for("my_fancy_plugin")
        .expect("plugin context should be available");
    assert!(context.allows_permission(PluginPermission::RegisterWebApi));
}

#[test]
fn plugin_loader_disables_and_unloads_without_dynamic_imports() {
    let manifest = PluginManifest::new("stateful", "0.1.0");
    let mut loader = PluginLoader::new();
    loader
        .discover_manifest(PluginLoadSource::python_compat("stateful"), manifest)
        .expect("manifest should be discovered");
    loader
        .mark_loaded("stateful")
        .expect("plugin should transition to loaded");
    loader
        .activate("stateful")
        .expect("plugin should transition to active");

    let disabled = loader
        .disable("stateful")
        .expect("plugin should transition to disabled");
    assert_eq!(disabled.previous, PluginLifecycleState::Active);
    assert_eq!(disabled.next, PluginLifecycleState::Disabled);

    let unloaded = loader
        .unload("stateful")
        .expect("plugin should transition to unloaded");
    assert_eq!(unloaded.next, PluginLifecycleState::Unloaded);
}

#[test]
fn hot_reload_plans_source_changes() {
    let source_change = PluginFileChange::new(
        "plugin",
        "plugins/plugin/src/lib.rs",
        PluginFileChangeKind::Modified,
    );
    let asset_change = PluginFileChange::new(
        "plugin",
        "plugins/plugin/logo.png",
        PluginFileChangeKind::Modified,
    );
    let removal = PluginFileChange::new("plugin", "plugins/plugin", PluginFileChangeKind::Removed);

    assert_eq!(plan_hot_reload(&source_change), HotReloadDecision::Reload);
    assert_eq!(plan_hot_reload(&asset_change), HotReloadDecision::Ignore);
    assert_eq!(plan_hot_reload(&removal), HotReloadDecision::Unload);
}

#[test]
fn plugin_loader_discovers_reserved_and_user_metadata_from_filesystem() {
    let root = temp_dir("discover");
    let user_dir = root.join("plugins");
    let reserved_dir = root.join("reserved");
    std::fs::create_dir_all(user_dir.join("weather")).expect("create user plugin");
    std::fs::create_dir_all(reserved_dir.join("builtin")).expect("create reserved plugin");
    std::fs::write(
        user_dir.join("weather").join("metadata.yaml"),
        "name: Weather Plugin\nversion: 1.2.3\ndesc: weather lookup\nauthor: Alice\nsupport_platforms:\n  - webchat\nastrbot_version: \">=0.0.0\"\n",
    )
    .expect("write user metadata");
    std::fs::write(
        reserved_dir.join("builtin").join("metadata.yaml"),
        "name: Builtin Plugin\nversion: 1.0.0\nauthor: AstrBot\n",
    )
    .expect("write reserved metadata");

    let mut loader = PluginLoader::new();
    let user = loader
        .discover_plugins_from_directory(&user_dir, PluginLoadSourceKind::PythonCompat, false)
        .expect("user plugins should be discovered");
    let reserved = loader
        .discover_plugins_from_directory(&reserved_dir, PluginLoadSourceKind::NativeRust, true)
        .expect("reserved plugins should be discovered");

    assert_eq!(user[0].plugin_id(), "weather");
    assert_eq!(
        user[0].manifest.description.as_deref(),
        Some("weather lookup")
    );
    assert_eq!(user[0].manifest.authors, vec!["Alice".to_string()]);
    assert_eq!(user[0].supported_platforms(), &["webchat".to_string()]);
    assert!(!user[0].source.is_reserved());
    assert!(reserved[0].source.is_reserved());

    let incompatible = root.join("bad");
    std::fs::create_dir_all(incompatible.join("bad_plugin")).expect("create bad plugin");
    std::fs::write(
        incompatible.join("bad_plugin").join("metadata.yaml"),
        "name: Bad Plugin\nversion: 1.0.0\nauthor: Bob\nastrbot_version: \">9999.0.0\"\n",
    )
    .expect("write bad metadata");
    assert!(
        loader
            .discover_plugins_from_directory(
                &incompatible,
                PluginLoadSourceKind::PythonCompat,
                false,
            )
            .is_err()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn plugin_loader_loads_module_registers_contributions_and_unload_cleans_runtime_assets() {
    let installer = RecordingDependencyInstaller::new();
    let mut loader = PluginLoader::new().with_dependency_installer(installer.clone());
    let module = Arc::new(TestModule::new("runtime_plugin").with_dependency("httpx"));
    let load_count = module.load_count.clone();
    let unload_count = module.unload_count.clone();
    let handler_calls = module.handler_calls.clone();
    let handler_terminates = module.handler_terminates.clone();
    let mut registry = PluginRegistry::new();
    let mut catalog = ToolCatalog::new();

    let loaded = loader
        .load_module(
            PluginLoadSource::native_rust("runtime_plugin"),
            module,
            &mut registry,
            &mut catalog,
        )
        .await
        .expect("plugin should load");

    assert_eq!(loaded.next, PluginLifecycleState::Active);
    assert_eq!(load_count.load(Ordering::SeqCst), 1);
    assert_eq!(registry.handler_count(), 1);
    assert_eq!(
        catalog
            .tool("weather")
            .expect("tool should be registered")
            .source
            .plugin_id
            .as_deref(),
        Some("runtime_plugin")
    );
    let registration = loader
        .registration("runtime_plugin")
        .expect("registration should be tracked");
    assert_eq!(registration.handler_count, 1);
    assert_eq!(registration.tool_names, vec!["weather".to_string()]);
    assert_eq!(registration.web_routes[0].route, "/api/plugin/test");
    assert_eq!(registration.platform_extensions[0].platform_type, "webchat");
    assert_eq!(installer.requests().len(), 1);
    assert_eq!(
        installer.requests()[0].environment.runtime_kind,
        PluginRuntimeKind::NativeRust
    );

    let mut event = super::event("/weather");
    let control = registry
        .handle_event(PluginEventType::AdapterMessage, &mut event)
        .await
        .expect("handler should run");
    assert_eq!(control, PluginControl::Stop);
    assert_eq!(handler_calls.load(Ordering::SeqCst), 1);

    loader
        .unload_plugin("runtime_plugin", &mut registry, &mut catalog)
        .await
        .expect("plugin should unload");
    assert_eq!(unload_count.load(Ordering::SeqCst), 1);
    assert_eq!(handler_terminates.load(Ordering::SeqCst), 1);
    assert_eq!(registry.handler_count(), 0);
    assert!(catalog.tool("weather").is_none());
    assert!(loader.registration("runtime_plugin").is_none());
    assert_eq!(
        loader.store().get("runtime_plugin").expect("record").state,
        PluginLifecycleState::Unloaded
    );
    assert!(
        loader
            .runtime_events()
            .iter()
            .any(|event| event.event_type == PluginEventType::OnPluginLoaded)
    );
    assert!(
        loader
            .runtime_events()
            .iter()
            .any(|event| event.event_type == PluginEventType::OnPluginUnloaded)
    );
}

#[tokio::test]
async fn plugin_loader_reload_replaces_handlers_and_tools() {
    let mut loader = PluginLoader::new();
    let first = Arc::new(TestModule::new("reload_plugin").with_tool("old_tool"));
    let first_unload_count = first.unload_count.clone();
    let first_handler_terminates = first.handler_terminates.clone();
    let second = Arc::new(TestModule::new("reload_plugin").with_tool("new_tool"));
    let mut registry = PluginRegistry::new();
    let mut catalog = ToolCatalog::new();

    loader
        .load_module(
            PluginLoadSource::native_rust("reload_plugin"),
            first,
            &mut registry,
            &mut catalog,
        )
        .await
        .expect("first plugin should load");
    loader
        .reload_module(
            PluginLoadSource::native_rust("reload_plugin"),
            second,
            &mut registry,
            &mut catalog,
        )
        .await
        .expect("plugin should reload");

    assert_eq!(first_unload_count.load(Ordering::SeqCst), 1);
    assert_eq!(first_handler_terminates.load(Ordering::SeqCst), 1);
    assert_eq!(registry.handler_count(), 1);
    assert!(catalog.tool("old_tool").is_none());
    assert!(catalog.tool("new_tool").is_some());
}

#[tokio::test]
async fn plugin_loader_records_plugin_error_when_load_fails() {
    let mut loader = PluginLoader::new();
    let module = Arc::new(TestModule::new("bad_runtime").failing_load());
    let mut registry = PluginRegistry::new();
    let mut catalog = ToolCatalog::new();

    let error = loader
        .load_module(
            PluginLoadSource::native_rust("bad_runtime"),
            module,
            &mut registry,
            &mut catalog,
        )
        .await
        .expect_err("load should fail");

    assert!(error.to_string().contains("module load failed"));
    assert_eq!(
        loader.store().get("bad_runtime").expect("record").state,
        PluginLifecycleState::Failed
    );
    assert!(
        loader
            .runtime_events()
            .iter()
            .any(|event| event.event_type == PluginEventType::OnPluginError)
    );
    assert_eq!(registry.handler_count(), 0);
    assert_eq!(
        catalog.active_tools(&ToolActivationPolicy::default()).len(),
        0
    );
}

#[tokio::test]
async fn plugin_loader_reuses_python_requirements_during_compat_load() {
    let root = temp_dir("python_requirements");
    let plugin_root = root.join("py_plugin");
    std::fs::create_dir_all(&plugin_root).expect("create python plugin");
    std::fs::write(
        plugin_root.join("requirements.txt"),
        "requests>=2.0\n# ignored\n",
    )
    .expect("write requirements");

    let installer = RecordingDependencyInstaller::new();
    let mut loader = PluginLoader::new().with_dependency_installer(installer.clone());
    let module = Arc::new(TestModule::new("py_plugin"));
    let mut registry = PluginRegistry::new();
    let mut catalog = ToolCatalog::new();

    loader
        .load_module(
            PluginLoadSource::python_compat("py_plugin").with_root_dir(&plugin_root),
            module,
            &mut registry,
            &mut catalog,
        )
        .await
        .expect("python compat plugin should load");

    let requests = installer.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].environment.runtime_kind,
        PluginRuntimeKind::PythonCompat
    );
    assert_eq!(
        requests[0].environment.plugin_root(),
        Some(plugin_root.as_path())
    );
    assert_eq!(requests[0].plan.dependencies()[0].name, "requests");
    assert_eq!(
        requests[0].plan.dependencies()[0].version_req.as_deref(),
        Some(">=2.0")
    );

    let _ = std::fs::remove_dir_all(root);
}

fn temp_dir(label: &str) -> PathBuf {
    let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::SeqCst);
    let mut root = std::env::temp_dir();
    root.push(format!(
        "astrbot_plugin_{label}_{}_{}",
        std::process::id(),
        id
    ));
    if root.exists() {
        let _ = std::fs::remove_dir_all(&root);
    }
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
