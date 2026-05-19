mod dependency;
mod hot_reload;
mod lifecycle;
mod metadata;
mod store;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use astrbot_tool::{ToolCatalog, ToolDescriptor};

pub use dependency::{
    DependencyConflictKind, DependencyConflictReport, DependencyErrorRedactor,
    DependencyInstallOutcome, DependencyInstallRequest, DependencyInstallStatus,
    NoopDependencyInstaller, PackagePreferencePolicy, PluginDependency, PluginDependencyInstaller,
    PluginDependencyKind, PluginDependencyPlan, PluginDependencyPlanInstaller,
    PluginImportEnvironment, PluginRuntimeKind, RecordingDependencyInstaller,
    RuntimeImportBehavior,
};
pub use hot_reload::{HotReloadDecision, PluginFileChange, PluginFileChangeKind, plan_hot_reload};
pub use lifecycle::{PluginLifecycleAction, PluginLifecycleEvent, PluginLifecycleState};
pub use metadata::{PluginLoadSource, PluginLoadSourceKind, PluginMetadata};
pub use store::{InMemoryPluginStore, PluginRecord, PluginStateStore};

use crate::manifest::PluginManifest;
use crate::registry::PluginRegistry;
use crate::sdk::{PLUGIN_SDK_VERSION, PluginContext, PluginModule};
use crate::tool::PluginToolDeclaration;
use crate::{PluginPlatformExtension, PluginWebApiRoute};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginRuntimeRegistration {
    pub plugin_id: String,
    pub handler_count: usize,
    pub tool_names: Vec<String>,
    pub web_routes: Vec<PluginWebApiRoute>,
    pub platform_extensions: Vec<PluginPlatformExtension>,
}

impl PluginRuntimeRegistration {
    fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            handler_count: 0,
            tool_names: Vec::new(),
            web_routes: Vec::new(),
            platform_extensions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginRuntimeEvent {
    pub plugin_id: String,
    pub event_type: crate::PluginEventType,
    pub message: Option<String>,
}

impl PluginRuntimeEvent {
    fn new(plugin_id: impl Into<String>, event_type: crate::PluginEventType) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            event_type,
            message: None,
        }
    }

    fn error(plugin_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            event_type: crate::PluginEventType::OnPluginError,
            message: Some(message.into()),
        }
    }
}

pub struct PluginLoader<S = InMemoryPluginStore> {
    store: S,
    dependency_installer: Arc<dyn PluginDependencyPlanInstaller>,
    registrations: HashMap<String, PluginRuntimeRegistration>,
    modules: HashMap<String, Arc<dyn PluginModule>>,
    runtime_events: Vec<PluginRuntimeEvent>,
}

impl PluginLoader<InMemoryPluginStore> {
    pub fn new() -> Self {
        Self::with_store(InMemoryPluginStore::new())
    }
}

impl Default for PluginLoader<InMemoryPluginStore> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> PluginLoader<S>
where
    S: PluginStateStore,
{
    pub fn with_store(store: S) -> Self {
        Self {
            store,
            dependency_installer: Arc::new(NoopDependencyInstaller),
            registrations: HashMap::new(),
            modules: HashMap::new(),
            runtime_events: Vec::new(),
        }
    }

    pub fn with_dependency_installer<I>(mut self, dependency_installer: I) -> Self
    where
        I: PluginDependencyPlanInstaller + 'static,
    {
        self.dependency_installer = Arc::new(dependency_installer);
        self
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn dependency_installer(&self) -> &dyn PluginDependencyPlanInstaller {
        self.dependency_installer.as_ref()
    }

    pub fn registration(&self, plugin_id: &str) -> Option<&PluginRuntimeRegistration> {
        self.registrations.get(plugin_id)
    }

    pub fn runtime_events(&self) -> &[PluginRuntimeEvent] {
        &self.runtime_events
    }

    pub fn discover_manifest(
        &mut self,
        source: PluginLoadSource,
        manifest: PluginManifest,
    ) -> Result<PluginMetadata> {
        let metadata = PluginMetadata::from_manifest(source, manifest)?;
        let record = PluginRecord::new(metadata.clone(), PluginLifecycleState::Discovered);
        self.store.upsert(record);
        Ok(metadata)
    }

    pub fn discover_plugins_from_directory(
        &mut self,
        root_dir: impl AsRef<Path>,
        kind: PluginLoadSourceKind,
        reserved: bool,
    ) -> Result<Vec<PluginMetadata>> {
        let root_dir = root_dir.as_ref();
        if !root_dir.exists() {
            return Ok(Vec::new());
        }

        let mut plugin_dirs = fs::read_dir(root_dir)
            .map_err(|err| {
                AstrbotError::Pipeline(format!(
                    "failed to read plugin directory {}: {err}",
                    root_dir.display()
                ))
            })?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                file_type.is_dir().then_some(entry.path())
            })
            .collect::<Vec<_>>();
        plugin_dirs.sort();

        let mut discovered = Vec::new();
        for plugin_dir in plugin_dirs {
            let metadata_path = plugin_dir.join("metadata.yaml");
            if !metadata_path.exists() {
                continue;
            }

            let file = read_metadata_yaml(&metadata_path)?;
            let dir_name = plugin_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("plugin");
            let mut source = PluginLoadSource::new(kind, dir_name).with_root_dir(&plugin_dir);
            if reserved {
                source = source.reserved();
            }

            let mut metadata = PluginMetadata::from_manifest(source, file.manifest)?;
            for platform in file.supported_platforms {
                metadata = metadata.with_supported_platform(platform);
            }
            if let Some(requirement) = file.runtime_version_requirement {
                metadata = metadata.with_runtime_version(requirement);
            }
            validate_metadata_runtime_version(&metadata)?;
            self.store.upsert(PluginRecord::new(
                metadata.clone(),
                PluginLifecycleState::Discovered,
            ));
            discovered.push(metadata);
        }

        Ok(discovered)
    }

    pub fn mark_loaded(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Load)
    }

    pub fn activate(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Activate)
    }

    pub fn disable(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Disable)
    }

    pub fn reload(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Reload)
    }

    pub fn unload(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Unload)
    }

    pub fn mark_failed(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Fail)
    }

    pub fn context_for(&self, plugin_id: &str) -> Option<PluginContext> {
        self.store()
            .get(plugin_id)
            .map(|record| PluginContext::from_manifest(&record.metadata.manifest))
    }

    pub async fn load_module(
        &mut self,
        source: PluginLoadSource,
        module: Arc<dyn PluginModule>,
        registry: &mut PluginRegistry,
        tool_catalog: &mut ToolCatalog,
    ) -> Result<PluginLifecycleEvent> {
        let mut metadata = PluginMetadata::from_manifest(source, module.manifest().clone())?;
        if let Some(requirement) = module.runtime_version_requirement() {
            metadata = metadata.with_runtime_version(requirement);
        }
        validate_metadata_runtime_version(&metadata)?;

        let plugin_id = metadata.plugin_id().to_string();
        let source = metadata.source.clone();
        self.store.upsert(PluginRecord::new(
            metadata.clone(),
            PluginLifecycleState::Discovered,
        ));

        let dependency_plan = dependency_plan_for_module(&plugin_id, &source, module.as_ref())?;
        if !dependency_plan.is_empty() {
            let outcome = self
                .ensure_dependencies(dependency_plan, import_environment(&source, &plugin_id))
                .await;
            match outcome {
                Ok(outcome) if outcome.is_success() => {}
                Ok(outcome) => {
                    let message = format!("plugin {plugin_id} dependency installation failed");
                    self.record_plugin_error(&plugin_id, message.clone())?;
                    return Err(AstrbotError::Pipeline(format!(
                        "{message}: {:?}",
                        outcome.conflicts()
                    )));
                }
                Err(err) => {
                    let message = err.to_string();
                    self.record_plugin_error(&plugin_id, message.clone())?;
                    return Err(err);
                }
            }
        }

        let context = PluginContext::from_manifest(&metadata.manifest);
        if let Err(err) = module.on_load(&context).await {
            let message = err.to_string();
            self.record_plugin_error(&plugin_id, message)?;
            return Err(err);
        }

        let mut registration = PluginRuntimeRegistration::new(&plugin_id);
        let handlers = module.handlers(&context);
        registration.handler_count = handlers.len();
        for handler in handlers {
            registry.register_handler(handler);
        }

        for declaration in module.tools(&context) {
            let descriptor =
                descriptor_from_tool_declaration(&plugin_id, &metadata.manifest.name, &declaration);
            registration.tool_names.push(descriptor.name.clone());
            tool_catalog.add_tool(descriptor);
        }

        registration.web_routes = module.web_routes(&context);
        registration.platform_extensions = module.platform_extensions(&context);
        self.registrations.insert(plugin_id.clone(), registration);
        self.modules.insert(plugin_id.clone(), module);

        self.mark_loaded(&plugin_id)?;
        let activated = self.activate(&plugin_id)?;
        self.runtime_events.push(PluginRuntimeEvent::new(
            &plugin_id,
            crate::PluginEventType::OnPluginLoaded,
        ));
        Ok(activated)
    }

    pub async fn reload_module(
        &mut self,
        source: PluginLoadSource,
        module: Arc<dyn PluginModule>,
        registry: &mut PluginRegistry,
        tool_catalog: &mut ToolCatalog,
    ) -> Result<PluginLifecycleEvent> {
        let metadata = PluginMetadata::from_manifest(source.clone(), module.manifest().clone())?;
        let plugin_id = metadata.plugin_id().to_string();
        if self.store.get(&plugin_id).is_some() {
            self.unload_plugin(&plugin_id, registry, tool_catalog)
                .await?;
        }
        self.load_module(source, module, registry, tool_catalog)
            .await
    }

    pub async fn unload_plugin(
        &mut self,
        plugin_id: &str,
        registry: &mut PluginRegistry,
        tool_catalog: &mut ToolCatalog,
    ) -> Result<PluginLifecycleEvent> {
        let record = self.store.get(plugin_id).cloned().ok_or_else(|| {
            AstrbotError::Pipeline(format!("plugin {plugin_id} is not discovered"))
        })?;
        let context = PluginContext::from_manifest(&record.metadata.manifest);
        let module = self.modules.remove(plugin_id);
        let mut unload_error = None;
        if let Some(module) = module {
            if let Err(err) = module.on_unload(&context).await {
                unload_error = Some(err);
            }
        }

        registry.unregister_plugin(plugin_id).await?;
        if record.metadata.manifest.name != plugin_id {
            registry
                .unregister_plugin(&record.metadata.manifest.name)
                .await?;
        }
        tool_catalog.remove_tools_by_plugin(plugin_id);
        self.registrations.remove(plugin_id);

        let event = self.unload(plugin_id)?;
        self.runtime_events.push(PluginRuntimeEvent::new(
            plugin_id,
            crate::PluginEventType::OnPluginUnloaded,
        ));

        if let Some(err) = unload_error {
            let message = err.to_string();
            self.record_plugin_error(plugin_id, message)?;
            return Err(err);
        }

        Ok(event)
    }

    pub async fn ensure_dependencies(
        &self,
        plan: PluginDependencyPlan,
        environment: PluginImportEnvironment,
    ) -> Result<DependencyInstallOutcome> {
        self.dependency_installer
            .install_dependencies(DependencyInstallRequest::new(plan, environment))
            .await
    }

    fn transition(
        &mut self,
        plugin_id: &str,
        action: PluginLifecycleAction,
    ) -> Result<PluginLifecycleEvent> {
        let record = self.store.get(plugin_id).cloned().ok_or_else(|| {
            astrbot_core::AstrbotError::Pipeline(format!("plugin {plugin_id} is not discovered"))
        })?;
        let previous = record.state;
        let next = action.next_state(previous);
        self.store.set_state(plugin_id, next)?;
        Ok(PluginLifecycleEvent::new(
            record.metadata.plugin_id().to_string(),
            action,
            previous,
            next,
        ))
    }

    fn record_plugin_error(&mut self, plugin_id: &str, message: String) -> Result<()> {
        if self.store.get(plugin_id).is_some() {
            self.mark_failed(plugin_id)?;
        }
        self.runtime_events
            .push(PluginRuntimeEvent::error(plugin_id, message));
        Ok(())
    }
}

struct MetadataFile {
    manifest: PluginManifest,
    supported_platforms: Vec<String>,
    runtime_version_requirement: Option<String>,
}

fn read_metadata_yaml(path: &Path) -> Result<MetadataFile> {
    let content = fs::read_to_string(path).map_err(|err| {
        AstrbotError::Pipeline(format!(
            "failed to read plugin metadata {}: {err}",
            path.display()
        ))
    })?;

    let mut name = None;
    let mut version = None;
    let mut description = None;
    let mut authors = Vec::new();
    let mut supported_platforms = Vec::new();
    let mut runtime_version_requirement = None;
    let mut current_list_key: Option<&str> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(item) = line.strip_prefix("- ") {
            match current_list_key {
                Some("author") | Some("authors") => {
                    push_unique(&mut authors, clean_yaml_value(item))
                }
                Some("support_platforms") => {
                    push_unique(&mut supported_platforms, clean_yaml_value(item));
                }
                _ => {}
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = clean_yaml_value(value);
        current_list_key = value.is_empty().then_some(key);

        match key {
            "name" => name = non_empty(value),
            "version" => version = non_empty(value),
            "desc" | "description" => description = non_empty(value),
            "author" | "authors" => {
                if !value.is_empty() {
                    for author in value.trim_matches(['[', ']']).split(',') {
                        push_unique(&mut authors, clean_yaml_value(author));
                    }
                }
            }
            "support_platforms" => {
                if !value.is_empty() {
                    for platform in value.trim_matches(['[', ']']).split(',') {
                        push_unique(&mut supported_platforms, clean_yaml_value(platform));
                    }
                }
            }
            "astrbot_version" => runtime_version_requirement = non_empty(value),
            _ => {}
        }
    }

    let name = name.ok_or_else(|| {
        AstrbotError::Pipeline(format!(
            "plugin metadata {} is missing required name",
            path.display()
        ))
    })?;
    let version = version.ok_or_else(|| {
        AstrbotError::Pipeline(format!(
            "plugin metadata {} is missing required version",
            path.display()
        ))
    })?;

    let mut manifest = PluginManifest::new(name, version);
    if let Some(description) = description {
        manifest = manifest.with_description(description);
    }
    for author in authors {
        manifest = manifest.with_author(author);
    }

    Ok(MetadataFile {
        manifest,
        supported_platforms,
        runtime_version_requirement,
    })
}

fn dependency_plan_for_module(
    plugin_id: &str,
    source: &PluginLoadSource,
    module: &dyn PluginModule,
) -> Result<PluginDependencyPlan> {
    let plan = module.dependency_plan(plugin_id);
    if !plan.is_empty() {
        return Ok(plan);
    }
    let Some(root_dir) = source.root_dir() else {
        return Ok(plan);
    };
    read_requirements_plan(plugin_id, root_dir)
}

fn read_requirements_plan(plugin_id: &str, root_dir: &Path) -> Result<PluginDependencyPlan> {
    let requirements_path = root_dir.join("requirements.txt");
    if !requirements_path.exists() {
        return Ok(PluginDependencyPlan::new(plugin_id));
    }
    let content = fs::read_to_string(&requirements_path).map_err(|err| {
        AstrbotError::Pipeline(format!(
            "failed to read plugin requirements {}: {err}",
            requirements_path.display()
        ))
    })?;
    let mut plan = PluginDependencyPlan::new(plugin_id);
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let (name, version_req) = split_requirement(line);
        let mut dependency =
            PluginDependency::new(PluginDependencyKind::PythonPackage, name.to_string());
        if let Some(version_req) = version_req {
            dependency = dependency.with_version_req(version_req.to_string());
        }
        plan = plan.with_dependency(dependency);
    }
    Ok(plan)
}

fn split_requirement(line: &str) -> (&str, Option<&str>) {
    for operator in ["==", ">=", "<=", "~=", "!=", ">", "<"] {
        if let Some(index) = line.find(operator) {
            let name = line[..index].trim();
            let version_req = line[index..].trim();
            return (name, (!version_req.is_empty()).then_some(version_req));
        }
    }
    (line, None)
}

fn import_environment(source: &PluginLoadSource, plugin_id: &str) -> PluginImportEnvironment {
    let mut environment = match source.kind() {
        PluginLoadSourceKind::NativeRust => PluginImportEnvironment::native_rust(plugin_id),
        PluginLoadSourceKind::PythonCompat => PluginImportEnvironment::python_compat(plugin_id),
        PluginLoadSourceKind::Wasm => {
            PluginImportEnvironment::new(PluginRuntimeKind::Wasm, plugin_id)
        }
        PluginLoadSourceKind::ExternalProcess => {
            PluginImportEnvironment::new(PluginRuntimeKind::ExternalProcess, plugin_id)
        }
    };
    if let Some(root_dir) = source.root_dir() {
        environment = environment.with_plugin_root(root_dir.clone());
    }
    environment
}

fn descriptor_from_tool_declaration(
    plugin_id: &str,
    origin_name: &str,
    declaration: &PluginToolDeclaration,
) -> ToolDescriptor {
    let mut descriptor = ToolDescriptor::new(&declaration.name)
        .with_source_metadata(declaration.source_metadata(plugin_id, origin_name));
    if let Some(description) = &declaration.description {
        descriptor = descriptor.with_description(description);
    }
    descriptor
}

fn validate_metadata_runtime_version(metadata: &PluginMetadata) -> Result<()> {
    if let Some(requirement) = metadata.runtime_version() {
        validate_runtime_version_requirement(requirement, PLUGIN_SDK_VERSION).map_err(
            |message| {
                AstrbotError::Pipeline(format!(
                    "plugin {} requires incompatible AstrBot version {requirement}: {message}",
                    metadata.plugin_id()
                ))
            },
        )?;
    }
    Ok(())
}

fn validate_runtime_version_requirement(
    requirement: &str,
    current: &str,
) -> std::result::Result<(), String> {
    let requirement = requirement.trim().trim_matches(['"', '\'']);
    if requirement.is_empty() || requirement == "*" {
        return Ok(());
    }

    for clause in requirement.split(',') {
        let clause = clause.trim();
        if clause.is_empty() {
            continue;
        }
        let (operator, expected) = parse_version_clause(clause);
        let ordering = compare_versions(current, expected)?;
        let passed = match operator {
            ">=" => ordering != std::cmp::Ordering::Less,
            ">" => ordering == std::cmp::Ordering::Greater,
            "<=" => ordering != std::cmp::Ordering::Greater,
            "<" => ordering == std::cmp::Ordering::Less,
            "==" | "=" | "" => ordering == std::cmp::Ordering::Equal,
            other => return Err(format!("unsupported operator {other}")),
        };
        if !passed {
            return Err(format!(
                "current version {current} does not satisfy {clause}"
            ));
        }
    }
    Ok(())
}

fn parse_version_clause(clause: &str) -> (&str, &str) {
    for operator in [">=", "<=", "==", ">", "<", "="] {
        if let Some(version) = clause.strip_prefix(operator) {
            return (operator, version.trim());
        }
    }
    ("", clause)
}

fn compare_versions(left: &str, right: &str) -> std::result::Result<std::cmp::Ordering, String> {
    Ok(parse_version(left)?.cmp(&parse_version(right)?))
}

fn parse_version(value: &str) -> std::result::Result<[u64; 3], String> {
    let value = value.trim().trim_matches(['"', '\'']);
    let mut parts = [0, 0, 0];
    for (index, part) in value.split('.').take(3).enumerate() {
        let numeric = part
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if numeric.is_empty() {
            return Err(format!("invalid version {value}"));
        }
        parts[index] = numeric
            .parse::<u64>()
            .map_err(|_| format!("invalid version {value}"))?;
    }
    Ok(parts)
}

fn clean_yaml_value(value: &str) -> String {
    value.trim().trim_matches(['"', '\'']).trim().to_string()
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
