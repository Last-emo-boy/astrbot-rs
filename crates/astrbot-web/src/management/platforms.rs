use astrbot_platform::PlatformManager;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformManagementResponse {
    pub platform_count: usize,
    pub platform_ids: Vec<String>,
    pub mock_platform_count: usize,
    pub webchat_platform_count: usize,
    pub onebot_platform_count: usize,
    pub recording_sink_count: usize,
}

impl PlatformManagementResponse {
    pub fn from_manager(manager: &PlatformManager) -> Self {
        Self {
            platform_count: manager.platform_count(),
            platform_ids: manager.platform_ids(),
            mock_platform_count: manager.mock_platform_count(),
            webchat_platform_count: manager.webchat_platform_count(),
            onebot_platform_count: manager.onebot_platform_count(),
            recording_sink_count: manager.recording_sink_count(),
        }
    }
}
