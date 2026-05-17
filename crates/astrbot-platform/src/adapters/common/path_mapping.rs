#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformPathMapping {
    from: String,
    to: String,
}

impl PlatformPathMapping {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: normalize_prefix(from),
            to: normalize_prefix(to),
        }
    }

    pub fn parse(mapping: &str) -> Option<Self> {
        let (from, to) = mapping.split_once(':')?;
        let from = from.trim();
        let to = to.trim();
        (!from.is_empty()).then(|| Self::new(from, to))
    }

    pub fn apply(&self, path: &str) -> Option<String> {
        let path = strip_file_scheme(path);
        path.starts_with(&self.from)
            .then(|| format!("{}{}", self.to, &path[self.from.len()..]))
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn to(&self) -> &str {
        &self.to
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformPathMappingRules {
    mappings: Vec<PlatformPathMapping>,
}

impl PlatformPathMappingRules {
    pub fn new<I>(mappings: I) -> Self
    where
        I: IntoIterator<Item = PlatformPathMapping>,
    {
        Self {
            mappings: mappings.into_iter().collect(),
        }
    }

    pub fn from_strings<I, S>(mappings: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new(
            mappings
                .into_iter()
                .filter_map(|mapping| PlatformPathMapping::parse(mapping.as_ref())),
        )
    }

    pub fn map_path(&self, path: &str) -> Option<String> {
        self.mappings.iter().find_map(|mapping| mapping.apply(path))
    }

    pub fn mappings(&self) -> &[PlatformPathMapping] {
        &self.mappings
    }
}

fn normalize_prefix(value: impl Into<String>) -> String {
    value
        .into()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

fn strip_file_scheme(path: &str) -> &str {
    path.strip_prefix("file://").unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{PlatformPathMapping, PlatformPathMappingRules};

    #[test]
    fn platform_path_mapping_rules_rewrite_record_and_image_urls() {
        let rules = PlatformPathMappingRules::new([PlatformPathMapping::new(
            "/host/uploads",
            "/container/uploads",
        )]);

        assert_eq!(
            rules.map_path("file:///host/uploads/a.png").as_deref(),
            Some("/container/uploads/a.png")
        );
        assert_eq!(rules.map_path("/other/a.png"), None);
    }

    #[test]
    fn platform_path_mapping_rules_parse_colon_strings() {
        let rules = PlatformPathMappingRules::from_strings(["/host:/container", "invalid"]);

        assert_eq!(rules.mappings().len(), 1);
        assert_eq!(rules.mappings()[0].from(), "/host");
        assert_eq!(rules.mappings()[0].to(), "/container");
    }
}
