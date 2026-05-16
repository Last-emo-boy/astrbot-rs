use crate::{
    DependencyConflictKind, DependencyConflictReport, DependencyErrorRedactor,
    DependencyInstallStatus, NoopDependencyInstaller, PackagePreferencePolicy, PluginDependency,
    PluginDependencyInstaller, PluginDependencyKind, PluginDependencyPlan, PluginImportEnvironment,
    PluginLoader, PluginRuntimeKind, RecordingDependencyInstaller,
};

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
