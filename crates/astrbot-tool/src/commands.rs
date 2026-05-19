use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDescriptor {
    pub handler_full_name: String,
    pub plugin_name: String,
    pub description: String,
    pub command_type: CommandType,
    pub original_command: String,
    pub current_fragment: String,
    pub parent_signature: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub permission: CommandPermission,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub reserved: bool,
}

impl CommandDescriptor {
    pub fn new(
        handler_full_name: impl Into<String>,
        plugin_name: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        let command = command.into();
        Self {
            handler_full_name: handler_full_name.into(),
            plugin_name: plugin_name.into(),
            description: String::new(),
            command_type: CommandType::Command,
            original_command: command.clone(),
            current_fragment: command,
            parent_signature: String::new(),
            aliases: Vec::new(),
            permission: CommandPermission::Everyone,
            enabled: true,
            reserved: false,
        }
    }

    pub fn with_parent_signature(mut self, parent_signature: impl Into<String>) -> Self {
        self.parent_signature = parent_signature.into();
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_command_type(mut self, command_type: CommandType) -> Self {
        self.command_type = command_type;
        self
    }

    pub fn with_current_fragment(mut self, current_fragment: impl Into<String>) -> Self {
        let current_fragment = current_fragment.into().trim().to_string();
        if !current_fragment.is_empty() {
            self.current_fragment = current_fragment;
        }
        self
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = alias.into();
        if !alias.trim().is_empty() {
            self.aliases.push(alias);
            self.aliases.sort();
            self.aliases.dedup();
        }
        self
    }

    pub fn with_permission(mut self, permission: CommandPermission) -> Self {
        self.permission = permission;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn reserved(mut self) -> Self {
        self.reserved = true;
        self
    }

    pub fn handler_name(&self) -> &str {
        self.handler_full_name
            .rsplit_once('.')
            .map(|(_, handler)| handler)
            .unwrap_or(&self.handler_full_name)
    }

    pub fn is_executable(&self) -> bool {
        self.enabled && self.command_type != CommandType::Group
    }

    pub fn effective_command(&self) -> String {
        compose_command(&self.parent_signature, &self.current_fragment)
    }

    pub fn effective_aliases(&self) -> Vec<String> {
        self.aliases
            .iter()
            .map(|alias| compose_command(&self.parent_signature, alias))
            .filter(|alias| !alias.trim().is_empty())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    #[default]
    Command,
    Group,
    SubCommand,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPermission {
    #[default]
    Everyone,
    Member,
    Admin,
}

fn compose_command(parent_signature: &str, fragment: &str) -> String {
    let parent_signature = parent_signature.trim();
    let fragment = fragment.trim();

    match (parent_signature.is_empty(), fragment.is_empty()) {
        (true, true) => String::new(),
        (true, false) => fragment.to_string(),
        (false, true) => parent_signature.to_string(),
        (false, false) => format!("{parent_signature} {fragment}"),
    }
}

fn default_enabled() -> bool {
    true
}

pub const BUILTIN_COMMAND_PLUGIN_NAME: &str = "builtin_commands";

pub fn builtin_command_descriptors() -> Vec<CommandDescriptor> {
    let mut commands = vec![
        builtin_command("help", "help", "查看帮助", CommandPermission::Everyone),
        builtin_command("llm", "llm", "开启/关闭 LLM", CommandPermission::Admin),
        builtin_group("plugin", "plugin", "插件管理"),
        builtin_sub_command(
            "plugin_ls",
            "plugin",
            "ls",
            "获取已经安装的插件列表。",
            CommandPermission::Everyone,
        ),
        builtin_sub_command(
            "plugin_off",
            "plugin",
            "off",
            "禁用插件",
            CommandPermission::Admin,
        ),
        builtin_sub_command(
            "plugin_on",
            "plugin",
            "on",
            "启用插件",
            CommandPermission::Admin,
        ),
        builtin_sub_command(
            "plugin_get",
            "plugin",
            "get",
            "安装插件",
            CommandPermission::Admin,
        ),
        builtin_sub_command(
            "plugin_help",
            "plugin",
            "help",
            "获取插件帮助",
            CommandPermission::Everyone,
        ),
        builtin_command("t2i", "t2i", "开关文本转图片", CommandPermission::Everyone),
        builtin_command(
            "tts",
            "tts",
            "开关文本转语音（会话级别）",
            CommandPermission::Everyone,
        ),
        builtin_command(
            "sid",
            "sid",
            "获取会话 ID 和管理员 ID",
            CommandPermission::Everyone,
        ),
        builtin_command("op", "op", "授权管理员", CommandPermission::Admin),
        builtin_command("deop", "deop", "取消授权管理员", CommandPermission::Admin),
        builtin_command("wl", "wl", "添加白名单", CommandPermission::Admin),
        builtin_command("dwl", "dwl", "删除白名单", CommandPermission::Admin),
        builtin_command(
            "provider",
            "provider",
            "查看或者切换 LLM Provider",
            CommandPermission::Admin,
        ),
        builtin_command(
            "reset",
            "reset",
            "重置 LLM 会话",
            CommandPermission::Everyone,
        ),
        builtin_command(
            "stop",
            "stop",
            "停止当前会话中正在运行的 Agent",
            CommandPermission::Everyone,
        ),
        builtin_command(
            "model",
            "model",
            "查看或者切换模型",
            CommandPermission::Admin,
        ),
        builtin_command(
            "history",
            "history",
            "查看对话记录",
            CommandPermission::Everyone,
        ),
        builtin_command("ls", "ls", "查看对话列表", CommandPermission::Everyone),
        builtin_command("new", "new", "创建新对话", CommandPermission::Everyone),
        builtin_command(
            "groupnew",
            "groupnew",
            "创建新群聊对话",
            CommandPermission::Admin,
        ),
        builtin_command(
            "switch",
            "switch",
            "通过 /ls 前面的序号切换对话",
            CommandPermission::Everyone,
        ),
        builtin_command(
            "rename",
            "rename",
            "重命名对话",
            CommandPermission::Everyone,
        ),
        builtin_command("del", "del", "删除当前对话", CommandPermission::Everyone),
        builtin_command("key", "key", "查看或者切换 Key", CommandPermission::Admin),
        builtin_command(
            "persona",
            "persona",
            "查看或者切换 Persona",
            CommandPermission::Admin,
        ),
        builtin_command(
            "dashboard_update",
            "dashboard_update",
            "更新管理面板",
            CommandPermission::Admin,
        ),
        builtin_command(
            "set_variable",
            "set",
            "设置会话变量",
            CommandPermission::Everyone,
        ),
        builtin_command(
            "unset_variable",
            "unset",
            "删除会话变量",
            CommandPermission::Everyone,
        ),
        builtin_command(
            "alter_cmd",
            "alter_cmd",
            "修改命令权限",
            CommandPermission::Admin,
        )
        .with_alias("alter"),
    ];
    commands.sort_by(|left, right| left.effective_command().cmp(&right.effective_command()));
    commands
}

fn builtin_command(
    handler_name: &str,
    command: &str,
    description: &str,
    permission: CommandPermission,
) -> CommandDescriptor {
    CommandDescriptor::new(
        builtin_handler_full_name(handler_name),
        BUILTIN_COMMAND_PLUGIN_NAME,
        command,
    )
    .with_description(description)
    .with_permission(permission)
    .reserved()
}

fn builtin_group(handler_name: &str, command: &str, description: &str) -> CommandDescriptor {
    builtin_command(
        handler_name,
        command,
        description,
        CommandPermission::Everyone,
    )
    .with_command_type(CommandType::Group)
}

fn builtin_sub_command(
    handler_name: &str,
    parent: &str,
    command: &str,
    description: &str,
    permission: CommandPermission,
) -> CommandDescriptor {
    builtin_command(handler_name, command, description, permission)
        .with_command_type(CommandType::SubCommand)
        .with_parent_signature(parent)
}

fn builtin_handler_full_name(handler_name: &str) -> String {
    format!("{BUILTIN_COMMAND_PLUGIN_NAME}.{handler_name}")
}
