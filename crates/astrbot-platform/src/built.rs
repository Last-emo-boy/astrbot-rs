use std::sync::Arc;

use crate::{MessageRecorder, MockPlatform, OneBotPlatform, PlatformAdapter, WebChatPlatform};

pub struct BuiltPlatform {
    pub(crate) adapter: Arc<dyn PlatformAdapter>,
    pub(crate) recording_sink: Option<Arc<dyn MessageRecorder>>,
    pub(crate) mock_platform: Option<Arc<MockPlatform>>,
    pub(crate) webchat_platform: Option<Arc<WebChatPlatform>>,
    pub(crate) onebot_platform: Option<Arc<OneBotPlatform>>,
}

impl BuiltPlatform {
    pub fn new(adapter: Arc<dyn PlatformAdapter>) -> Self {
        Self {
            adapter,
            recording_sink: None,
            mock_platform: None,
            webchat_platform: None,
            onebot_platform: None,
        }
    }

    pub fn mock(platform: Arc<MockPlatform>) -> Self {
        let adapter: Arc<dyn PlatformAdapter> = platform.clone();
        Self {
            adapter,
            recording_sink: Some(platform.sink()),
            mock_platform: Some(platform),
            webchat_platform: None,
            onebot_platform: None,
        }
    }

    pub fn with_recording_sink(
        adapter: Arc<dyn PlatformAdapter>,
        recording_sink: Arc<dyn MessageRecorder>,
    ) -> Self {
        Self {
            adapter,
            recording_sink: Some(recording_sink),
            mock_platform: None,
            webchat_platform: None,
            onebot_platform: None,
        }
    }

    pub fn webchat(platform: Arc<WebChatPlatform>) -> Self {
        let adapter: Arc<dyn PlatformAdapter> = platform.clone();
        Self {
            adapter,
            recording_sink: Some(platform.sink()),
            mock_platform: None,
            webchat_platform: Some(platform),
            onebot_platform: None,
        }
    }

    pub fn onebot(platform: Arc<OneBotPlatform>) -> Self {
        let adapter: Arc<dyn PlatformAdapter> = platform.clone();
        Self {
            adapter,
            recording_sink: Some(platform.sink()),
            mock_platform: None,
            webchat_platform: None,
            onebot_platform: Some(platform),
        }
    }
}
