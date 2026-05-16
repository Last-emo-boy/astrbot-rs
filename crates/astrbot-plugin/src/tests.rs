use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use astrbot_core::{
    MessageChain, MessageEvent, MessageSender, MessageSession, MessageSessionKind, MessageSink,
    Result,
};
use async_trait::async_trait;

use super::*;

struct NoopSink;

#[async_trait]
impl MessageSink for NoopSink {
    async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
        Ok(())
    }
}

fn event(message: impl Into<String>) -> MessageEvent {
    MessageEvent::new(
        "event",
        "console",
        "console",
        MessageSession::new("console", "user"),
        MessageSender::new("user", None),
        MessageChain::plain(message),
        Arc::new(NoopSink),
    )
}

struct CountingHandler {
    calls: Arc<AtomicUsize>,
    control: PluginControl,
}

#[async_trait]
impl PluginHandler for CountingHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.control)
    }
}

struct TerminatingHandler {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl PluginHandler for TerminatingHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        Ok(PluginControl::Continue)
    }

    async fn terminate(&self) -> Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn command_filter_matches_alias_and_prefix() {
    let filter = CommandFilter::new("/ping").with_alias("p").with_prefix("!");

    assert!(filter.matches(&event("!ping now")));
    assert!(filter.matches(&event("!p now")));
    assert!(!filter.matches(&event("/ping now")));
}

#[test]
fn typed_filters_match_platform_session_permission_and_regex() {
    let mut group = event("hello 123");
    group.session = group.session.with_kind(MessageSessionKind::Group);

    assert!(PlatformFilter::new("console").matches(&group));
    assert!(MessageSessionKindFilter::group().matches(&group));

    let scope = PermissionScope::new().with_admin_user_id("user");
    assert!(PermissionFilter::admin(scope).matches(&group));

    let regex = RegexFilter::new(r"\d+").expect("regex should compile");
    assert!(regex.matches(&group));
}

#[tokio::test]
async fn registry_orders_handlers_and_stops_on_stop_control() {
    let low_calls = Arc::new(AtomicUsize::new(0));
    let high_calls = Arc::new(AtomicUsize::new(0));
    let mut registry = PluginRegistry::new();
    registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("plugin", "low", PluginEventType::AdapterMessage).with_priority(0),
        Arc::new(CountingHandler {
            calls: low_calls.clone(),
            control: PluginControl::Continue,
        }),
    ));
    registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("plugin", "high", PluginEventType::AdapterMessage).with_priority(10),
        Arc::new(CountingHandler {
            calls: high_calls.clone(),
            control: PluginControl::Stop,
        }),
    ));

    let mut event = event("/ping");
    let control = registry
        .handle_event(PluginEventType::AdapterMessage, &mut event)
        .await
        .expect("registry should handle event");

    assert_eq!(control, PluginControl::Stop);
    assert_eq!(high_calls.load(Ordering::SeqCst), 1);
    assert_eq!(low_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn registry_terminates_registered_handlers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let mut registry = PluginRegistry::new();
    registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("plugin", "handler", PluginEventType::AdapterMessage),
        Arc::new(TerminatingHandler {
            terminate_count: terminate_count.clone(),
        }),
    ));

    registry
        .terminate()
        .await
        .expect("registry should terminate handlers");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[test]
fn manifest_drives_plugin_context_sandbox_permissions() {
    let manifest = PluginManifest::new("tools", "0.1.0")
        .with_capability(PluginCapability::SandboxTool)
        .with_permission(PluginPermission::SendMessage)
        .with_tool_capability(ToolCapability::Browser);

    let harness = PluginTestHarness::from_manifest(&manifest);
    let ctx = harness.context();

    assert_eq!(ctx.plugin_name(), "tools");
    assert!(ctx.allows_permission(PluginPermission::SendMessage));
    assert!(ctx.allows_tool_capability(ToolCapability::Browser));
    assert!(!ctx.allows_tool_capability(ToolCapability::Shell));
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

#[tokio::test]
async fn plugin_dependency_plan_is_installer_boundary() {
    let plan = PluginDependencyPlan::new("tools").with_dependency(
        PluginDependency::new(PluginDependencyKind::PythonPackage, "watchfiles")
            .with_version_req(">=0.21")
            .optional(),
    );

    assert_eq!(plan.dependencies().len(), 1);
    assert!(plan.dependencies()[0].optional);

    NoopDependencyInstaller
        .ensure_dependencies(&plan)
        .await
        .expect("noop installer should accept dependency plan");
}

#[tokio::test]
async fn plugin_loader_runs_dependency_plan_through_installer_port() {
    let recorder = RecordingDependencyInstaller::new();
    let loader = PluginLoader::new().with_dependency_installer(recorder.clone());
    let plan = PluginDependencyPlan::new("tools").with_dependency(PluginDependency::new(
        PluginDependencyKind::PythonPackage,
        "watchfiles",
    ));
    let environment = PluginImportEnvironment::python_compat("tools")
        .with_plugin_root("plugins/tools")
        .with_isolated_dependency_root("data/plugins/.deps/tools");

    let outcome = loader
        .ensure_dependencies(plan.clone(), environment.clone())
        .await
        .expect("installer should run through port");

    assert_eq!(outcome.status, DependencyInstallStatus::Completed);
    assert_eq!(outcome.installed(), plan.dependencies());

    let requests = recorder.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].plan, plan);
    assert_eq!(requests[0].environment, environment);
}

#[test]
fn plugin_import_environment_models_isolated_and_site_package_preference() {
    let environment = PluginImportEnvironment::python_compat("tools")
        .with_plugin_root("plugins/tools")
        .with_isolated_dependency_root("data/plugins/.deps/tools")
        .with_site_packages_root("runtime/site-packages")
        .prefer_installed_site_packages();

    assert_eq!(
        environment.package_preference(),
        PackagePreferencePolicy::PreferInstalledSitePackages
    );
    assert!(environment.should_prefer_site_packages());
    assert_eq!(
        environment.import_roots(),
        vec![
            std::path::PathBuf::from("runtime/site-packages"),
            std::path::PathBuf::from("data/plugins/.deps/tools"),
            std::path::PathBuf::from("plugins/tools"),
        ]
    );
}

#[test]
fn packaged_runtime_environment_keeps_import_patch_policy_typed() {
    let environment = PluginImportEnvironment::python_compat("desktop-plugin")
        .with_site_packages_root("astrbot/site-packages")
        .packaged_python_runtime();

    assert_eq!(environment.runtime_kind, PluginRuntimeKind::PythonCompat);
    assert!(environment.runtime_behavior().is_packaged_python());
    assert!(environment.runtime_behavior().patch_distribution_finder());
    assert!(environment.should_prefer_site_packages());
}

#[test]
fn dependency_conflicts_are_classified_and_redacted_for_user_surfaces() {
    let output = [
        "The user requested httpx==0.20",
        "astrbot-core depends on httpx==0.27 (constraint)",
        "Cannot install because these package versions have conflicting dependencies",
        "Using index https://user:token@example.com/simple?token=secret",
        "--password=hunter2",
    ];

    let report = DependencyConflictReport::from_installer_output("tools", output)
        .expect("conflict should be classified");

    assert_eq!(report.kind, DependencyConflictKind::CoreVersionConflict);
    assert!(report.is_core_conflict());
    assert!(
        report
            .details()
            .iter()
            .any(|line| line.contains("https://<redacted>@example.com/simple?token=****"))
    );
    assert!(
        report
            .details()
            .iter()
            .any(|line| line.contains("--password=****"))
    );
    assert!(!report.details().join("\n").contains("hunter2"));
    assert!(!report.details().join("\n").contains("user:token"));
}

#[test]
fn dependency_redactor_handles_inline_and_next_arg_secrets() {
    let redactor = DependencyErrorRedactor::new();
    let args = vec![
        "--index-url=https://user:token@example.com/simple".to_string(),
        "--password".to_string(),
        "hunter2".to_string(),
        "token=abc123".to_string(),
    ];

    assert_eq!(
        redactor.redact_args(&args),
        vec![
            "--index-url=https://<redacted>@example.com/simple".to_string(),
            "--password".to_string(),
            "****".to_string(),
            "token=****".to_string(),
        ]
    );
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
fn plugin_extension_descriptors_are_typed() {
    let platform_extension = PluginPlatformExtension::new(
        "plugin",
        "webchat-extra",
        PluginPlatformExtensionKind::MessageBridge,
        "webchat",
    )
    .with_description("extra webchat bridge");
    assert_eq!(platform_extension.platform_type, "webchat");
    assert_eq!(
        platform_extension.kind,
        PluginPlatformExtensionKind::MessageBridge
    );

    let route = PluginWebApiRoute::new("plugin", "api/plugins/plugin")
        .with_method(PluginWebApiMethod::Post)
        .with_description("plugin management route");
    assert_eq!(route.route, "/api/plugins/plugin");
    assert!(route.methods.contains(&PluginWebApiMethod::Get));
    assert!(route.methods.contains(&PluginWebApiMethod::Post));
}

struct EchoToolExecutor;

#[async_trait]
impl ToolExecutor for EchoToolExecutor {
    async fn execute(&self, request: ToolExecutionRequest) -> Result<ToolExecutionResult> {
        Ok(ToolExecutionResult::completed(
            request.argument("text").unwrap_or(""),
        ))
    }
}

#[test]
fn tool_capability_decision_reports_missing_sandbox_requirements() {
    let declaration = PluginToolDeclaration::local("browser")
        .requires_permission(PluginPermission::UseNetwork)
        .requires_capability(ToolCapability::Browser);
    let decision = ToolCapabilityDecision::check(&declaration, &SandboxProfile::restricted());

    assert!(!decision.allowed);
    assert_eq!(
        decision.missing_permissions,
        vec![PluginPermission::UseNetwork]
    );
    assert_eq!(decision.missing_capabilities, vec![ToolCapability::Browser]);
    assert!(
        decision
            .rejection_message("browser")
            .expect("rejection should explain missing requirements")
            .contains("missing tool capabilities")
    );
}

#[tokio::test]
async fn sandboxed_tool_executor_runs_allowed_tools() {
    let declaration =
        PluginToolDeclaration::local("browser").requires_capability(ToolCapability::Browser);
    let context = PluginContext::new("tools").with_sandbox_profile(
        SandboxProfile::restricted().with_tool_capability(ToolCapability::Browser),
    );
    let request = ToolExecutionRequest::new(declaration, context).with_argument("text", "ok");

    let result = SandboxedToolExecutor::new(EchoToolExecutor)
        .execute(request)
        .await
        .expect("allowed tool should execute");

    assert_eq!(result.status, ToolExecutionStatus::Completed);
    assert_eq!(result.content.as_deref(), Some("ok"));
}

#[tokio::test]
async fn sandboxed_tool_executor_rejects_missing_capability() {
    let declaration =
        PluginToolDeclaration::local("browser").requires_capability(ToolCapability::Browser);
    let context = PluginContext::new("tools");
    let request = ToolExecutionRequest::new(declaration, context);

    let err = SandboxedToolExecutor::new(EchoToolExecutor)
        .execute(request)
        .await
        .expect_err("missing browser capability should reject execution");

    assert!(err.to_string().contains("rejected by sandbox"));
}

#[test]
fn tool_declarations_model_handoff_and_background_boundaries() {
    let handoff = PluginToolDeclaration::handoff(
        "delegate",
        HandoffToolTarget::new("researcher")
            .with_provider_id("provider-a")
            .allow_background(),
    );
    let background = PluginToolDeclaration::background(
        "long-job",
        BackgroundTaskPolicy::new()
            .with_max_seconds(300)
            .with_note("summarize later"),
    )
    .requires_permission(PluginPermission::SpawnBackgroundTask);

    match handoff.kind {
        PluginToolKind::Handoff(target) => {
            assert_eq!(target.agent_name, "researcher");
            assert_eq!(target.provider_id.as_deref(), Some("provider-a"));
            assert!(target.background_allowed);
        }
        _ => panic!("expected handoff tool kind"),
    }

    match background.kind {
        PluginToolKind::Background(policy) => {
            assert!(policy.wake_on_complete);
            assert_eq!(policy.max_seconds, Some(300));
            assert_eq!(policy.note.as_deref(), Some("summarize later"));
        }
        _ => panic!("expected background tool kind"),
    }
}
