use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{CommandHandlerResult, IncomingCommand, OutboundMessage};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCommand {
    pub keyword: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltinContext {
    pub admin_ids: Vec<String>,
}

impl BuiltinContext {
    pub fn new(admin_ids: &[&str]) -> Self {
        let mut ids = Vec::new();
        for id in admin_ids {
            ids.push((*id).to_string());
        }
        Self { admin_ids: ids }
    }

    pub fn is_admin(&self, sender_id: &str) -> bool {
        self.admin_ids.iter().any(|id| id == sender_id)
    }
}

pub const BUILTIN_COMMANDS: &[(&str, &str)] = &[
    ("help", "列出内置指令"),
    ("sid", "显示 UMO / UID / 会话来源"),
    ("op", "授权管理员"),
    ("deop", "取消管理员"),
    ("wl", "添加白名单"),
    ("dwl", "删除白名单"),
    ("alter_cmd", "修改指令权限"),
    ("set", "设置会话变量"),
    ("unset", "删除会话变量"),
    ("reset", "重置当前对话"),
    ("stop", "停止当前会话任务"),
    ("history", "查看对话历史"),
    ("ls", "查看对话列表"),
    ("new", "创建新对话"),
    ("switch", "切换对话"),
    ("rename", "重命名对话"),
    ("del", "删除当前对话"),
    ("persona", "查看或切换人格"),
    ("llm", "查看或切换 LLM"),
    ("model", "查看或切换模型"),
    ("provider", "查看、切换或检查 provider"),
    ("plugin", "管理插件"),
    ("tts", "文本转语音"),
    ("t2i", "文本转图片"),
    ("websearch", "网页搜索"),
    ("sleep", "临时休眠会话"),
    ("wake", "唤醒会话"),
    ("rate", "设置会话限流"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebSearchEngine {
    Bing,
    Sogou,
}

impl WebSearchEngine {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bing => "Bing",
            Self::Sogou => "Sogou",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub summary: String,
    pub timestamp: String,
}

impl WebSearchResult {
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        summary: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            summary: summary.into(),
            timestamp: timestamp.into(),
        }
    }
}

pub fn parse_command(command: &IncomingCommand) -> ParsedCommand {
    ParsedCommand {
        keyword: command.keyword.trim().to_string(),
        args: split_args(&command.argument),
    }
}

pub fn split_args(input: &str) -> Vec<String> {
    input.split_whitespace().map(ToString::to_string).collect()
}

pub fn reply_text(command: &IncomingCommand, text: impl Into<String>) -> CommandHandlerResult {
    CommandHandlerResult::reply(OutboundMessage {
        session_id: command.session_id.clone(),
        text: text.into(),
    })
}

pub fn require_admin(
    command: &IncomingCommand,
    ctx: &BuiltinContext,
) -> Result<(), CommandHandlerResult> {
    if ctx.is_admin(&command.sender_id) {
        Ok(())
    } else {
        Err(reply_text(command, "权限不足：该命令仅管理员可用。"))
    }
}

pub fn sid(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    let admin_state = if ctx.is_admin(&command.sender_id) {
        "是"
    } else {
        "否"
    };
    reply_text(
        command,
        format!(
            "会话 ID：{}\n用户 ID：{}\n管理员：{}",
            command.session_id, command.sender_id, admin_state
        ),
    )
}

pub fn admin(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    if let Err(response) = require_admin(command, ctx) {
        return response;
    }
    let mut args = split_args(&command.argument);
    let action = if command.keyword == "admin" {
        if args.is_empty() {
            return reply_text(command, "支持的管理员命令：admin op/deop/wl/dwl <id>。");
        }
        args.remove(0)
    } else {
        command.keyword.clone()
    };
    match action.as_str() {
        "op" => target_response(command, &args, "已授权管理员"),
        "deop" => target_response(command, &args, "已取消管理员授权"),
        "wl" => target_response(command, &args, "已添加白名单"),
        "dwl" => target_response(command, &args, "已删除白名单"),
        _ => reply_text(command, "支持的管理员命令：admin op/deop/wl/dwl <id>。"),
    }
}

pub fn set_variable(command: &IncomingCommand) -> CommandHandlerResult {
    let args = split_args(&command.argument);
    if args.len() < 2 {
        return reply_text(command, "用法：/set <key> <value>");
    }
    reply_text(
        command,
        format!("已设置会话变量 {} = {}", args[0], args[1..].join(" ")),
    )
}

pub fn unset_variable(command: &IncomingCommand) -> CommandHandlerResult {
    let args = split_args(&command.argument);
    if args.is_empty() {
        return reply_text(command, "用法：/unset <key>");
    }
    reply_text(command, format!("已删除会话变量 {}", args[0]))
}

pub fn alter_cmd(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    if let Err(response) = require_admin(command, ctx) {
        return response;
    }
    let args = split_args(&command.argument);
    if args.len() < 2 {
        return reply_text(
            command,
            "用法：/alter_cmd <command> <admin|member>；reset 权限细分使用 /alter_cmd reset scene <1|2|3> <admin|member>。",
        );
    }
    if args[0] == "reset" && args.get(1).map(String::as_str) == Some("config") {
        return reply_text(
            command,
            "reset 权限场景：1=群聊+会话隔离开，2=群聊+会话隔离关，3=私聊。",
        );
    }
    if args[0] == "reset" && args.get(1).map(String::as_str) == Some("scene") {
        if args.len() < 4 || !matches!(args[3].as_str(), "admin" | "member") {
            return reply_text(
                command,
                "用法：/alter_cmd reset scene <1|2|3> <admin|member>",
            );
        }
        return reply_text(
            command,
            format!(
                "已将 reset 命令场景 {} 的权限级别调整为 {}。",
                args[2], args[3]
            ),
        );
    }
    if !matches!(args.last().map(String::as_str), Some("admin" | "member")) {
        return reply_text(command, "指令类型错误，可选类型有 admin, member。");
    }
    let permission = args.last().expect("checked above");
    let command_name = args[..args.len() - 1].join(" ");
    reply_text(
        command,
        format!("已将「{command_name}」指令的权限级别调整为 {permission}。"),
    )
}

pub fn help(command: &IncomingCommand) -> CommandHandlerResult {
    let mut text = String::from("AstrBot 内置指令：");
    for (name, desc) in BUILTIN_COMMANDS {
        text.push_str(&format!("\n/{name} - {desc}"));
    }
    reply_text(command, text)
}

pub fn llm(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    if let Err(response) = require_admin(command, ctx) {
        return response;
    }
    let args = split_args(&command.argument);
    match args.first().map(String::as_str) {
        None => reply_text(
            command,
            "LLM 命令：/llm list 查看；/llm <provider> 切换；/llm reset 重置；/llm on/off 开关。",
        ),
        Some("list") => reply_text(command, "LLM provider 列表读取请求已接收。"),
        Some("reset") => reply_text(command, "LLM provider 会话偏好已重置。"),
        Some("on" | "enable" | "开启") => reply_text(command, "LLM 聊天功能已开启。"),
        Some("off" | "disable" | "关闭") => reply_text(command, "LLM 聊天功能已关闭。"),
        Some(provider_id) => reply_text(
            command,
            format!("LLM provider 切换请求已接收：{provider_id}"),
        ),
    }
}

pub fn provider(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    if let Err(response) = require_admin(command, ctx) {
        return response;
    }
    let args = split_args(&command.argument);
    match args.first().map(String::as_str) {
        None | Some("list") => reply_text(command, "Provider 列表读取请求已接收。"),
        Some("health") => reply_text(command, "Provider 健康检查请求已接收。"),
        Some("tts" | "stt") if args.len() >= 2 => reply_text(
            command,
            format!(
                "{} provider 切换请求已接收：{}",
                args[0].to_uppercase(),
                args[1]
            ),
        ),
        Some(index_or_id) => reply_text(command, format!("Provider 切换请求已接收：{index_or_id}")),
    }
}

pub fn model(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    if let Err(response) = require_admin(command, ctx) {
        return response;
    }
    let value = command.argument.trim();
    if value.is_empty() || value == "list" {
        reply_text(command, "模型列表读取请求已接收。")
    } else if value == "reset" {
        reply_text(command, "模型选择已重置为 provider 默认值。")
    } else {
        reply_text(command, format!("模型切换请求已接收：{value}"))
    }
}

pub fn persona(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    if let Err(response) = require_admin(command, ctx) {
        return response;
    }
    let args = split_args(&command.argument);
    match args.first().map(String::as_str) {
        None => reply_text(
            command,
            "Persona 命令：/persona list；/persona view <name>；/persona unset；/persona <name>。",
        ),
        Some("list") => reply_text(command, "Persona 文件夹树列表读取请求已接收。"),
        Some("view") if args.len() >= 2 => {
            reply_text(command, format!("Persona 详情读取请求已接收：{}", args[1]))
        }
        Some("view") => reply_text(command, "请输入人格情景名。"),
        Some("unset") => reply_text(command, "取消当前对话 Persona 请求已接收。"),
        Some(_) => reply_text(
            command,
            format!("Persona 切换请求已接收：{}", args.join(" ")),
        ),
    }
}

pub fn plugin(command: &IncomingCommand, ctx: &BuiltinContext) -> CommandHandlerResult {
    let args = split_args(&command.argument);
    let action = args.first().map(String::as_str).unwrap_or("help");
    match action {
        "ls" | "list" => reply_text(command, "插件列表读取请求已接收。"),
        "help" => reply_text(
            command,
            "插件命令：plugin list/help/enable/disable/install/upgrade。",
        ),
        "on" | "enable" | "off" | "disable" | "get" | "install" | "upgrade" => {
            if let Err(response) = require_admin(command, ctx) {
                return response;
            }
            if args.len() < 2 {
                return reply_text(command, format!("用法：/plugin {action} <plugin|repo>"));
            }
            reply_text(
                command,
                format!(
                    "插件操作已接收：{} {}",
                    canonical_plugin_action(action),
                    args[1]
                ),
            )
        }
        _ => reply_text(command, "未知插件子命令。"),
    }
}

pub fn tts(command: &IncomingCommand) -> CommandHandlerResult {
    let arg = command.argument.trim();
    match arg {
        "" => reply_text(
            command,
            "TTS 命令：/tts <text> 单次合成；/tts on/off 自动语音。",
        ),
        "on" | "enable" | "开启" => reply_text(command, "已开启当前会话的文本转语音。"),
        "off" | "disable" | "关闭" => reply_text(command, "已关闭当前会话的文本转语音。"),
        text => reply_text(command, format!("TTS 单次合成请求已接收：{text}")),
    }
}

pub fn t2i(command: &IncomingCommand) -> CommandHandlerResult {
    let args = split_args(&command.argument);
    match args.first().map(String::as_str) {
        None => reply_text(command, "T2I 命令：/t2i <prompt>；/t2i template <name>。"),
        Some("template") if args.len() >= 2 => {
            reply_text(command, format!("T2I 模板预览请求已接收：{}", args[1]))
        }
        Some("template") => reply_text(command, "用法：/t2i template <name>"),
        Some("on" | "enable" | "开启") => reply_text(command, "已开启文本转图片模式。"),
        Some("off" | "disable" | "关闭") => reply_text(command, "已关闭文本转图片模式。"),
        Some(_) => reply_text(command, format!("T2I 出图请求已接收：{}", args.join(" "))),
    }
}

pub fn conversation(command: &IncomingCommand) -> CommandHandlerResult {
    match command.keyword.as_str() {
        "reset" => reply_text(command, "已重置当前会话。"),
        "stop" => reply_text(command, "已请求停止当前会话任务。"),
        "history" => reply_text(command, "对话历史读取请求已接收。"),
        "ls" => reply_text(command, "对话列表读取请求已接收。"),
        "new" => reply_text(command, "新对话创建请求已接收。"),
        "groupnew" if command.argument.trim().is_empty() => {
            reply_text(command, "请输入群聊 ID。/groupnew 群聊ID。")
        }
        "groupnew" => reply_text(
            command,
            format!("新群聊对话创建请求已接收：{}", command.argument.trim()),
        ),
        "switch" if command.argument.trim().is_empty() => {
            reply_text(command, "请输入对话序号。/switch 对话序号。")
        }
        "switch" => reply_text(
            command,
            format!("对话切换请求已接收：{}", command.argument.trim()),
        ),
        "rename" if command.argument.trim().is_empty() => {
            reply_text(command, "请输入新的对话名称。")
        }
        "rename" => reply_text(
            command,
            format!("对话重命名请求已接收：{}", command.argument.trim()),
        ),
        "del" => reply_text(command, "对话删除请求已接收。"),
        "key" => reply_text(
            command,
            format!("Key 操作请求已接收：{}", command.argument.trim()),
        ),
        _ => reply_text(command, "未知对话命令。"),
    }
}

pub fn session_controller(command: &IncomingCommand) -> CommandHandlerResult {
    let mut args = split_args(&command.argument);
    let action = if command.keyword == "session" {
        if args.is_empty() {
            return reply_text(
                command,
                "会话控制命令：/sleep <minutes>；/wake；/rate <limit/min>。",
            );
        }
        args.remove(0)
    } else {
        command.keyword.clone()
    };
    match action.as_str() {
        "sleep" => match args.first().and_then(|value| value.parse::<u32>().ok()) {
            Some(minutes) if minutes > 0 => reply_text(
                command,
                format!("会话 {} 已休眠 {minutes} 分钟。", command.session_id),
            ),
            _ => reply_text(command, "用法：/sleep <minutes>"),
        },
        "wake" => reply_text(command, format!("会话 {} 已唤醒。", command.session_id)),
        "rate" => match args.first().and_then(|value| value.parse::<u32>().ok()) {
            Some(limit) if limit > 0 => reply_text(
                command,
                format!("会话 {} 限流已设置为 {limit}/min。", command.session_id),
            ),
            _ => reply_text(command, "用法：/rate <limit/min>"),
        },
        _ => reply_text(command, "未知会话控制命令。"),
    }
}

pub fn web_searcher(command: &IncomingCommand) -> CommandHandlerResult {
    let args = split_args(&command.argument);
    if args.is_empty() {
        return reply_text(command, "用法：/websearch [bing|sogou] <query>");
    }
    let (engine, query) = match args.first().map(String::as_str) {
        Some("bing") => ("Bing", args[1..].join(" ")),
        Some("sogou") => ("Sogou", args[1..].join(" ")),
        _ => ("Bing+Sogou", args.join(" ")),
    };
    if query.trim().is_empty() {
        return reply_text(command, "用法：/websearch [bing|sogou] <query>");
    }
    reply_text(
        command,
        format!(
            "网页搜索请求已接收：engine={engine} query={query}；结果将包含摘要、链接和时间戳。"
        ),
    )
}

pub fn format_web_search_results(
    engine: WebSearchEngine,
    query: &str,
    results: &[WebSearchResult],
) -> String {
    if results.is_empty() {
        return format!("{} 未返回与「{}」相关的结果。", engine.name(), query);
    }
    let mut text = format!("{} 搜索「{}」结果：", engine.name(), query);
    for (idx, result) in results.iter().enumerate() {
        text.push_str(&format!(
            "\n{}. {}\n摘要：{}\n链接：{}\n时间：{}",
            idx + 1,
            result.title,
            result.summary,
            result.url,
            result.timestamp
        ));
    }
    text
}

fn target_response(
    command: &IncomingCommand,
    args: &[String],
    prefix: &str,
) -> CommandHandlerResult {
    if args.is_empty() {
        reply_text(command, "用法错误：缺少目标 ID。")
    } else {
        reply_text(command, format!("{prefix}：{}", args[0]))
    }
}

fn canonical_plugin_action(action: &str) -> &'static str {
    match action {
        "on" | "enable" => "enable",
        "off" | "disable" => "disable",
        "get" | "install" => "install",
        "upgrade" => "upgrade",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginResponse;

    fn command(keyword: &str, argument: &str, sender_id: &str) -> IncomingCommand {
        IncomingCommand {
            session_id: "session-1".to_string(),
            sender_id: sender_id.to_string(),
            keyword: keyword.to_string(),
            argument: argument.to_string(),
        }
    }

    fn text(result: CommandHandlerResult) -> String {
        match result.into_response() {
            PluginResponse::Replies { messages } => messages[0].text.clone(),
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn sid_reports_session_and_admin_state() {
        let ctx = BuiltinContext::new(&["admin-1"]);
        let response = text(sid(&command("sid", "", "admin-1"), &ctx));
        assert!(response.contains("session-1"));
        assert!(response.contains("admin-1"));
        assert!(response.contains("管理员：是"));
    }

    #[test]
    fn admin_only_rejects_non_admin() {
        let ctx = BuiltinContext::new(&["admin-1"]);
        let response = text(llm(&command("llm", "off", "user-1"), &ctx));
        assert!(response.contains("权限不足"));
    }

    #[test]
    fn set_and_unset_validate_arguments() {
        assert!(text(set_variable(&command("set", "", "user-1"))).contains("用法"));
        assert!(text(set_variable(&command("set", "mode chat", "user-1"))).contains("mode = chat"));
        assert!(text(unset_variable(&command("unset", "mode", "user-1"))).contains("mode"));
    }

    #[test]
    fn conversation_routes_common_commands() {
        assert!(text(conversation(&command("reset", "", "user-1"))).contains("重置"));
        assert!(text(conversation(&command("rename", "daily", "user-1"))).contains("daily"));
    }

    #[test]
    fn persona_and_llm_use_admin_flow() {
        let ctx = BuiltinContext::new(&["admin-1"]);
        assert!(text(persona(&command("persona", "default", "admin-1"), &ctx)).contains("default"));
        assert!(text(llm(&command("llm", "on", "admin-1"), &ctx)).contains("LLM"));
    }

    #[test]
    fn help_lists_full_builtin_surface() {
        let response = text(help(&command("help", "", "user-1")));

        for expected in [
            "/websearch",
            "/sleep",
            "/wake",
            "/rate",
            "/plugin",
            "/provider",
        ] {
            assert!(
                response.contains(expected),
                "missing {expected}: {response}"
            );
        }
    }

    #[test]
    fn plugin_aliases_cover_management_actions() {
        let ctx = BuiltinContext::new(&["admin-1"]);

        let enable = text(plugin(
            &command("plugin", "enable weather", "admin-1"),
            &ctx,
        ));
        let install = text(plugin(
            &command("plugin", "install https://example.test/repo", "admin-1"),
            &ctx,
        ));
        let denied = text(plugin(
            &command("plugin", "disable weather", "user-1"),
            &ctx,
        ));

        assert!(enable.contains("enable weather"));
        assert!(install.contains("install https://example.test/repo"));
        assert!(denied.contains("权限不足"));
    }

    #[test]
    fn websearch_formats_engine_results_with_links_and_timestamps() {
        let results = vec![WebSearchResult::new(
            "AstrBot Docs",
            "https://astrbot.app",
            "Rust rewrite docs",
            "2026-05-20T09:00:00Z",
        )];

        let response = format_web_search_results(WebSearchEngine::Bing, "astrbot", &results);

        assert!(response.contains("Bing"));
        assert!(response.contains("AstrBot Docs"));
        assert!(response.contains("https://astrbot.app"));
        assert!(response.contains("2026-05-20T09:00:00Z"));
    }

    #[test]
    fn session_controller_validates_sleep_wake_and_rate() {
        assert!(text(session_controller(&command("sleep", "15", "user-1"))).contains("15 分钟"));
        assert!(text(session_controller(&command("wake", "", "user-1"))).contains("已唤醒"));
        assert!(text(session_controller(&command("rate", "6", "user-1"))).contains("6/min"));
        assert!(
            text(session_controller(&command("session", "rate 3", "user-1"))).contains("3/min")
        );
        assert!(text(session_controller(&command("sleep", "0", "user-1"))).contains("用法"));
    }

    #[test]
    fn tts_and_t2i_cover_one_shot_and_template_commands() {
        assert!(text(tts(&command("tts", "hello world", "user-1"))).contains("单次合成"));
        assert!(text(tts(&command("tts", "on", "user-1"))).contains("已开启"));
        assert!(text(t2i(&command("t2i", "a city", "user-1"))).contains("出图"));
        assert!(
            text(t2i(&command("t2i", "template error_prompt", "user-1"))).contains("error_prompt")
        );
    }
}
