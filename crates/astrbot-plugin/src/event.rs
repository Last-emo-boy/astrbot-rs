#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginEventType {
    OnAstrBotLoaded,
    OnPlatformLoaded,
    AdapterMessage,
    OnWaitingLlmRequest,
    OnLlmRequest,
    OnLlmResponse,
    OnDecoratingResult,
    OnCallingFuncTool,
    OnUsingLlmTool,
    OnLlmToolRespond,
    OnAfterMessageSent,
    OnPluginError,
    OnPluginLoaded,
    OnPluginUnloaded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginControl {
    Continue,
    Stop,
}
