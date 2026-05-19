use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UmopConfigRoute {
    pub pattern: String,
    pub config_id: String,
}

impl UmopConfigRoute {
    pub fn new(pattern: impl Into<String>, config_id: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            config_id: config_id.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UmopConfigRouter {
    routes: Vec<UmopConfigRoute>,
}

impl UmopConfigRouter {
    pub fn new(routes: Vec<UmopConfigRoute>) -> Result<Self> {
        validate_routes(&routes)?;
        Ok(Self { routes })
    }

    pub fn routes(&self) -> &[UmopConfigRoute] {
        &self.routes
    }

    pub fn from_routing_map(routing: BTreeMap<String, String>) -> Result<Self> {
        Self::new(
            routing
                .into_iter()
                .map(|(pattern, config_id)| UmopConfigRoute::new(pattern, config_id))
                .collect(),
        )
    }

    pub fn to_routing_map(&self) -> BTreeMap<String, String> {
        self.routes
            .iter()
            .map(|route| (route.pattern.clone(), route.config_id.clone()))
            .collect()
    }

    pub fn resolve_config_id(&self, umo: &str) -> Option<&str> {
        let target = UmopConfigRoutePattern::parse(umo).ok()?;
        self.routes.iter().find_map(|route| {
            let pattern = UmopConfigRoutePattern::parse(&route.pattern).ok()?;
            pattern.matches(&target).then_some(route.config_id.as_str())
        })
    }

    pub fn replace_routes(&mut self, routes: Vec<UmopConfigRoute>) -> Result<()> {
        validate_routes(&routes)?;
        self.routes = routes;
        Ok(())
    }

    pub fn set_route(
        &mut self,
        pattern: impl Into<String>,
        config_id: impl Into<String>,
    ) -> Result<()> {
        let route = UmopConfigRoute::new(pattern, config_id);
        UmopConfigRoutePattern::parse(&route.pattern)?;

        if let Some(existing) = self
            .routes
            .iter_mut()
            .find(|existing| existing.pattern == route.pattern)
        {
            *existing = route;
        } else {
            self.routes.push(route);
        }
        Ok(())
    }

    pub fn delete_route(&mut self, pattern: &str) -> Result<bool> {
        UmopConfigRoutePattern::parse(pattern)?;
        let before = self.routes.len();
        self.routes.retain(|route| route.pattern != pattern);
        Ok(self.routes.len() != before)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UmopConfigRouteStore {
    path: PathBuf,
}

impl UmopConfigRouteStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<UmopConfigRouter> {
        if !self.path.exists() {
            return Ok(UmopConfigRouter::default());
        }

        let content = fs::read_to_string(&self.path)
            .map_err(|err| AstrbotError::Pipeline(format!("read umop config routes: {err}")))?;
        if content.trim().is_empty() {
            return Ok(UmopConfigRouter::default());
        }
        let routes = parse_stored_routes(&content)?;
        UmopConfigRouter::new(routes)
    }

    pub fn save(&self, router: &UmopConfigRouter) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                AstrbotError::Pipeline(format!("create umop route config directory: {err}"))
            })?;
        }
        let payload = StoredUmopRoutes {
            routes: router.routes().to_vec(),
        };
        let serialized = serde_json::to_string_pretty(&payload).map_err(|err| {
            AstrbotError::Pipeline(format!("serialize umop config routes: {err}"))
        })?;
        fs::write(&self.path, serialized)
            .map_err(|err| AstrbotError::Pipeline(format!("write umop config routes: {err}")))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredUmopRoutes {
    routes: Vec<UmopConfigRoute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UmopConfigRoutePattern {
    platform_id: String,
    message_type: String,
    session_id: String,
}

impl UmopConfigRoutePattern {
    pub fn parse(value: &str) -> Result<Self> {
        let parts = value.splitn(3, ':').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(AstrbotError::Pipeline(
                "umop must use [platform_id]:[message_type]:[session_id] format".to_string(),
            ));
        }

        Ok(Self {
            platform_id: parts[0].to_string(),
            message_type: parts[1].to_string(),
            session_id: parts[2].to_string(),
        })
    }

    pub fn platform_id(&self) -> &str {
        &self.platform_id
    }

    pub fn message_type(&self) -> &str {
        &self.message_type
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn matches(&self, target: &Self) -> bool {
        component_matches(&self.platform_id, &target.platform_id)
            && component_matches(&self.message_type, &target.message_type)
            && component_matches(&self.session_id, &target.session_id)
    }
}

fn validate_routes(routes: &[UmopConfigRoute]) -> Result<()> {
    for route in routes {
        UmopConfigRoutePattern::parse(&route.pattern)?;
    }
    Ok(())
}

fn parse_stored_routes(content: &str) -> Result<Vec<UmopConfigRoute>> {
    let value = serde_json::from_str::<Value>(content)
        .map_err(|err| AstrbotError::Pipeline(format!("parse umop config routes: {err}")))?;

    if value.is_array() {
        return serde_json::from_value(value).map_err(|err| {
            AstrbotError::Pipeline(format!("parse umop config route array: {err}"))
        });
    }

    if value.get("routes").is_some() {
        let stored = serde_json::from_value::<StoredUmopRoutes>(value).map_err(|err| {
            AstrbotError::Pipeline(format!("parse umop config route catalog: {err}"))
        })?;
        return Ok(stored.routes);
    }

    let routing = serde_json::from_value::<BTreeMap<String, String>>(value)
        .map_err(|err| AstrbotError::Pipeline(format!("parse umop config route mapping: {err}")))?;
    Ok(routing
        .into_iter()
        .map(|(pattern, config_id)| UmopConfigRoute::new(pattern, config_id))
        .collect())
}

fn component_matches(pattern: &str, value: &str) -> bool {
    pattern.is_empty() || wildcard_match(pattern, value)
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();

    let mut pattern_index = 0;
    let mut value_index = 0;
    let mut star_index = None;
    let mut star_value_index = 0;

    while value_index < value.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == value[value_index] {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_value_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_value_index += 1;
            value_index = star_value_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}
