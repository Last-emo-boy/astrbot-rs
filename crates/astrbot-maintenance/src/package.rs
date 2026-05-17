use astrbot_plugin::{PluginDependency, PluginDependencyKind, PluginDependencyPlan};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenancePackageInstallRequest {
    pub package: Option<String>,
    pub requirements_path: Option<String>,
    pub mirror: Option<String>,
}

impl MaintenancePackageInstallRequest {
    pub fn package(package: impl Into<String>) -> Self {
        Self {
            package: Some(package.into()),
            requirements_path: None,
            mirror: None,
        }
    }

    pub fn requirements(requirements_path: impl Into<String>) -> Self {
        Self {
            package: None,
            requirements_path: Some(requirements_path.into()),
            mirror: None,
        }
    }

    pub fn with_mirror(mut self, mirror: impl Into<String>) -> Self {
        let mirror = mirror.into();
        self.mirror = (!mirror.trim().is_empty()).then_some(mirror);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenancePluginDependencyPlanDescriptor {
    pub plugin_id: String,
    pub dependencies: Vec<MaintenancePluginDependencyDescriptor>,
}

impl From<&PluginDependencyPlan> for MaintenancePluginDependencyPlanDescriptor {
    fn from(plan: &PluginDependencyPlan) -> Self {
        Self {
            plugin_id: plan.plugin_id().to_string(),
            dependencies: plan
                .dependencies()
                .iter()
                .map(MaintenancePluginDependencyDescriptor::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenancePluginDependencyDescriptor {
    pub kind: String,
    pub name: String,
    pub version_req: Option<String>,
    pub optional: bool,
}

impl From<&PluginDependency> for MaintenancePluginDependencyDescriptor {
    fn from(dependency: &PluginDependency) -> Self {
        Self {
            kind: match dependency.kind {
                PluginDependencyKind::RustCrate => "rust_crate",
                PluginDependencyKind::PythonPackage => "python_package",
                PluginDependencyKind::SystemPackage => "system_package",
                PluginDependencyKind::Binary => "binary",
            }
            .to_string(),
            name: dependency.name.clone(),
            version_req: dependency.version_req.clone(),
            optional: dependency.optional,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenancePackageInstallPlan {
    pub request: MaintenancePackageInstallRequest,
    pub plugin_dependency_plan: Option<MaintenancePluginDependencyPlanDescriptor>,
    pub global_runtime_install: bool,
}

impl MaintenancePackageInstallPlan {
    pub fn global(request: MaintenancePackageInstallRequest) -> Self {
        Self {
            request,
            plugin_dependency_plan: None,
            global_runtime_install: true,
        }
    }

    pub fn plugin_dependency(plugin_id: impl Into<String>, package: impl Into<String>) -> Self {
        let dependency = PluginDependency::new(PluginDependencyKind::PythonPackage, package);
        let plugin_dependency_plan =
            PluginDependencyPlan::new(plugin_id).with_dependency(dependency);
        Self {
            request: MaintenancePackageInstallRequest {
                package: None,
                requirements_path: None,
                mirror: None,
            },
            plugin_dependency_plan: Some((&plugin_dependency_plan).into()),
            global_runtime_install: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MaintenancePackageInstallPlan, MaintenancePackageInstallRequest};

    #[test]
    fn global_package_install_plan_stays_separate_from_plugin_dependency_plan() {
        let plan = MaintenancePackageInstallPlan::global(
            MaintenancePackageInstallRequest::package("requests==2.32.0")
                .with_mirror("https://mirror.example/simple"),
        );

        assert!(plan.global_runtime_install);
        assert!(plan.plugin_dependency_plan.is_none());
        assert_eq!(plan.request.package.as_deref(), Some("requests==2.32.0"));
    }

    #[test]
    fn plugin_dependency_install_plan_is_not_global_runtime_maintenance() {
        let plan = MaintenancePackageInstallPlan::plugin_dependency("weather", "httpx");

        assert!(!plan.global_runtime_install);
        assert_eq!(
            plan.plugin_dependency_plan
                .as_ref()
                .expect("plugin plan")
                .plugin_id,
            "weather"
        );
    }
}
