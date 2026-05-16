use serde::{Deserialize, Serialize};

use super::{PlatformManagementResponse, PluginManagementResponse, ProviderManagementResponse};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementStatusResponse {
    pub providers: ProviderManagementResponse,
    pub platforms: PlatformManagementResponse,
    pub plugins: PluginManagementResponse,
}

impl ManagementStatusResponse {
    pub fn new(
        providers: ProviderManagementResponse,
        platforms: PlatformManagementResponse,
        plugins: PluginManagementResponse,
    ) -> Self {
        Self {
            providers,
            platforms,
            plugins,
        }
    }
}
