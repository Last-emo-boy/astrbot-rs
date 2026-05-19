import { createSignal } from "solid-js";

export type Locale = "zh" | "en";

const LOCALE_KEY = "astrbot.dashboard.locale";

function detectInitial(): Locale {
  try {
    const stored = localStorage.getItem(LOCALE_KEY);
    if (stored === "zh" || stored === "en") return stored;
  } catch {
    /* ignore */
  }
  return navigator.language.startsWith("zh") ? "zh" : "en";
}

const [locale, setLocaleSignal] = createSignal<Locale>(detectInitial());

export { locale };

export function setLocale(value: Locale): void {
  setLocaleSignal(value);
  try {
    localStorage.setItem(LOCALE_KEY, value);
  } catch {
    /* ignore */
  }
}

const messages: Record<Locale, Record<string, string>> = {
  zh: {
    "app.title": "AstrBot 控制台",
    "nav.overview": "概览",
    "nav.chat": "聊天",
    "nav.conversation": "会话历史",
    "nav.persona": "人格",
    "nav.providers": "模型提供商",
    "nav.platforms": "消息平台",
    "nav.plugins": "插件",
    "nav.market": "插件市场",
    "nav.skills": "技能",
    "nav.tools": "工具",
    "nav.subagent": "子代理",
    "nav.knowledge": "知识库",
    "nav.mcp": "MCP 服务器",
    "nav.cron": "定时任务",
    "nav.sessions": "会话规则",
    "nav.projects": "ChatUI 项目",
    "nav.config": "全局配置",
    "nav.console": "控制台日志",
    "nav.trace": "调用轨迹",
    "nav.apiKeys": "API 密钥",
    "nav.observability": "观测",
    "nav.t2i": "T2I 模板",
    "nav.backup": "备份",
    "nav.update": "更新",
    "nav.settings": "设置",
    "nav.about": "关于",
    "common.save": "保存",
    "common.cancel": "取消",
    "common.delete": "删除",
    "common.create": "新建",
    "common.edit": "编辑",
    "common.refresh": "刷新",
    "common.confirm": "确认",
    "common.search": "搜索",
    "common.loading": "加载中…",
    "common.empty": "暂无数据",
    "common.error": "错误",
    "common.actions": "操作",
    "common.name": "名称",
    "common.status": "状态",
    "common.enabled": "已启用",
    "common.disabled": "已停用",
    "common.created": "创建时间",
    "common.updated": "更新时间",
    "common.logout": "退出登录",
    "login.title": "登录到 AstrBot",
    "login.username": "用户名",
    "login.password": "密码",
    "login.submit": "登录",
    "login.error": "登录失败，请检查用户名和密码",
    "welcome.title": "欢迎使用 AstrBot",
    "welcome.subtitle": "请先完成初始化或前往登录页",
    "theme.toggle": "切换主题",
    "locale.toggle": "中 / EN",
  },
  en: {
    "app.title": "AstrBot Dashboard",
    "nav.overview": "Overview",
    "nav.chat": "Chat",
    "nav.conversation": "History",
    "nav.persona": "Personas",
    "nav.providers": "Providers",
    "nav.platforms": "Platforms",
    "nav.plugins": "Plugins",
    "nav.market": "Marketplace",
    "nav.skills": "Skills",
    "nav.tools": "Tools",
    "nav.subagent": "SubAgents",
    "nav.knowledge": "Knowledge",
    "nav.mcp": "MCP Servers",
    "nav.cron": "Cron",
    "nav.sessions": "Sessions",
    "nav.projects": "Projects",
    "nav.config": "Config",
    "nav.console": "Console",
    "nav.trace": "Traces",
    "nav.apiKeys": "API Keys",
    "nav.observability": "Observability",
    "nav.t2i": "T2I Templates",
    "nav.backup": "Backup",
    "nav.update": "Updates",
    "nav.settings": "Settings",
    "nav.about": "About",
    "common.save": "Save",
    "common.cancel": "Cancel",
    "common.delete": "Delete",
    "common.create": "New",
    "common.edit": "Edit",
    "common.refresh": "Refresh",
    "common.confirm": "Confirm",
    "common.search": "Search",
    "common.loading": "Loading…",
    "common.empty": "Empty",
    "common.error": "Error",
    "common.actions": "Actions",
    "common.name": "Name",
    "common.status": "Status",
    "common.enabled": "Enabled",
    "common.disabled": "Disabled",
    "common.created": "Created",
    "common.updated": "Updated",
    "common.logout": "Sign out",
    "login.title": "Sign in to AstrBot",
    "login.username": "Username",
    "login.password": "Password",
    "login.submit": "Sign in",
    "login.error": "Login failed; check username/password",
    "welcome.title": "Welcome to AstrBot",
    "welcome.subtitle": "Initialize the dashboard or proceed to login.",
    "theme.toggle": "Toggle theme",
    "locale.toggle": "EN / 中",
  },
};

export function t(key: string): string {
  const dict = messages[locale()];
  return dict[key] ?? key;
}
