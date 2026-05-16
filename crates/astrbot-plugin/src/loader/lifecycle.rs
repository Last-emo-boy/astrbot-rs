#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginLifecycleState {
    Discovered,
    Loaded,
    Active,
    Disabled,
    Reloading,
    Unloaded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginLifecycleAction {
    Load,
    Activate,
    Disable,
    Reload,
    Unload,
    Fail,
}

impl PluginLifecycleAction {
    pub fn next_state(self, previous: PluginLifecycleState) -> PluginLifecycleState {
        match self {
            Self::Load => PluginLifecycleState::Loaded,
            Self::Activate => PluginLifecycleState::Active,
            Self::Disable => PluginLifecycleState::Disabled,
            Self::Reload => PluginLifecycleState::Reloading,
            Self::Unload => PluginLifecycleState::Unloaded,
            Self::Fail => {
                if previous == PluginLifecycleState::Unloaded {
                    PluginLifecycleState::Unloaded
                } else {
                    PluginLifecycleState::Failed
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginLifecycleEvent {
    pub plugin_id: String,
    pub action: PluginLifecycleAction,
    pub previous: PluginLifecycleState,
    pub next: PluginLifecycleState,
}

impl PluginLifecycleEvent {
    pub fn new(
        plugin_id: impl Into<String>,
        action: PluginLifecycleAction,
        previous: PluginLifecycleState,
        next: PluginLifecycleState,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            action,
            previous,
            next,
        }
    }
}
