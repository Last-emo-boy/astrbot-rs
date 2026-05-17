pub const CONSOLE_PLATFORM_TYPE: &str = "console";
pub const WEBCHAT_PLATFORM_TYPE: &str = "webchat";
pub const ONEBOT_PLATFORM_TYPE: &str = "onebot";
pub const MOCK_PLATFORM_TYPE: &str = "mock";

pub struct PlatformConfig {
    pub id: String,
    pub platform_type: String,
    pub enabled: bool,
    pub name: Option<String>,
}

impl PlatformConfig {
    pub fn mock(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: MOCK_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn console(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: CONSOLE_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn webchat(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: WEBCHAT_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn onebot(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: ONEBOT_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}
