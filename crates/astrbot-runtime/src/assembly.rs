use std::sync::Arc;

use astrbot_core::{MessageEvent, MessageEventResult, Result};
use astrbot_platform::{PlatformBuildContext, PlatformConfig, PlatformManager, PlatformRegistry};
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PermissionFilter, PermissionLevel, PermissionScope,
    PluginControl, PluginEventType, PluginHandler, PluginRegistry, RegisteredHandler,
};
use astrbot_provider::{ProviderManager, ProviderRegistry};
use astrbot_tool::{
    CommandDescriptor, CommandPermission, CommandType, builtin_command_descriptors,
};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::RuntimeConfig;
use crate::provider_selection::provider_manager_config_set;
pub(crate) fn build_platform_manager(
    config: &RuntimeConfig,
    event_tx: mpsc::Sender<MessageEvent>,
) -> Result<PlatformManager> {
    let registry = PlatformRegistry::with_builtin_platforms();
    let platform_configs = config
        .platforms
        .clone()
        .into_iter()
        .map(PlatformConfig::from);

    PlatformManager::from_configs(
        &registry,
        platform_configs,
        PlatformBuildContext::new(event_tx),
    )
}

pub(crate) fn build_provider_manager(config: &RuntimeConfig) -> Result<ProviderManager> {
    let registry = ProviderRegistry::with_builtin_providers();

    ProviderManager::from_configs(&registry, provider_manager_config_set(config))
}

pub(crate) fn build_plugin_registry(config: &RuntimeConfig) -> Arc<PluginRegistry> {
    let mut registry = PluginRegistry::new();
    register_builtin_command_handlers(&mut registry);
    for command in config
        .command_plugins
        .iter()
        .filter(|command| command.enabled)
    {
        let mut registered = RegisteredHandler::new(
            HandlerMetadata::new(
                command.plugin_name.clone(),
                command.handler_name.clone(),
                PluginEventType::AdapterMessage,
            )
            .with_priority(command.priority),
            Arc::new(StaticReplyHandler {
                response: command.response.clone(),
            }),
        )
        .with_filter(CommandFilter::new(command.command.clone()));
        if let Some(permission_filter) = permission_filter(command.permission) {
            registered = registered.with_filter(permission_filter);
        }
        registry.register_handler(registered);
    }
    Arc::new(registry)
}

fn register_builtin_command_handlers(registry: &mut PluginRegistry) {
    for descriptor in builtin_command_descriptors()
        .into_iter()
        .filter(CommandDescriptor::is_executable)
    {
        let mut registered = RegisteredHandler::new(
            HandlerMetadata::new(
                descriptor.plugin_name.clone(),
                descriptor.handler_name().to_string(),
                PluginEventType::AdapterMessage,
            )
            .with_priority(100)
            .with_description(descriptor.description.clone()),
            Arc::new(BuiltinCommandHandler {
                descriptor: descriptor.clone(),
            }),
        )
        .with_filter(command_filter(&descriptor));
        if let Some(permission_filter) = permission_filter(descriptor.permission) {
            registered = registered.with_filter(permission_filter);
        }
        registry.register_handler(registered);
    }
}

fn command_filter(descriptor: &CommandDescriptor) -> CommandFilter {
    let filter = match descriptor.command_type {
        CommandType::Command => CommandFilter::new(descriptor.current_fragment.clone()),
        CommandType::Group => CommandFilter::group(descriptor.current_fragment.clone()),
        CommandType::SubCommand => CommandFilter::sub_command(
            descriptor.parent_signature.clone(),
            descriptor.current_fragment.clone(),
        ),
    };
    filter.with_aliases(descriptor.aliases.clone())
}

fn permission_filter(permission: CommandPermission) -> Option<PermissionFilter> {
    let scope = PermissionScope::new();
    match permission {
        CommandPermission::Everyone => None,
        CommandPermission::Member => Some(PermissionFilter::new(PermissionLevel::Member, scope)),
        CommandPermission::Admin => Some(PermissionFilter::admin(scope)),
    }
}

struct BuiltinCommandHandler {
    descriptor: CommandDescriptor,
}

#[async_trait]
impl PluginHandler for BuiltinCommandHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(builtin_command_response(
            &self.descriptor,
            event,
        )));
        Ok(PluginControl::Continue)
    }
}

struct StaticReplyHandler {
    response: String,
}

#[async_trait]
impl PluginHandler for StaticReplyHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(self.response.clone()));
        Ok(PluginControl::Continue)
    }
}

fn builtin_command_response(descriptor: &CommandDescriptor, event: &MessageEvent) -> String {
    match descriptor.handler_name() {
        "help" => format!(
            "可用命令: {}",
            builtin_command_descriptors()
                .into_iter()
                .filter(CommandDescriptor::is_executable)
                .map(|command| format!("/{}", command.effective_command()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "sid" => format!(
            "Session ID: {}\nSender ID: {}\nPlatform ID: {}",
            event.session.conversation_id, event.sender.id, event.platform_id
        ),
        "plugin_ls" => "已安装插件: builtin_commands".to_string(),
        "plugin_help" => "builtin_commands 提供 help、llm、plugin、provider、conversation、persona、tts/t2i 和管理命令。".to_string(),
        "alter_cmd" => "命令权限和重命名可在 Dashboard 命令管理中调整。".to_string(),
        "set_variable" => command_ack(descriptor, "会话变量设置请求已接收。"),
        "unset_variable" => command_ack(descriptor, "会话变量删除请求已接收。"),
        "reset" => command_ack(descriptor, "当前会话重置请求已接收。"),
        "stop" => command_ack(descriptor, "当前会话停止请求已接收。"),
        "history" => command_ack(descriptor, "对话历史查询请求已接收。"),
        "ls" => command_ack(descriptor, "对话列表查询请求已接收。"),
        "new" => command_ack(descriptor, "新建对话请求已接收。"),
        "switch" => command_ack(descriptor, "切换对话请求已接收。"),
        "rename" => command_ack(descriptor, "重命名对话请求已接收。"),
        "del" => command_ack(descriptor, "删除对话请求已接收。"),
        "t2i" => command_ack(descriptor, "文本转图片开关请求已接收。"),
        "tts" => command_ack(descriptor, "文本转语音开关请求已接收。"),
        _ => command_ack(
            descriptor,
            &format!("内置命令 /{} 已接收。", descriptor.effective_command()),
        ),
    }
}

fn command_ack(descriptor: &CommandDescriptor, message: &str) -> String {
    format!("/{}: {message}", descriptor.effective_command())
}
