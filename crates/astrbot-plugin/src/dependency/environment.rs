use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginRuntimeKind {
    NativeRust,
    PythonCompat,
    Wasm,
    ExternalProcess,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PackagePreferencePolicy {
    #[default]
    PreferIsolatedRoot,
    PreferInstalledSitePackages,
    DisableSitePackagesFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeImportBehavior {
    NativeRust,
    PythonModule,
    PackagedPython { patch_distribution_finder: bool },
    Wasm,
    ExternalProcess,
}

impl RuntimeImportBehavior {
    pub fn is_packaged_python(self) -> bool {
        matches!(self, Self::PackagedPython { .. })
    }

    pub fn patch_distribution_finder(self) -> bool {
        match self {
            Self::PackagedPython {
                patch_distribution_finder,
            } => patch_distribution_finder,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginImportEnvironment {
    pub plugin_id: String,
    pub runtime_kind: PluginRuntimeKind,
    plugin_root: Option<PathBuf>,
    isolated_dependency_root: Option<PathBuf>,
    site_packages_roots: Vec<PathBuf>,
    package_preference: PackagePreferencePolicy,
    runtime_behavior: RuntimeImportBehavior,
}

impl PluginImportEnvironment {
    pub fn new(runtime_kind: PluginRuntimeKind, plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            runtime_kind,
            plugin_root: None,
            isolated_dependency_root: None,
            site_packages_roots: Vec::new(),
            package_preference: PackagePreferencePolicy::default(),
            runtime_behavior: default_runtime_behavior(runtime_kind),
        }
    }

    pub fn native_rust(plugin_id: impl Into<String>) -> Self {
        Self::new(PluginRuntimeKind::NativeRust, plugin_id)
    }

    pub fn python_compat(plugin_id: impl Into<String>) -> Self {
        Self::new(PluginRuntimeKind::PythonCompat, plugin_id)
    }

    pub fn with_plugin_root(mut self, plugin_root: impl Into<PathBuf>) -> Self {
        self.plugin_root = Some(plugin_root.into());
        self
    }

    pub fn with_isolated_dependency_root(
        mut self,
        isolated_dependency_root: impl Into<PathBuf>,
    ) -> Self {
        self.isolated_dependency_root = Some(isolated_dependency_root.into());
        self
    }

    pub fn with_site_packages_root(mut self, site_packages_root: impl Into<PathBuf>) -> Self {
        push_unique_path(&mut self.site_packages_roots, site_packages_root.into());
        self
    }

    pub fn with_package_preference(mut self, package_preference: PackagePreferencePolicy) -> Self {
        self.package_preference = package_preference;
        self
    }

    pub fn prefer_installed_site_packages(self) -> Self {
        self.with_package_preference(PackagePreferencePolicy::PreferInstalledSitePackages)
    }

    pub fn disable_site_packages_fallback(self) -> Self {
        self.with_package_preference(PackagePreferencePolicy::DisableSitePackagesFallback)
    }

    pub fn packaged_python_runtime(mut self) -> Self {
        self.runtime_kind = PluginRuntimeKind::PythonCompat;
        self.runtime_behavior = RuntimeImportBehavior::PackagedPython {
            patch_distribution_finder: true,
        };
        self.package_preference = PackagePreferencePolicy::PreferInstalledSitePackages;
        self
    }

    pub fn plugin_root(&self) -> Option<&Path> {
        self.plugin_root.as_deref()
    }

    pub fn isolated_dependency_root(&self) -> Option<&Path> {
        self.isolated_dependency_root.as_deref()
    }

    pub fn site_packages_roots(&self) -> &[PathBuf] {
        &self.site_packages_roots
    }

    pub fn package_preference(&self) -> PackagePreferencePolicy {
        self.package_preference
    }

    pub fn runtime_behavior(&self) -> RuntimeImportBehavior {
        self.runtime_behavior
    }

    pub fn should_prefer_site_packages(&self) -> bool {
        matches!(
            self.package_preference,
            PackagePreferencePolicy::PreferInstalledSitePackages
        ) && !self.site_packages_roots.is_empty()
    }

    pub fn import_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        match self.package_preference {
            PackagePreferencePolicy::PreferInstalledSitePackages => {
                extend_unique_paths(&mut roots, self.site_packages_roots.iter().cloned());
                push_optional_path(&mut roots, self.isolated_dependency_root.clone());
                push_optional_path(&mut roots, self.plugin_root.clone());
            }
            PackagePreferencePolicy::PreferIsolatedRoot => {
                push_optional_path(&mut roots, self.isolated_dependency_root.clone());
                push_optional_path(&mut roots, self.plugin_root.clone());
                extend_unique_paths(&mut roots, self.site_packages_roots.iter().cloned());
            }
            PackagePreferencePolicy::DisableSitePackagesFallback => {
                push_optional_path(&mut roots, self.isolated_dependency_root.clone());
                push_optional_path(&mut roots, self.plugin_root.clone());
            }
        }
        roots
    }
}

fn default_runtime_behavior(runtime_kind: PluginRuntimeKind) -> RuntimeImportBehavior {
    match runtime_kind {
        PluginRuntimeKind::NativeRust => RuntimeImportBehavior::NativeRust,
        PluginRuntimeKind::PythonCompat => RuntimeImportBehavior::PythonModule,
        PluginRuntimeKind::Wasm => RuntimeImportBehavior::Wasm,
        PluginRuntimeKind::ExternalProcess => RuntimeImportBehavior::ExternalProcess,
    }
}

fn push_optional_path(paths: &mut Vec<PathBuf>, path: Option<PathBuf>) {
    if let Some(path) = path {
        push_unique_path(paths, path);
    }
}

fn extend_unique_paths(paths: &mut Vec<PathBuf>, values: impl IntoIterator<Item = PathBuf>) {
    for value in values {
        push_unique_path(paths, value);
    }
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|known| known == &path) {
        paths.push(path);
    }
}
