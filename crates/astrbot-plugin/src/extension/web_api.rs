#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginWebApiMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginWebApiRoute {
    pub plugin_id: String,
    pub route: String,
    pub methods: Vec<PluginWebApiMethod>,
    pub description: Option<String>,
}

impl PluginWebApiRoute {
    pub fn new(plugin_id: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            route: normalize_route(&route.into()),
            methods: vec![PluginWebApiMethod::Get],
            description: None,
        }
    }

    pub fn with_method(mut self, method: PluginWebApiMethod) -> Self {
        if !self.methods.contains(&method) {
            self.methods.push(method);
        }
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }
}

fn normalize_route(route: &str) -> String {
    let route = route.trim();
    if route.is_empty() {
        "/".to_string()
    } else if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    }
}
