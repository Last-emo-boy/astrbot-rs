use std::fs;
use std::path::Path;

use crate::{McpRoot, McpRootAlias, McpRootResolver, McpRootsCapabilityConfig, McpUri};

#[test]
fn roots_keep_aliases_and_uri_typed() {
    let defaults = McpRootsCapabilityConfig::enabled_for_default_paths();
    assert_eq!(defaults.paths, vec!["data".to_string(), "temp".to_string()]);
    assert!(
        McpRootAlias::all()
            .iter()
            .any(|alias| alias.as_str() == "knowledge_base")
    );

    let root = McpRoot::new(McpUri::new("file:///tmp").expect("uri")).named("temp");
    let json = serde_json::to_value(root).expect("root should serialize");

    assert_eq!(json["uri"], "file:///tmp");
    assert_eq!(json["name"], "temp");
}

#[test]
fn roots_resolver_maps_aliases_to_allowlisted_file_roots() {
    let temp = temp_root("aliases");
    for path in [
        temp.join("data"),
        temp.join("data").join("config"),
        temp.join("data").join("plugins"),
        temp.join("data").join("plugin_data"),
        temp.join("data").join("temp"),
        temp.join("data").join("skills"),
        temp.join("data").join("knowledge_base"),
        temp.join("data").join("backups"),
    ] {
        fs::create_dir_all(path).expect("dir");
    }

    let roots = McpRootResolver::new(
        &temp,
        McpRootsCapabilityConfig {
            enabled: true,
            paths: McpRootAlias::all()
                .iter()
                .map(|alias| alias.as_str().to_string())
                .collect(),
        },
    )
    .resolve()
    .expect("roots should resolve");

    let names = roots
        .iter()
        .filter_map(|root| root.name.as_deref())
        .collect::<Vec<_>>();
    assert!(names.contains(&"root"));
    assert!(names.contains(&"knowledge_base"));
    assert!(
        roots
            .iter()
            .all(|root| root.uri.as_str().starts_with("file://"))
    );
}

#[test]
fn roots_resolver_exposes_skills_alias_to_data_skills_root() {
    let temp = temp_root("skills");
    let skills = temp.join("data").join("skills");
    fs::create_dir_all(&skills).expect("skills dir");

    let roots = McpRootResolver::new(
        &temp,
        McpRootsCapabilityConfig {
            enabled: true,
            paths: vec!["skills".to_string()],
        },
    )
    .resolve()
    .expect("skills root should resolve");

    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].name.as_deref(), Some("skills"));
    assert!(
        roots[0]
            .uri
            .as_str()
            .replace("%3A", ":")
            .replace('\\', "/")
            .ends_with("/data/skills")
    );
}

#[test]
fn roots_resolver_rejects_non_allowlisted_and_accepts_explicit_allowlist() {
    let temp = temp_root("allowlist");
    let outside = temp.join("outside");
    fs::create_dir_all(temp.join("data").join("temp")).expect("temp dir");
    fs::create_dir_all(&outside).expect("outside dir");

    let rejected = McpRootResolver::new(
        &temp,
        McpRootsCapabilityConfig {
            enabled: true,
            paths: vec![outside.to_string_lossy().to_string()],
        },
    )
    .resolve()
    .expect("resolver should not fail");
    assert!(rejected.is_empty());

    let accepted = McpRootResolver::new(
        &temp,
        McpRootsCapabilityConfig {
            enabled: true,
            paths: vec![outside.to_string_lossy().to_string()],
        },
    )
    .with_allowed_path(&outside)
    .resolve()
    .expect("allowlisted path should resolve");
    assert_eq!(accepted[0].name.as_deref(), Some("outside"));
}

#[cfg(unix)]
#[test]
fn roots_resolver_rejects_symlink_roots() {
    use std::os::unix::fs::symlink;

    let temp = temp_root("symlink");
    let real = temp.join("real");
    let link = temp.join("data").join("temp");
    fs::create_dir_all(&real).expect("real dir");
    fs::create_dir_all(temp.join("data")).expect("data dir");
    symlink(&real, &link).expect("symlink");

    let roots = McpRootResolver::new(
        &temp,
        McpRootsCapabilityConfig {
            enabled: true,
            paths: vec!["temp".to_string()],
        },
    )
    .resolve()
    .expect("resolver should not fail");
    assert!(roots.is_empty());
}

#[cfg(windows)]
#[test]
fn roots_resolver_rejects_symlink_roots() {
    use std::os::windows::fs::symlink_dir;

    let temp = temp_root("symlink");
    let real = temp.join("real");
    let link = temp.join("data").join("temp");
    fs::create_dir_all(&real).expect("real dir");
    fs::create_dir_all(temp.join("data")).expect("data dir");
    if symlink_dir(&real, &link).is_err() {
        return;
    }

    let roots = McpRootResolver::new(
        &temp,
        McpRootsCapabilityConfig {
            enabled: true,
            paths: vec!["temp".to_string()],
        },
    )
    .resolve()
    .expect("resolver should not fail");
    assert!(roots.is_empty());
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("astrbot-mcp-roots-{name}-{}", std::process::id()));
    if Path::new(&path).exists() {
        fs::remove_dir_all(&path).expect("clean temp root");
    }
    fs::create_dir_all(&path).expect("temp root");
    path
}
