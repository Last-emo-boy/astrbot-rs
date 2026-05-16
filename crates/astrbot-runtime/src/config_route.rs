use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

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
