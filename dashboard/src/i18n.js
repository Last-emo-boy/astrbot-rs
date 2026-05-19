const LOCALE_KEY = "astrbot.locale";
const SOURCE_LOCALE_KEY = "astrbot-locale";

const localeAliases = {
  zh: "zh-CN",
  "zh-CN": "zh-CN",
  cn: "zh-CN",
  en: "en-US",
  "en-US": "en-US",
  ru: "ru-RU",
  "ru-RU": "ru-RU",
};

export const locales = [
  { id: "zh-CN", legacyId: "zh", label: "简体中文", flag: "CN" },
  { id: "en-US", legacyId: "en", label: "English", flag: "US" },
  { id: "ru-RU", legacyId: "ru", label: "Русский", flag: "RU" },
];

const routeCopy = {
  "zh-CN": {
    overview: ["控制台", "控制台", "运行状态、服务闭环和核心指标"],
    chat: ["Chat", "Chat", "通过 Rust WebChat 管道发送消息并读取历史"],
    chatbox: ["ChatBox", "ChatBox", "OpenAPI realtime 控制与 WebChat 历史"],
    conversation: ["会话", "Conversation", "WebChat conversation 历史与项目归属"],
    console: ["Console", "Console", "读取 management log buffer"],
    trace: ["Trace", "Trace", "查看 pipeline trace event"],
    config: ["系统配置", "系统配置", "读取、预览并应用 runtime 配置"],
    providers: ["Provider", "Provider", "模型、语音、Embedding、Rerank 能力状态"],
    platforms: ["平台", "平台", "平台适配器与 WebChat 运行状态"],
    sessions: ["会话规则", "会话规则", "Session service/provider 偏好与分组"],
    personas: ["Persona", "Persona", "管理人格预设、文件夹与解析规则"],
    cron: ["Cron", "Cron", "管理定时任务、主动唤醒和 scheduler 状态"],
    plugins: ["插件", "插件", "已注册 handler 与插件状态"],
    market: ["插件市场", "插件市场", "插件 catalog 与 install/update/uninstall 执行闭环"],
    skills: ["Skills", "Skills", "技能目录、sandbox cache 与安装/删除执行闭环"],
    subagent: ["SubAgent", "SubAgent", "Sub-agent 配置接口与 Rust 侧接入状态"],
    tools: ["工具", "工具", "工具来源、Commands 权限与 MCP server bridge plan"],
    knowledge: ["知识库", "知识库", "创建 KB、预检 provider、跟踪上传任务"],
    projects: ["Chat 项目", "Chat 项目", "管理 ChatUI 项目与会话归属"],
    backup: ["备份", "备份", "备份预检、导出/导入任务、进度查询与上传会话"],
    update: ["更新维护", "更新维护", "版本检查、更新/迁移/包安装 operation 闭环"],
    settings: ["Settings", "Settings", "Dashboard 设置和运行时摘要"],
    about: ["About", "About", "Rust 版复刻范围与闭环说明"],
  },
  "en-US": {
    overview: ["Console", "Console", "Runtime status, service closure, and core metrics"],
    chat: ["Chat", "Chat", "Send messages through Rust WebChat and read history"],
    chatbox: ["ChatBox", "ChatBox", "OpenAPI realtime control and WebChat history"],
    conversation: ["Conversations", "Conversations", "WebChat conversation history and project membership"],
    console: ["Console", "Console", "Read the management log buffer"],
    trace: ["Trace", "Trace", "Inspect pipeline trace events"],
    config: ["System Config", "System Config", "Read, preview, and apply runtime config"],
    providers: ["Providers", "Providers", "Model, voice, embedding, and rerank capabilities"],
    platforms: ["Platforms", "Platforms", "Platform adapters and WebChat runtime status"],
    sessions: ["Session Rules", "Session Rules", "Session service/provider preferences and groups"],
    personas: ["Personas", "Personas", "Manage persona presets, folders, and resolution rules"],
    cron: ["Cron", "Cron", "Manage scheduled jobs, proactive wakeups, and scheduler state"],
    plugins: ["Plugins", "Plugins", "Registered handlers and plugin state"],
    market: ["Plugin Market", "Plugin Market", "Plugin catalog and install/update/uninstall closures"],
    skills: ["Skills", "Skills", "Skill catalog, sandbox cache, and install/delete closures"],
    subagent: ["SubAgent", "SubAgent", "Sub-agent config APIs and Rust-side bridge state"],
    tools: ["Tools", "Tools", "Tool sources, command permissions, and MCP server bridge plans"],
    knowledge: ["Knowledge", "Knowledge", "Create KBs, preflight providers, and track upload tasks"],
    projects: ["Chat Projects", "Chat Projects", "Manage ChatUI projects and conversation membership"],
    backup: ["Backup", "Backup", "Backup precheck, export/import tasks, progress, and uploads"],
    update: ["Maintenance", "Maintenance", "Version checks, update/migration/package operation closures"],
    settings: ["Settings", "Settings", "Dashboard settings and runtime summary"],
    about: ["About", "About", "Rust port scope and closure boundaries"],
  },
  "ru-RU": {
    overview: ["Консоль", "Консоль", "Статус runtime, границы сервисов и основные метрики"],
    chat: ["Chat", "Chat", "Отправка сообщений через Rust WebChat и чтение истории"],
    chatbox: ["ChatBox", "ChatBox", "OpenAPI realtime control и история WebChat"],
    conversation: ["Диалоги", "Диалоги", "История WebChat conversation и привязка к проектам"],
    console: ["Console", "Console", "Чтение management log buffer"],
    trace: ["Trace", "Trace", "Просмотр pipeline trace events"],
    config: ["Конфиг", "Конфиг", "Чтение, preview и применение runtime config"],
    providers: ["Providers", "Providers", "Модели, voice, embedding и rerank capabilities"],
    platforms: ["Платформы", "Платформы", "Platform adapters и статус WebChat"],
    sessions: ["Session Rules", "Session Rules", "Session service/provider preferences и группы"],
    personas: ["Personas", "Personas", "Persona presets, папки и правила resolution"],
    cron: ["Cron", "Cron", "Scheduled jobs, proactive wakeups и scheduler state"],
    plugins: ["Плагины", "Плагины", "Registered handlers и состояние plugins"],
    market: ["Plugin Market", "Plugin Market", "Plugin catalog и install/update/uninstall closures"],
    skills: ["Skills", "Skills", "Skill catalog, sandbox cache и install/delete closures"],
    subagent: ["SubAgent", "SubAgent", "Sub-agent config APIs и Rust-side bridge state"],
    tools: ["Tools", "Tools", "Tool sources, command permissions и MCP bridge plans"],
    knowledge: ["Knowledge", "Knowledge", "KB, provider preflight и upload tasks"],
    projects: ["Chat Projects", "Chat Projects", "ChatUI projects и conversation membership"],
    backup: ["Backup", "Backup", "Backup precheck, export/import tasks, progress и uploads"],
    update: ["Maintenance", "Maintenance", "Version checks, update/migration/package operation closures"],
    settings: ["Settings", "Settings", "Dashboard settings и runtime summary"],
    about: ["About", "About", "Scope of Rust port и closure boundaries"],
  },
};

const localeText = {
  "zh-CN": {
    common: {
      language: "语言",
      close: "关闭",
      save: "保存",
      reset: "恢复默认",
      refresh: "刷新",
      browserFallback: "浏览器降级",
      configured: "已配置",
      unavailable: "不可用",
      connected: "已连接",
      pending: "等待中",
    },
    actions: {
      save: "保存",
      reset: "恢复默认",
      cancel: "取消",
      refresh: "刷新",
      moveUp: "上移",
      moveDown: "下移",
      moveToMore: "移入更多",
      moveToMain: "移入主区",
      hide: "隐藏",
      show: "显示",
      check: "检查",
      install: "安装",
      restart: "重启",
    },
    groups: {
      runtime: "运行",
      config: "配置",
      extensions: "扩展",
      data: "数据",
      more: "更多功能",
    },
    settings: {
      summary: {
        apiBase: "API Base",
        apiKeys: "API Keys",
        backups: "Backups",
        version: "Version",
      },
      network: {
        title: "网络",
        subtitle: "配置后端 API 地址和更新流程使用的 GitHub 代理。",
        apiBase: "API 基础地址",
        sameOrigin: "同源",
        presetName: "预设名称",
        presetUrl: "预设 URL",
        addPreset: "添加预设",
        removeLastCustom: "删除最后一个自定义",
        useGithubProxy: "使用 GitHub 代理",
        githubProxyUrl: "GitHub 代理 URL",
        saveProxy: "保存代理",
        testProxy: "测试代理",
      },
      apiKey: {
        title: "API Keys",
        subtitle: "创建 scoped OpenAPI 凭据，并吊销或删除已有 key。",
      },
      migration: {
        title: "Migration",
        subtitle: "使用选定平台映射运行 Python v4 兼容迁移。",
        open: "打开迁移",
      },
      appearance: {
        title: "主题",
        subtitle: "切换语言、明暗主题、侧边栏密度和主/次色。",
        theme: "主题",
        light: "浅色",
        dark: "深色",
        primary: "主色",
        secondary: "辅助色",
        compact: "紧凑侧边栏",
        save: "保存偏好",
        resetTheme: "恢复本地主题",
      },
      sidebar: {
        title: "侧边栏",
        subtitle: "按源端 SidebarCustomizer 调整导航顺序，并在主区和更多功能之间移动页面。",
        customizeTitle: "自定义侧边栏",
        customizeSubtitle: "调整模块顺序，或移入/移出更多功能分组。设置仅保存在浏览器本地。",
        mainItems: "主要模块",
        moreItems: "更多功能",
        open: "自定义侧边栏",
        reset: "恢复默认",
        save: "保存侧边栏",
        empty: "暂无条目",
      },
      backup: {
        title: "备份",
        subtitle: "管理数据备份。",
        open: "打开备份",
      },
      system: {
        title: "系统",
        subtitle: "更新、changelog、重启、退出和本地 Dashboard 重置控制。",
        routes: "Routes",
        token: "Token",
        primary: "Primary",
        changelog: "打开 Changelog",
        restartPlan: "计划重启",
        restartNow: "立即重启",
        logout: "退出登录",
      },
      desktop: {
        title: "Desktop Bridge",
        subtitle: "检测 Electron desktop bridge、后端控制能力和应用更新入口。",
        runtime: "Runtime",
        backend: "Backend",
        updater: "Updater",
        refresh: "检查 bridge",
        restartBackend: "重启后端",
        checkUpdate: "检查应用更新",
        installUpdate: "安装应用更新",
        fallback: "未检测到 desktop bridge，使用浏览器/HTTP API 降级路径。",
        trayListener: "Tray restart listener",
        lastProbe: "最后检查：{time}",
      },
      runtimeSummary: "Runtime 摘要",
      recentOperation: "最近操作",
    },
    messages: {
      success: {
        preferencesSaved: "Dashboard 偏好已保存",
        sidebarSaved: "侧边栏设置已保存",
        sidebarReset: "侧边栏设置已恢复默认",
        themeReset: "本地主题设置已重置",
        desktopProbe: "Desktop bridge 检查已完成",
        localeChanged: "语言已切换",
      },
      errors: {
        invalidColor: "颜色必须是 #RRGGBB 格式",
        desktopUnavailable: "Desktop bridge 不可用",
      },
      validation: {
        required: "{field} 为必填项",
      },
    },
  },
  "en-US": {
    common: {
      language: "Language",
      close: "Close",
      save: "Save",
      reset: "Reset",
      refresh: "Refresh",
      browserFallback: "Browser fallback",
      configured: "Configured",
      unavailable: "Unavailable",
      connected: "Connected",
      pending: "Pending",
    },
    actions: {
      save: "Save",
      reset: "Reset",
      cancel: "Cancel",
      refresh: "Refresh",
      moveUp: "Move up",
      moveDown: "Move down",
      moveToMore: "Move to More",
      moveToMain: "Move to Main",
      hide: "Hide",
      show: "Show",
      check: "Check",
      install: "Install",
      restart: "Restart",
    },
    groups: {
      runtime: "Runtime",
      config: "Config",
      extensions: "Extensions",
      data: "Data",
      more: "More Features",
    },
    settings: {
      summary: {
        apiBase: "API Base",
        apiKeys: "API Keys",
        backups: "Backups",
        version: "Version",
      },
      network: {
        title: "Network",
        subtitle: "Configure backend API base and GitHub proxy used by update workflows.",
        apiBase: "API Base",
        sameOrigin: "same-origin",
        presetName: "Preset name",
        presetUrl: "Preset URL",
        addPreset: "Add preset",
        removeLastCustom: "Remove last custom",
        useGithubProxy: "Use GitHub proxy",
        githubProxyUrl: "GitHub proxy URL",
        saveProxy: "Save proxy",
        testProxy: "Test proxy",
      },
      apiKey: {
        title: "API Keys",
        subtitle: "Create scoped OpenAPI credentials and revoke or delete existing keys.",
      },
      migration: {
        title: "Migration",
        subtitle: "Run Python v4 compatibility migration with selected platform mappings.",
        open: "Open Migration",
      },
      appearance: {
        title: "Style",
        subtitle: "Switch language, theme, sidebar density, and primary or secondary colors.",
        theme: "Theme",
        light: "Light",
        dark: "Dark",
        primary: "Primary color",
        secondary: "Secondary color",
        compact: "Compact sidebar",
        save: "Save preferences",
        resetTheme: "Reset local theme",
      },
      sidebar: {
        title: "Sidebar",
        subtitle: "Mirror the source SidebarCustomizer by ordering routes and moving pages between Main and More Features.",
        customizeTitle: "Customize Sidebar",
        customizeSubtitle: "Reorder modules or move them in and out of More Features. Settings are stored locally in this browser.",
        mainItems: "Main Modules",
        moreItems: "More Features",
        open: "Customize Sidebar",
        reset: "Reset to Default",
        save: "Save Sidebar",
        empty: "No items",
      },
      backup: {
        title: "Backup",
        subtitle: "Manage data backups.",
        open: "Open Backup",
      },
      system: {
        title: "System",
        subtitle: "Update, changelog, restart, logout, and local dashboard reset controls.",
        routes: "Routes",
        token: "Token",
        primary: "Primary",
        changelog: "Open Changelog",
        restartPlan: "Plan restart",
        restartNow: "Restart now",
        logout: "Logout",
      },
      desktop: {
        title: "Desktop Bridge",
        subtitle: "Detect Electron desktop bridge, backend control capability, and app update affordance.",
        runtime: "Runtime",
        backend: "Backend",
        updater: "Updater",
        refresh: "Check bridge",
        restartBackend: "Restart backend",
        checkUpdate: "Check app update",
        installUpdate: "Install app update",
        fallback: "Desktop bridge was not detected; browser and HTTP API fallbacks are active.",
        trayListener: "Tray restart listener",
        lastProbe: "Last probe: {time}",
      },
      runtimeSummary: "Runtime Summary",
      recentOperation: "Recent Operation",
    },
    messages: {
      success: {
        preferencesSaved: "Dashboard preferences saved",
        sidebarSaved: "Sidebar customization saved",
        sidebarReset: "Sidebar customization reset",
        themeReset: "Local theme settings reset",
        desktopProbe: "Desktop bridge probe completed",
        localeChanged: "Language changed",
      },
      errors: {
        invalidColor: "Colors must use #RRGGBB format",
        desktopUnavailable: "Desktop bridge is unavailable",
      },
      validation: {
        required: "{field} is required",
      },
    },
  },
  "ru-RU": {
    common: {
      language: "Язык",
      close: "Закрыть",
      save: "Сохранить",
      reset: "Сброс",
      refresh: "Обновить",
      browserFallback: "Browser fallback",
      configured: "Настроено",
      unavailable: "Недоступно",
      connected: "Подключено",
      pending: "Ожидание",
    },
    actions: {
      save: "Сохранить",
      reset: "Сброс",
      cancel: "Отмена",
      refresh: "Обновить",
      moveUp: "Вверх",
      moveDown: "Вниз",
      moveToMore: "В More",
      moveToMain: "В Main",
      hide: "Скрыть",
      show: "Показать",
      check: "Проверить",
      install: "Установить",
      restart: "Перезапуск",
    },
    groups: {
      runtime: "Запуск",
      config: "Настройки",
      extensions: "Расширения",
      data: "Данные",
      more: "More Features",
    },
    settings: {
      summary: {
        apiBase: "API Base",
        apiKeys: "API Keys",
        backups: "Backups",
        version: "Version",
      },
      network: {
        title: "Сеть",
        subtitle: "Настройка backend API base и GitHub proxy для обновлений.",
        apiBase: "API Base",
        sameOrigin: "same-origin",
        presetName: "Preset name",
        presetUrl: "Preset URL",
        addPreset: "Add preset",
        removeLastCustom: "Remove last custom",
        useGithubProxy: "Use GitHub proxy",
        githubProxyUrl: "GitHub proxy URL",
        saveProxy: "Save proxy",
        testProxy: "Test proxy",
      },
      apiKey: {
        title: "API Keys",
        subtitle: "Создание scoped OpenAPI credentials и управление key.",
      },
      migration: {
        title: "Migration",
        subtitle: "Запуск Python v4 compatibility migration с platform mapping.",
        open: "Open Migration",
      },
      appearance: {
        title: "Стиль",
        subtitle: "Смена языка, темы, плотности sidebar и primary/secondary цветов.",
        theme: "Theme",
        light: "Light",
        dark: "Dark",
        primary: "Primary color",
        secondary: "Secondary color",
        compact: "Compact sidebar",
        save: "Save preferences",
        resetTheme: "Reset local theme",
      },
      sidebar: {
        title: "Sidebar",
        subtitle: "Настройка порядка navigation и групп Main/More Features.",
        customizeTitle: "Customize Sidebar",
        customizeSubtitle: "Reorder modules or move them in and out of More Features. Settings are stored locally in this browser.",
        mainItems: "Main Modules",
        moreItems: "More Features",
        open: "Customize Sidebar",
        reset: "Reset to Default",
        save: "Save Sidebar",
        empty: "No items",
      },
      backup: {
        title: "Backup",
        subtitle: "Управление backup data.",
        open: "Open Backup",
      },
      system: {
        title: "Система",
        subtitle: "Update, changelog, restart, logout и local dashboard reset controls.",
        routes: "Routes",
        token: "Token",
        primary: "Primary",
        changelog: "Open Changelog",
        restartPlan: "Plan restart",
        restartNow: "Restart now",
        logout: "Logout",
      },
      desktop: {
        title: "Desktop Bridge",
        subtitle: "Проверка Electron desktop bridge, backend control и app update affordance.",
        runtime: "Runtime",
        backend: "Backend",
        updater: "Updater",
        refresh: "Check bridge",
        restartBackend: "Restart backend",
        checkUpdate: "Check app update",
        installUpdate: "Install app update",
        fallback: "Desktop bridge не обнаружен; активен browser/HTTP API fallback.",
        trayListener: "Tray restart listener",
        lastProbe: "Last probe: {time}",
      },
      runtimeSummary: "Runtime Summary",
      recentOperation: "Recent Operation",
    },
    messages: {
      success: {
        preferencesSaved: "Dashboard preferences saved",
        sidebarSaved: "Sidebar customization saved",
        sidebarReset: "Sidebar customization reset",
        themeReset: "Local theme settings reset",
        desktopProbe: "Desktop bridge probe completed",
        localeChanged: "Language changed",
      },
      errors: {
        invalidColor: "Colors must use #RRGGBB format",
        desktopUnavailable: "Desktop bridge is unavailable",
      },
      validation: {
        required: "{field} is required",
      },
    },
  },
};

export const dictionaries = Object.fromEntries(
  locales.map(({ id }) => [id, buildDictionary(id)]),
);

export function locale() {
  const stored = window.localStorage.getItem(LOCALE_KEY)
    || window.localStorage.getItem(SOURCE_LOCALE_KEY)
    || "zh-CN";
  return normalizeLocale(stored);
}

export function legacyLocale() {
  return locales.find((item) => item.id === locale())?.legacyId || "zh";
}

export function setLocale(nextLocale) {
  const normalized = normalizeLocale(nextLocale);
  window.localStorage.setItem(LOCALE_KEY, normalized);
  window.localStorage.setItem(SOURCE_LOCALE_KEY, normalized);
  if (document.documentElement) {
    document.documentElement.lang = normalized;
  }
  return normalized;
}

export function t(key, params = {}) {
  const current = dictionaries[locale()] || dictionaries["zh-CN"];
  const fallback = dictionaries["zh-CN"];
  const value = valueForKey(current, key) ?? valueForKey(fallback, key);
  if (typeof value !== "string") return key;
  return interpolate(value, params);
}

export function hasTranslation(localeId, key) {
  const normalized = normalizeLocale(localeId);
  return typeof valueForKey(dictionaries[normalized], key) === "string";
}

export function validateI18nDictionaries() {
  const baseLocale = "zh-CN";
  const baseKeys = flattenKeys(dictionaries[baseLocale]);
  const errors = [];
  for (const { id } of locales) {
    const keys = flattenKeys(dictionaries[id]);
    const keySet = new Set(keys);
    for (const key of baseKeys) {
      if (!keySet.has(key)) {
        errors.push({ type: "missing", locale: id, key });
        continue;
      }
      const baseTokens = interpolationTokens(valueForKey(dictionaries[baseLocale], key));
      const localeTokens = interpolationTokens(valueForKey(dictionaries[id], key));
      if (baseTokens.join(",") !== localeTokens.join(",")) {
        errors.push({ type: "interpolation", locale: id, key, expected: baseTokens, actual: localeTokens });
      }
    }
  }
  return {
    isValid: errors.length === 0,
    locales: locales.map((item) => item.id),
    totalKeys: baseKeys.length,
    errors,
  };
}

export function normalizeLocale(value) {
  return localeAliases[String(value || "").trim()] || "zh-CN";
}

function buildDictionary(localeId) {
  const text = localeText[localeId];
  return {
    group: {
      runtime: text.groups.runtime,
      config: text.groups.config,
      extensions: text.groups.extensions,
      data: text.groups.data,
    },
    route: buildRouteDictionary(routeCopy[localeId]),
    core: {
      common: text.common,
      actions: text.actions,
      navigation: {
        welcome: routeCopy[localeId].overview[0],
        dashboard: routeCopy[localeId].overview[1],
        platforms: routeCopy[localeId].platforms[0],
        providers: routeCopy[localeId].providers[0],
        commands: routeCopy[localeId].tools[0],
        persona: routeCopy[localeId].personas[0],
        subagent: routeCopy[localeId].subagent[0],
        toolUse: routeCopy[localeId].tools[0],
        extension: routeCopy[localeId].plugins[0],
        config: routeCopy[localeId].config[0],
        chat: routeCopy[localeId].chat[0],
        cron: routeCopy[localeId].cron[0],
        conversation: routeCopy[localeId].conversation[0],
        sessionManagement: routeCopy[localeId].sessions[0],
        console: routeCopy[localeId].console[0],
        trace: routeCopy[localeId].trace[0],
        knowledgeBase: routeCopy[localeId].knowledge[0],
        about: routeCopy[localeId].about[0],
        settings: routeCopy[localeId].settings[0],
        groups: { more: text.groups.more },
      },
    },
    features: {
      settings: text.settings,
    },
    messages: text.messages,
    settings: {
      language: text.common.language,
    },
  };
}

function buildRouteDictionary(copy) {
  return Object.fromEntries(
    Object.entries(copy).map(([id, [label, title, subtitle]]) => [
      id,
      { label, title, subtitle },
    ]),
  );
}

function valueForKey(source, key) {
  return String(key || "")
    .split(".")
    .reduce((cursor, part) => (cursor && typeof cursor === "object" ? cursor[part] : undefined), source);
}

function interpolate(value, params) {
  return value.replace(/\{(\w+)\}/g, (match, name) => (
    Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : match
  ));
}

function flattenKeys(source, prefix = "") {
  if (!source || typeof source !== "object") return [];
  return Object.entries(source).flatMap(([key, value]) => {
    const next = prefix ? `${prefix}.${key}` : key;
    return value && typeof value === "object" ? flattenKeys(value, next) : [next];
  });
}

function interpolationTokens(value) {
  if (typeof value !== "string") return [];
  return Array.from(value.matchAll(/\{(\w+)\}/g), (match) => match[1]).sort();
}
