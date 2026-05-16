use std::path::{Component, Path, PathBuf};

use astrbot_core::{AstrbotError, Result};

use crate::path_config::RuntimePathLayout;

pub const DASHBOARD_INDEX_ROUTES: &[&str] = &[
    "/",
    "/auth/login",
    "/config",
    "/logs",
    "/extension",
    "/dashboard/default",
    "/alkaid",
    "/alkaid/knowledge-base",
    "/alkaid/long-term-memory",
    "/alkaid/other",
    "/console",
    "/chat",
    "/settings",
    "/platforms",
    "/providers",
    "/about",
    "/extension-marketplace",
    "/conversation",
    "/tool-use",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashboardAssetSource {
    Explicit,
    UserDist,
    BundledDist,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardAssetSelection {
    pub source: DashboardAssetSource,
    pub root_dir: PathBuf,
    pub webui_enabled: bool,
}

impl DashboardAssetSelection {
    pub fn index_file(&self) -> PathBuf {
        self.root_dir.join("index.html")
    }

    pub fn asset_path(&self, request_path: &str) -> Option<PathBuf> {
        if is_dashboard_index_route(request_path) {
            return Some(self.index_file());
        }
        safe_asset_path(&self.root_dir, request_path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardAssetPolicy {
    webui_enabled: bool,
    explicit_webui_dir: Option<PathBuf>,
    user_dist_dir: PathBuf,
    bundled_dist_dir: Option<PathBuf>,
}

impl DashboardAssetPolicy {
    pub fn from_layout(layout: &RuntimePathLayout) -> Self {
        Self {
            webui_enabled: true,
            explicit_webui_dir: None,
            user_dist_dir: layout.data_dir.join("dist"),
            bundled_dist_dir: None,
        }
    }

    pub fn new(user_dist_dir: impl Into<PathBuf>) -> Self {
        Self {
            webui_enabled: true,
            explicit_webui_dir: None,
            user_dist_dir: user_dist_dir.into(),
            bundled_dist_dir: None,
        }
    }

    pub fn enable_webui(mut self, enabled: bool) -> Self {
        self.webui_enabled = enabled;
        self
    }

    pub fn with_explicit_webui_dir(mut self, webui_dir: impl Into<PathBuf>) -> Self {
        self.explicit_webui_dir = Some(webui_dir.into());
        self
    }

    pub fn with_bundled_dist_dir(mut self, bundled_dist_dir: impl Into<PathBuf>) -> Self {
        self.bundled_dist_dir = Some(bundled_dist_dir.into());
        self
    }

    pub fn select(&self) -> DashboardAssetSelection {
        if let Some(explicit) = self
            .explicit_webui_dir
            .as_ref()
            .filter(|path| path.exists())
        {
            return DashboardAssetSelection {
                source: DashboardAssetSource::Explicit,
                root_dir: explicit.clone(),
                webui_enabled: self.webui_enabled,
            };
        }
        if self.user_dist_dir.exists() {
            return DashboardAssetSelection {
                source: DashboardAssetSource::UserDist,
                root_dir: self.user_dist_dir.clone(),
                webui_enabled: self.webui_enabled,
            };
        }
        if let Some(bundled) = self.bundled_dist_dir.as_ref().filter(|path| path.exists()) {
            return DashboardAssetSelection {
                source: DashboardAssetSource::BundledDist,
                root_dir: bundled.clone(),
                webui_enabled: self.webui_enabled,
            };
        }

        DashboardAssetSelection {
            source: DashboardAssetSource::UserDist,
            root_dir: self.user_dist_dir.clone(),
            webui_enabled: self.webui_enabled,
        }
    }

    pub fn validate(&self) -> Result<DashboardAssetSelection> {
        let selection = self.select();
        if selection.webui_enabled && !selection.index_file().is_file() {
            return Err(AstrbotError::Pipeline(format!(
                "dashboard static assets not found: {}",
                selection.index_file().display()
            )));
        }
        Ok(selection)
    }
}

pub fn is_dashboard_index_route(request_path: &str) -> bool {
    let normalized = normalize_route(request_path);
    DASHBOARD_INDEX_ROUTES
        .iter()
        .any(|route| *route == normalized)
}

fn normalize_route(request_path: &str) -> String {
    let mut normalized = request_path.trim();
    if normalized.is_empty() {
        return "/".to_string();
    }
    if let Some(stripped) = normalized.strip_suffix('/') {
        normalized = stripped;
    }
    if normalized.is_empty() {
        return "/".to_string();
    }
    if normalized.starts_with('/') {
        normalized.to_string()
    } else {
        format!("/{normalized}")
    }
}

fn safe_asset_path(root_dir: &Path, request_path: &str) -> Option<PathBuf> {
    let request_path = request_path.trim().trim_start_matches('/');
    if request_path.is_empty() {
        return Some(root_dir.join("index.html"));
    }
    let path = Path::new(request_path);
    if path.is_absolute() {
        return None;
    }

    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    (!relative.as_os_str().is_empty()).then(|| root_dir.join(relative))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{DashboardAssetPolicy, DashboardAssetSource, is_dashboard_index_route};

    #[test]
    fn dashboard_asset_policy_selects_explicit_user_then_bundled_dist() {
        let root = temp_dashboard_asset_root("select");
        let explicit = root.join("explicit");
        let user_dist = root.join("data-dist");
        let bundled = root.join("bundled-dist");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&user_dist).expect("user dist should create");
        fs::create_dir_all(&bundled).expect("bundled dist should create");
        fs::write(user_dist.join("index.html"), "user").expect("user index should write");
        fs::write(bundled.join("index.html"), "bundled").expect("bundled index should write");

        let policy = DashboardAssetPolicy::new(&user_dist)
            .with_explicit_webui_dir(&explicit)
            .with_bundled_dist_dir(&bundled);
        assert_eq!(policy.select().source, DashboardAssetSource::UserDist);

        fs::create_dir_all(&explicit).expect("explicit dist should create");
        fs::write(explicit.join("index.html"), "explicit").expect("explicit index should write");
        assert_eq!(policy.select().source, DashboardAssetSource::Explicit);
        assert!(policy.validate().is_ok());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dashboard_asset_policy_maps_spa_routes_and_rejects_traversal() {
        let selection = DashboardAssetPolicy::new("data/dist").select();

        assert!(is_dashboard_index_route("/chat"));
        assert_eq!(
            selection.asset_path("/chat"),
            Some(PathBuf::from("data/dist/index.html"))
        );
        assert_eq!(
            selection.asset_path("/assets/app.js"),
            Some(PathBuf::from("data/dist/assets/app.js"))
        );
        assert_eq!(selection.asset_path("/../secret.txt"), None);
    }

    #[test]
    fn dashboard_asset_validation_allows_disabled_webui_without_index() {
        let selection = DashboardAssetPolicy::new("missing/dist")
            .enable_webui(false)
            .validate()
            .expect("disabled webui should not require index");

        assert!(!selection.webui_enabled);
    }

    fn temp_dashboard_asset_root(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "astrbot-dashboard-assets-{}-{suffix}",
            std::process::id()
        ))
    }
}
