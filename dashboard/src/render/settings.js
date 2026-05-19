import { apiBase, dashboardPreferences, desktopBridgeSnapshot, managementToken } from "../api.js";
import { escapeHtml, jsonBlock } from "../dom.js";
import { locale, locales, t } from "../i18n.js";
import { allSidebarRoutes, localizedRoutes, resolveSidebarPreferences } from "../routes.js";
import { state } from "../state.js";
import {
  button,
  chip,
  closurePill,
  dataTable,
  dialog,
  markdownViewer,
  metric,
  pill,
  statusItem,
  tabs,
  uiState,
} from "./shared.js";

const DEFAULT_API_BASE_PRESETS = [
  { name: "Same origin", url: "" },
  { name: "Local 6185", url: "http://127.0.0.1:6185" },
  { name: "localhost 6185", url: "http://localhost:6185" },
];

const GITHUB_PROXY_PRESETS = [
  "https://edgeone.gh-proxy.com",
  "https://hk.gh-proxy.com",
  "https://gh-proxy.com",
  "https://gh.llkk.cc",
];

const API_KEY_SCOPES = [
  ["chat", "Chat"],
  ["file", "File"],
  ["config", "Config"],
  ["im", "IM"],
  ["management.read", "Management read"],
  ["openapi.chat", "OpenAPI chat"],
];

export function renderSettings() {
  const preferences = dashboardPreferences();
  const routes = localizedRoutes();
  const apiKeys = state.apiKeys?.api_keys || [];
  const updateCheck = updateCheckData();
  const backupFiles = backupFileRows();
  const desktop = state.desktopBridge || desktopBridgeSnapshot();
  return `
    <div class="settings-page" data-page="settings">
      <div class="grid cols-4 settings-summary">
        ${metric(t("features.settings.summary.apiBase"), apiBase() || t("features.settings.network.sameOrigin"), "network")}
        ${metric(t("features.settings.summary.apiKeys"), apiKeys.length, "scoped access")}
        ${metric(t("features.settings.summary.backups"), backupFiles.length, "filesystem")}
        ${metric(t("features.settings.summary.version"), updateCheck.current_version || updateCheck.version || "-", updateCheck.has_new_version ? "update available" : "current")}
      </div>
      <div class="settings-layout">
        <section class="panel settings-list-panel">
          <h2>${escapeHtml(t("route.settings.title"))}</h2>
          <div class="settings-list">
            ${renderNetworkSettings(preferences)}
            ${renderApiKeySettings(apiKeys)}
            ${renderMigrationEntry()}
            ${renderSidebarSettings(preferences)}
            ${renderAppearanceSettings(preferences)}
            ${renderDesktopBridgeSettings(desktop)}
            ${renderBackupEntry(backupFiles)}
            ${renderSystemActions(preferences, routes)}
          </div>
        </section>
        <aside class="panel settings-side-panel">
          <h2>${escapeHtml(t("features.settings.runtimeSummary"))}</h2>
          <div class="status-list">
            ${statusItem("连接状态", state.status ? t("core.common.connected") : t("core.common.pending"))}
            ${statusItem("Management Token", managementToken() ? t("core.common.configured") : "not set")}
            ${statusItem("Capabilities", state.capabilities?.services?.length || 0)}
            ${statusItem("当前页面", state.routePath || "/settings")}
            ${statusItem("Theme", preferences.theme)}
            ${statusItem("Proxy", preferences.githubProxyEnabled ? preferences.githubProxyUrl || "enabled" : "disabled")}
            ${statusItem(t("features.settings.desktop.runtime"), desktop.bridgePresent ? "desktop" : t("core.common.browserFallback"))}
          </div>
          ${state.operation ? `<div class="settings-operation"><h3>${escapeHtml(t("features.settings.recentOperation"))}</h3>${jsonBlock(state.operation)}</div>` : ""}
        </aside>
      </div>
      ${renderBackupDialog()}
      ${renderMigrationDialog()}
      ${renderChangelogDialog()}
      ${renderSidebarCustomizerDialog()}
    </div>
  `;
}

export function renderUpdate() {
  return `
    <div class="settings-page" data-page="update">
      ${renderUpdateWorkflow()}
      ${renderMigrationDialog()}
      ${renderChangelogDialog()}
    </div>
  `;
}

export function renderBackup() {
  return `
    <div class="settings-page" data-page="backup">
      ${renderBackupWorkflow()}
    </div>
  `;
}

export function renderAbout() {
  const isLegacyAlkaid = state.routeReplacementFor === "legacy-alkaid"
    || ["/alkaid", "/alkaid/long-term-memory", "/alkaid/other"].includes(state.routeSourcePath);
  const preferences = dashboardPreferences();
  const updateCheck = updateCheckData();
  const currentVersion = updateCheck.current_version || updateCheck.version || state.changelog?.current_version || "-";
  const dashboardVersion = updateCheck.dashboard_version || currentVersion;
  const services = state.capabilities?.services || [];
  const desktop = state.desktopBridge || desktopBridgeSnapshot();
  return `
    <div class="about-page" data-page="about">
      ${isLegacyAlkaid ? renderLegacyAlkaidReplacement() : ""}
      <section class="panel about-hero">
        <div class="about-logo-block">
          <img class="about-logo" src="/assets/images/astrbot_logo_mini.webp" alt="AstrBot Logo" width="110" height="110" />
          <div>
            <div class="eyebrow">AstrBot Dashboard</div>
            <h2>AstrBot</h2>
            <p>A project out of interests and loves.</p>
          </div>
        </div>
        <div class="banner-actions">
          <a class="button" href="https://github.com/AstrBotDevs/AstrBot" target="_blank" rel="noopener noreferrer">Star this project</a>
          <a class="button secondary" href="https://github.com/AstrBotDevs/AstrBot/issues" target="_blank" rel="noopener noreferrer">Submit Issue</a>
          <a class="button ghost" href="https://github.com/AstrBotDevs/AstrBot#readme" target="_blank" rel="noopener noreferrer">README</a>
          ${button({ label: "Changelog", action: "settings-open-changelog", variant: "ghost", icon: "☰" })}
        </div>
      </section>

      <div class="grid cols-4 about-metrics">
        ${metric("AstrBot Version", currentVersion, updateCheck.has_new_version ? "update available" : "current")}
        ${metric("Dashboard Version", dashboardVersion, updateCheck.dashboard_has_new_version ? "dashboard update" : "webui")}
        ${metric("Services", services.length, "capabilities")}
        ${metric("Runtime Uptime", formatDuration(state.stats?.uptime_seconds || 0), "management stats")}
      </div>

      <div class="grid cols-2">
        <section class="panel">
          <h2>项目链接</h2>
          <div class="status-list">
            ${statusItem("Repository", "github.com/AstrBotDevs/AstrBot")}
            ${statusItem("Documentation", "docs.astrbot.app")}
            ${statusItem("License", "AGPL v3")}
            ${statusItem("Copyright", "AstrBotDevs and contributors")}
          </div>
          <div class="actions wrap-actions">
            <a class="button secondary" href="https://docs.astrbot.app" target="_blank" rel="noopener noreferrer">Documentation</a>
            <a class="button ghost" href="https://github.com/AstrBotDevs/AstrBot/releases" target="_blank" rel="noopener noreferrer">GitHub Releases</a>
          </div>
        </section>

        <section class="panel">
          <h2>系统信息</h2>
          <div class="status-list">
            ${statusItem("API Base", apiBase() || "same-origin")}
            ${statusItem("Management Token", managementToken() ? t("core.common.configured") : "not set")}
            ${statusItem("Theme", preferences.theme)}
            ${statusItem("Locale", locale())}
            ${statusItem("Desktop Bridge", desktop.bridgePresent ? "desktop" : t("core.common.browserFallback"))}
            ${statusItem("Current Route", state.routeSourcePath || state.routePath || "/about")}
          </div>
        </section>
      </div>

      <section class="panel">
        <h2>闭环状态</h2>
        <p class="empty">当前前端以 Rust management API 为准复刻 AstrBot Dashboard 的后台信息架构。Plan-only 与 in-memory 服务会在 capabilities 中明确标注，避免把未接真实执行器的功能伪装为 runtime 闭环。</p>
        ${services.length ? `
          <table class="table">
            <thead><tr><th>服务</th><th>层级</th><th>API</th></tr></thead>
            <tbody>
              ${services.map((service) => `
                <tr><td>${escapeHtml(service.label)}</td><td>${closurePill(service.closure_level)}</td><td><code>${escapeHtml(service.api_base)}</code></td></tr>
              `).join("")}
            </tbody>
          </table>
        ` : `<p class="empty">capabilities 未加载。</p>`}
      </section>
      ${renderChangelogDialog()}
    </div>
  `;
}

function renderLegacyAlkaidReplacement() {
  return `
    <section class="panel legacy-alkaid-replacement" data-page="legacy-alkaid-replacement">
      <div class="panel-title-row">
        <div>
          <div class="eyebrow">Legacy Alkaid</div>
          <h2>旧版 Alkaid 插件页面已显式弃用</h2>
        </div>
        <div class="banner-actions">
          <a class="button" href="#/knowledge-base">新版知识库</a>
          <a class="button ghost" href="#/alkaid/knowledge-base">旧版 KB 迁移入口</a>
        </div>
      </div>
      <div class="notice-banner">
        <span>${pill("deprecated", "warn")}</span>
        <span>legacy Alkaid plugin UI 不在 RS Dashboard runtime parity 范围；这些入口会落到可见 replacement 页面，避免死链或假装接通旧插件 API。</span>
      </div>
      <div class="status-list compact">
        ${statusItem("/alkaid", "显式弃用，显示本说明")}
        ${statusItem("/alkaid/long-term-memory", "显式弃用；不执行 /api/plug/alkaid/ltm/*")}
        ${statusItem("/alkaid/other", "显式弃用；源端路由已注释")}
        ${statusItem("/alkaid/knowledge-base", "保留为 legacy KB migration surface")}
      </div>
      <p class="empty">长期记忆图谱、fact 查询和 user_ids 等旧 Alkaid LTM 接口属于插件历史页面：/api/plug/alkaid/ltm/graph/search、/api/plug/alkaid/ltm/graph/add、/api/plug/alkaid/ltm/graph、/api/plug/alkaid/ltm/user_ids、/api/plug/alkaid/ltm/graph/fact。</p>
    </section>
  `;
}

function renderNetworkSettings(preferences) {
  const presets = mergeApiBasePresets(preferences);
  const selectedProxy = preferences.githubProxyUrl || GITHUB_PROXY_PRESETS[0];
  return `
    <section class="settings-row" id="settings-network">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.network.title"))}</h3>
        <p>${escapeHtml(t("features.settings.network.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        <div class="settings-preset-row">
          ${presets.map((preset) => `
            <button class="ui-chip ${apiBase() === preset.url ? "active" : ""}" type="button" data-action="settings-preset-apply" data-url="${escapeHtml(preset.url)}">${escapeHtml(preset.name)}</button>
          `).join("")}
        </div>
        <div class="form-row"><label>${escapeHtml(t("features.settings.network.apiBase"))}</label><input id="api-base" value="${escapeHtml(apiBase())}" placeholder="http://127.0.0.1:6185" /></div>
        <div class="form-grid cols-2">
          <div class="form-row"><label>${escapeHtml(t("features.settings.network.presetName"))}</label><input id="api-preset-name" placeholder="Staging" /></div>
          <div class="form-row"><label>${escapeHtml(t("features.settings.network.presetUrl"))}</label><input id="api-preset-url" placeholder="https://dashboard.example" /></div>
        </div>
        <div class="actions compact-actions">
          ${button({ label: t("features.settings.network.addPreset"), action: "settings-preset-add", variant: "secondary", icon: "+" })}
          ${preferences.apiBasePresets?.length ? button({ label: t("features.settings.network.removeLastCustom"), action: "settings-preset-remove", variant: "ghost", icon: "x", attrs: { "data-name": preferences.apiBasePresets.at(-1)?.name || "" } }) : ""}
        </div>
        <div class="proxy-selector">
          <label class="check-row"><input id="github-proxy-enabled" type="checkbox" ${preferences.githubProxyEnabled ? "checked" : ""} /> ${escapeHtml(t("features.settings.network.useGithubProxy"))}</label>
          <div class="settings-preset-row">
            ${GITHUB_PROXY_PRESETS.map((proxy) => `
              <button class="ui-chip ${selectedProxy === proxy ? "active" : ""}" type="button" data-action="settings-proxy-apply" data-url="${escapeHtml(proxy)}">${escapeHtml(proxy.replace(/^https?:\/\//, ""))}</button>
            `).join("")}
          </div>
          <div class="form-row"><label>${escapeHtml(t("features.settings.network.githubProxyUrl"))}</label><input id="github-proxy-url" value="${escapeHtml(selectedProxy)}" placeholder="https://gh-proxy.example" /></div>
          <div class="actions compact-actions">
            ${button({ label: t("features.settings.network.saveProxy"), action: "settings-proxy-save", variant: "secondary", icon: "✓" })}
            ${button({ label: t("features.settings.network.testProxy"), action: "settings-proxy-test", variant: "ghost", icon: "↗" })}
          </div>
        </div>
      </div>
    </section>
  `;
}

function renderApiKeySettings(apiKeys) {
  return `
    <section class="settings-row" id="settings-api-keys">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.apiKey.title"))}</h3>
        <p>${escapeHtml(t("features.settings.apiKey.subtitle"))}</p>
        <a href="https://docs.astrbot.app/dev/openapi.html" target="_blank" rel="noopener noreferrer">OpenAPI docs</a>
      </div>
      <div class="settings-row-control">
        <div class="form-grid cols-2">
          <div class="form-row"><label>Name</label><input id="api-key-name" value="Dashboard automation" /></div>
          <div class="form-row"><label>Expires in days</label><select id="api-key-expires"><option value="7">7 days</option><option value="30" selected>30 days</option><option value="90">90 days</option><option value="180">180 days</option><option value="365">365 days</option></select></div>
          <div class="form-row"><label>Key ID</label><input id="api-key-id" placeholder="auto" /></div>
          <div class="form-row"><label>Secret</label><input id="api-key-secret" type="password" placeholder="auto-generated" /></div>
        </div>
        <div class="scope-grid">
          ${API_KEY_SCOPES.map(([value, label]) => `
            <label class="check-row"><input type="checkbox" data-api-key-scope="${escapeHtml(value)}" ${["chat", "file", "config", "im"].includes(value) ? "checked" : ""} /> ${escapeHtml(label)} <code>${escapeHtml(value)}</code></label>
          `).join("")}
        </div>
        <input id="api-key-scopes" type="hidden" value="chat,file,config,im" />
        <div class="actions compact-actions">
          ${button({ label: "Create key", action: "api-key-issue", icon: "+" })}
          ${button({ label: "Refresh", action: "load-api-keys", variant: "ghost", icon: "↻" })}
        </div>
        ${state.operation?.issued_api_key ? `<div class="notice warning"><strong>Secret 只显示一次：</strong><code>${escapeHtml(state.operation.secret)}</code></div>` : ""}
        ${renderApiKeyTable(apiKeys)}
      </div>
    </section>
  `;
}

function renderApiKeyTable(apiKeys) {
  if (state.apiKeys?.unavailable) {
    return uiState({ state: "error", message: state.apiKeys.unavailable, compact: true });
  }
  return dataTable({
    id: "settings-api-key-table",
    columns: [
      { key: "name", label: "Name", html: true, render: (key) => `<strong>${escapeHtml(key.name)}</strong><br><span class="metric-label">${escapeHtml(key.key_id)}</span>` },
      { key: "key_prefix", label: "Prefix", html: true, render: (key) => `<code>${escapeHtml(key.key_prefix || "-")}</code>` },
      { key: "scopes", label: "Scopes", html: true, render: (key) => (key.scopes || []).map((scope) => chip(scope, "label")).join(" ") || "-" },
      { key: "active", label: "Status", html: true, render: (key) => key.active ? pill("active", "ok") : pill(key.is_expired ? "expired" : "revoked", "warn") },
      { key: "last_used_at", label: "Last used", render: (key) => formatDate(key.last_used_at) },
      { key: "created_by", label: "Created by" },
      { key: "actions", label: "Actions", html: true, render: (key) => `
        <div class="button-cell">
          ${button({ label: "Revoke", action: "api-key-revoke", variant: "secondary", disabled: !key.active, attrs: { "data-key": key.key_id } })}
          ${button({ label: "Delete", action: "api-key-delete", variant: "ghost", attrs: { "data-key": key.key_id } })}
        </div>
      ` },
    ],
    rows: apiKeys,
    emptyMessage: "暂无 API key。",
    rowKey: "key_id",
  });
}

function renderMigrationEntry() {
  return `
    <section class="settings-row" id="settings-migration">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.migration.title"))}</h3>
        <p>${escapeHtml(t("features.settings.migration.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        ${state.migration?.check ? `<div class="status-list compact">${statusItem("Pending storage migrations", (state.migration.check.pending_storage_migrations || []).length)}${statusItem("Legacy data", state.migration.check.legacy_data_migration_needed ? "needed" : "clean")}</div>` : uiState({ state: "empty", message: "Migration check has not been loaded.", compact: true })}
        ${button({ label: t("features.settings.migration.open"), action: "settings-open-migration", icon: "↻" })}
      </div>
    </section>
  `;
}

function renderSidebarSettings(preferences) {
  const sidebar = resolveSidebarPreferences(preferences);
  return `
    <section class="settings-row" id="settings-sidebar">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.sidebar.title"))}</h3>
        <p>${escapeHtml(t("features.settings.sidebar.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        <div class="status-list compact">
          ${statusItem(t("features.settings.sidebar.mainItems"), sidebar.mainItems.length)}
          ${statusItem(t("features.settings.sidebar.moreItems"), sidebar.moreItems.length)}
        </div>
        <div class="sidebar-preview-list">
          ${sidebar.mainItems.slice(0, 8).map((id) => chip(routeLabel(id), "label")).join("")}
          ${sidebar.moreItems.length ? chip(`${t("core.navigation.groups.more")} +${sidebar.moreItems.length}`, "active") : ""}
        </div>
        <div class="actions compact-actions">
          ${button({ label: t("features.settings.sidebar.open"), action: "settings-sidebar-open", variant: "secondary", icon: "☰" })}
          ${button({ label: t("features.settings.sidebar.reset"), action: "settings-sidebar-reset", variant: "ghost", icon: "↺" })}
        </div>
      </div>
    </section>
  `;
}

function renderAppearanceSettings(preferences) {
  return `
    <section class="settings-row" id="settings-style">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.appearance.title"))}</h3>
        <p>${escapeHtml(t("features.settings.appearance.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        <div class="form-grid cols-2">
          <div class="form-row"><label>${escapeHtml(t("features.settings.appearance.theme"))}</label><select id="dashboard-theme"><option value="light" ${preferences.theme === "dark" ? "" : "selected"}>${escapeHtml(t("features.settings.appearance.light"))}</option><option value="dark" ${preferences.theme === "dark" ? "selected" : ""}>${escapeHtml(t("features.settings.appearance.dark"))}</option></select></div>
          <div class="form-row"><label>${escapeHtml(t("settings.language"))}</label><select id="dashboard-locale">${locales.map((item) => `<option value="${escapeHtml(item.id)}" ${locale() === item.id ? "selected" : ""}>${escapeHtml(item.label)}</option>`).join("")}</select></div>
          <div class="form-row color-field"><label>${escapeHtml(t("features.settings.appearance.primary"))}</label><input id="theme-primary-color" value="${escapeHtml(preferences.primaryColor || "#5b5ce2")}" pattern="#[0-9a-fA-F]{6}" /><span class="color-swatch" style="background:${escapeHtml(preferences.primaryColor || "#5b5ce2")}"></span></div>
          <div class="form-row color-field"><label>${escapeHtml(t("features.settings.appearance.secondary"))}</label><input id="theme-secondary-color" value="${escapeHtml(preferences.secondaryColor || "#0f766e")}" pattern="#[0-9a-fA-F]{6}" /><span class="color-swatch" style="background:${escapeHtml(preferences.secondaryColor || "#0f766e")}"></span></div>
        </div>
        <label class="check-row"><input id="sidebar-compact" type="checkbox" ${preferences.sidebarCompact ? "checked" : ""} /> ${escapeHtml(t("features.settings.appearance.compact"))}</label>
        <div class="actions compact-actions">
          ${button({ label: t("features.settings.appearance.save"), action: "save-dashboard-preferences", icon: "✓" })}
          ${button({ label: t("features.settings.appearance.resetTheme"), action: "settings-reset-theme", variant: "ghost", icon: "↺" })}
        </div>
      </div>
    </section>
  `;
}

function renderDesktopBridgeSettings(desktop) {
  const backend = desktop.backendState || {};
  const update = desktop.appUpdate || {};
  return `
    <section class="settings-row" id="settings-desktop-bridge">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.desktop.title"))}</h3>
        <p>${escapeHtml(t("features.settings.desktop.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        <div class="status-list compact">
          ${statusItem(t("features.settings.desktop.runtime"), desktop.bridgePresent ? "desktop" : t("core.common.browserFallback"))}
          ${statusItem(t("features.settings.desktop.backend"), desktop.hasBackendState ? backend.running === false ? "stopped" : "manageable" : t("core.common.unavailable"))}
          ${statusItem(t("features.settings.desktop.updater"), desktop.updaterPresent ? update.latestVersion || "available" : t("core.common.unavailable"))}
          ${statusItem(t("features.settings.desktop.trayListener"), desktop.hasTrayRestartListener ? "available" : t("core.common.unavailable"))}
        </div>
        ${desktop.bridgePresent ? "" : `<div class="notice">${escapeHtml(t("features.settings.desktop.fallback"))}</div>`}
        ${desktop.checkedAt ? `<p class="empty">${escapeHtml(t("features.settings.desktop.lastProbe", { time: formatDate(desktop.checkedAt) }))}</p>` : ""}
        <div class="actions wrap-actions">
          ${button({ label: t("features.settings.desktop.refresh"), action: "settings-desktop-probe", variant: "secondary", icon: "↻" })}
          ${button({ label: t("features.settings.desktop.restartBackend"), action: "settings-desktop-restart", variant: "ghost", icon: "↻", disabled: !desktop.hasBackendRestart })}
          ${button({ label: t("features.settings.desktop.checkUpdate"), action: "settings-desktop-update-check", variant: "ghost", icon: "↑", disabled: !desktop.updaterPresent })}
          ${button({ label: t("features.settings.desktop.installUpdate"), action: "settings-desktop-update-install", variant: "ghost", icon: "✓", disabled: !desktop.updaterPresent })}
        </div>
        ${state.desktopBridge ? jsonBlock(state.desktopBridge) : ""}
      </div>
    </section>
  `;
}

function renderBackupEntry(backupFiles) {
  return `
    <section class="settings-row" id="settings-backup">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.backup.title"))}</h3>
        <p>${escapeHtml(t("features.settings.backup.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        <div class="status-list compact">
          ${statusItem("Backup files", backupFiles.length)}
          ${statusItem("Last task", state.backupTask?.task?.task_id || state.backupTask?.operation?.operation_id || "-")}
        </div>
        ${button({ label: t("features.settings.backup.open"), action: "settings-open-backup", icon: "⇅" })}
      </div>
    </section>
  `;
}

function renderSystemActions(preferences, routes) {
  return `
    <section class="settings-row" id="settings-system">
      <div class="settings-row-copy">
        <h3>${escapeHtml(t("features.settings.system.title"))}</h3>
        <p>${escapeHtml(t("features.settings.system.subtitle"))}</p>
      </div>
      <div class="settings-row-control">
        <div class="status-list compact">
          ${statusItem(t("features.settings.system.routes"), routes.length)}
          ${statusItem(t("features.settings.system.token"), managementToken() ? t("core.common.configured") : "not set")}
          ${statusItem(t("features.settings.system.primary"), preferences.primaryColor || "default")}
        </div>
        <div class="actions wrap-actions">
          ${button({ label: t("features.settings.system.changelog"), action: "settings-open-changelog", variant: "secondary", icon: "☰" })}
          ${button({ label: t("features.settings.system.restartPlan"), action: "restart-plan", variant: "secondary", icon: "↻" })}
          ${button({ label: t("features.settings.system.restartNow"), action: "restart-run", variant: "ghost", icon: "↻" })}
          ${button({ label: t("features.settings.system.logout"), action: "clear-management-token", variant: "danger", icon: "⇥" })}
        </div>
        <input id="restart-reason" value="settings page request" hidden />
      </div>
    </section>
  `;
}

function renderBackupDialog() {
  return dialog({
    id: "backup-dialog",
    title: "Backup",
    maxWidth: "920px",
    open: state.settingsDialog === "backup",
    body: renderBackupWorkflow({ embedded: true }),
  });
}

function renderMigrationDialog() {
  return dialog({
    id: "migration-dialog",
    title: "Migration",
    maxWidth: "760px",
    open: state.settingsDialog === "migration",
    body: renderMigrationWorkflow(),
  });
}

function renderChangelogDialog() {
  return dialog({
    id: "changelog-dialog",
    title: "Changelog",
    maxWidth: "820px",
    open: state.settingsDialog === "changelog",
    body: renderChangelogWorkflow(),
  });
}

function renderSidebarCustomizerDialog() {
  const draft = sidebarDraft();
  return dialog({
    id: "sidebar-customizer-dialog",
    title: t("features.settings.sidebar.customizeTitle"),
    maxWidth: "840px",
    open: state.settingsDialog === "sidebar-customizer",
    closeAction: "settings-dialog-close",
    body: `
      <p class="empty">${escapeHtml(t("features.settings.sidebar.customizeSubtitle"))}</p>
      <div class="sidebar-customizer-grid">
        ${renderSidebarBucket("main", t("features.settings.sidebar.mainItems"), draft.mainItems)}
        ${renderSidebarBucket("more", t("features.settings.sidebar.moreItems"), draft.moreItems)}
      </div>
    `,
    actions: [
      { label: t("features.settings.sidebar.reset"), action: "settings-sidebar-reset", variant: "ghost" },
      { label: t("features.settings.sidebar.save"), action: "settings-sidebar-save", variant: "primary" },
    ],
  });
}

function renderSidebarBucket(list, title, ids) {
  return `
    <section class="sidebar-customizer-bucket" data-sidebar-list="${escapeHtml(list)}">
      <h3>${escapeHtml(title)}</h3>
      <div class="sidebar-customizer-list">
        ${ids.length ? ids.map((id, index) => renderSidebarCustomizerItem(list, id, index, ids.length)).join("") : `<p class="empty">${escapeHtml(t("features.settings.sidebar.empty"))}</p>`}
      </div>
    </section>
  `;
}

function renderSidebarCustomizerItem(list, id, index, total) {
  const toAction = list === "main" ? "settings-sidebar-move-more" : "settings-sidebar-move-main";
  const toLabel = list === "main" ? t("core.actions.moveToMore") : t("core.actions.moveToMain");
  return `
    <div class="sidebar-customizer-item" data-route-id="${escapeHtml(id)}">
      <span class="sidebar-customizer-icon">${escapeHtml(routeIcon(id))}</span>
      <strong>${escapeHtml(routeLabel(id))}</strong>
      <div class="sidebar-customizer-actions">
        ${button({ label: t("core.actions.moveUp"), action: "settings-sidebar-move-up", variant: "ghost", disabled: index === 0, attrs: { "data-route-id": id, "data-list": list } })}
        ${button({ label: t("core.actions.moveDown"), action: "settings-sidebar-move-down", variant: "ghost", disabled: index === total - 1, attrs: { "data-route-id": id, "data-list": list } })}
        ${button({ label: toLabel, action: toAction, variant: "secondary", attrs: { "data-route-id": id } })}
      </div>
    </div>
  `;
}

export function renderBackupWorkflow({ embedded = false } = {}) {
  const files = backupFileRows();
  const taskId = state.backupTask?.task?.task_id || "backup-demo";
  const fileTable = dataTable({
    id: embedded ? "backup-dialog-files" : "backup-files",
    columns: [
      { key: "filename", label: "File", html: true, render: (file) => `<strong>${escapeHtml(file.filename)}</strong><br><span class="metric-label">${escapeHtml(file.modified_at_unix ? new Date(file.modified_at_unix * 1000).toISOString() : file.created_at || "-")}</span>` },
      { key: "size", label: "Size", render: (file) => formatBytes(file.size_bytes ?? file.size ?? 0) },
      { key: "version", label: "Version", render: (file) => file.astrbot_version || file.version || "-" },
      { key: "actions", label: "Actions", html: true, render: (file) => `
        <div class="button-cell">
          ${button({ label: "Restore", action: "backup-file-restore", variant: "secondary", attrs: { "data-filename": file.filename } })}
          ${button({ label: "Rename", action: "backup-file-rename", variant: "secondary", attrs: { "data-filename": file.filename } })}
          ${button({ label: "Download", action: "backup-file-download", variant: "ghost", attrs: { "data-filename": file.filename } })}
          ${button({ label: "Delete", action: "backup-file-delete", variant: "ghost", attrs: { "data-filename": file.filename } })}
        </div>
      ` },
    ],
    rows: files,
    emptyMessage: "备份目录暂无文件。",
    rowKey: "filename",
  });
  return `
    <section class="maintenance-workflow backup-workflow">
      ${tabs({
        id: embedded ? "backup-dialog-tabs" : "backup-route-tabs",
        activeId: "export",
        items: [
          { id: "export", label: "Export", body: `
            <div class="settings-dialog-grid">
              <div>
                <h3>Export backup</h3>
                <p class="empty">Exports SQLite tables and configured runtime directories into a downloadable zip manifest.</p>
                <div class="form-row"><label>Task ID</label><input id="backup-task-id" value="${escapeHtml(taskId)}" /></div>
                <div class="actions compact-actions">
                  ${button({ label: "Export", action: "backup-export", icon: "⇧" })}
                  ${button({ label: "Progress", action: "backup-progress", variant: "ghost", icon: "↻" })}
                </div>
              </div>
              <div>${state.backupTask ? jsonBlock(state.backupTask) : uiState({ state: "empty", message: "No backup task result yet.", compact: true })}</div>
            </div>
          ` },
          { id: "import", label: "Import", body: `
            <div class="settings-dialog-grid">
              <div>
                <h3>Import or upload backup</h3>
                <div class="form-row"><label>Upload ID</label><input id="upload-id" value="upload-demo" /></div>
                <div class="form-row"><label>Filename</label><input id="upload-filename" value="backup.zip" /></div>
                <div class="form-grid cols-2">
                  <div class="form-row"><label>Total size</label><input id="upload-size" value="1048577" /></div>
                  <div class="form-row"><label>Chunk bytes</label><input id="upload-chunk-bytes" value="1048576" /></div>
                </div>
                <input id="upload-chunk-index" value="0" hidden />
                <div class="actions wrap-actions">
                  ${button({ label: "Start upload", action: "backup-upload-start", variant: "secondary", icon: "+" })}
                  ${button({ label: "Chunk", action: "backup-upload-chunk", variant: "secondary", icon: "•" })}
                  ${button({ label: "Complete", action: "backup-upload-complete", variant: "ghost", icon: "✓" })}
                  ${button({ label: "Abort", action: "backup-upload-abort", variant: "ghost", icon: "x" })}
                  ${button({ label: "Import", action: "backup-import", icon: "⇩" })}
                </div>
              </div>
              <div>
                <h3>Restore options</h3>
                <div class="form-row"><label>Restore task ID</label><input id="backup-restore-task-id" value="restore-dashboard" /></div>
                <div class="form-row"><label>Restore mode</label><select id="backup-restore-mode"><option value="Merge">Merge</option><option value="Replace">Replace</option></select></div>
                <div class="form-row"><label>New filename</label><input id="backup-new-filename" value="backup-renamed.zip" /></div>
              </div>
            </div>
          ` },
          { id: "list", label: "List", body: `
            <div class="panel-title-row">
              <h3>Backup files</h3>
              ${button({ label: "Refresh", action: "backup-files-refresh", variant: "ghost", icon: "↻" })}
            </div>
            ${state.backupFiles?.unavailable ? uiState({ state: "error", message: state.backupFiles.unavailable, compact: true }) : fileTable}
          ` },
        ],
      })}
    </section>
  `;
}

export function renderUpdateWorkflow() {
  const check = updateCheckData();
  const releases = state.releases?.releases || [];
  return `
    <div class="grid cols-3">
      ${metric("当前版本", check.current_version || check.version || "-", check.has_new_version ? "有新版本" : "当前版本")}
      ${metric("最新版本", check.latest_version || "-", "release service")}
      ${metric("Dashboard", check.dashboard_version || "-", check.dashboard_has_new_version ? "需更新" : "无需更新")}
    </div>
    <div class="grid cols-2">
      <section class="panel">
        <h2>Update</h2>
        ${state.update?.unavailable ? uiState({ state: "error", message: state.update.unavailable, compact: true }) : ""}
        <div class="form-row"><label>Version</label><input id="update-version" value="${escapeHtml(check.latest_version || check.current_version || check.version || "0.1.0")}" /></div>
        <div class="form-row"><label>Proxy</label><input id="update-proxy" value="${escapeHtml(dashboardPreferences().githubProxyUrl || "")}" placeholder="https://proxy.example" /></div>
        <div class="form-row"><label>Operation ID</label><input id="operation-id" value="${escapeHtml(state.operation?.operation?.operation_id || "project-update-v4.1.0")}" /></div>
        <div class="actions wrap-actions">
          ${button({ label: "Project plan", action: "project-plan", icon: "↑" })}
          ${button({ label: "Dashboard plan", action: "dashboard-plan", variant: "secondary", icon: "↑" })}
          ${button({ label: "Run", action: "operation-run", variant: "secondary", icon: "▶" })}
          ${button({ label: "Poll", action: "operation-get", variant: "ghost", icon: "↻" })}
          ${button({ label: "List", action: "operation-list", variant: "ghost", icon: "☰" })}
        </div>
      </section>
      <section class="panel">
        <h2>Package / Restart</h2>
        <div class="form-row"><label>Package</label><input id="package-name" value="requests==2.32.0" /></div>
        <div class="form-row"><label>Mirror</label><input id="package-mirror" placeholder="https://mirror.example/simple" /></div>
        <div class="form-row"><label>Restart reason</label><input id="restart-reason" value="dashboard maintenance" /></div>
        <div class="actions wrap-actions">
          ${button({ label: "Package plan", action: "package-plan", variant: "secondary", icon: "□" })}
          ${button({ label: "Install", action: "package-run", icon: "✓" })}
          ${button({ label: "Restart plan", action: "restart-plan", variant: "secondary", icon: "↻" })}
          ${button({ label: "Restart run", action: "restart-run", variant: "ghost", icon: "↻" })}
        </div>
      </section>
    </div>
    <div class="grid cols-2">
      <section class="panel"><h2>Releases</h2>${releases.length ? jsonBlock(releases) : uiState({ state: "empty", message: "暂无 release 元数据。", compact: true })}</section>
      <section class="panel"><h2>Migration Check</h2>${state.migration?.check ? jsonBlock(state.migration.check) : uiState({ state: "empty", message: "迁移检查不可用。", compact: true })}</section>
    </div>
    <section class="panel">
      <div class="panel-title-row">
        <h2>Changelog</h2>
        ${button({ label: "Open Changelog", action: "settings-open-changelog", variant: "ghost", icon: "☰" })}
      </div>
      ${renderChangelogWorkflow({ compact: true })}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近 Operation</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

function renderMigrationWorkflow() {
  const check = state.migration?.check;
  return `
    <div class="settings-dialog-grid">
      <div>
        <p class="empty">Select platform mappings before running Python v4 data/config migration. Existing Rust storage migrations are reported by the check endpoint.</p>
        ${check ? `<div class="status-list compact">${statusItem("Pending", (check.pending_storage_migrations || []).join(", ") || "none")}${statusItem("Legacy data", check.legacy_data_migration_needed ? "needed" : "clean")}</div>` : uiState({ state: "empty", message: "Migration check has not been loaded.", compact: true })}
        <div class="form-row"><label>Platform ID map JSON</label><textarea id="migration-platform-map" rows="8">{
  "webchat": { "platform_id": "webchat", "platform_type": "webchat" }
}</textarea></div>
      </div>
      <div>
        <h3>Actions</h3>
        <input id="operation-id" value="${escapeHtml(state.operation?.operation?.operation_id || "migration")}" />
        <div class="actions wrap-actions">
          ${button({ label: "Migration plan", action: "migration-plan", icon: "↻" })}
          ${button({ label: "Run operation", action: "operation-run", variant: "secondary", icon: "▶" })}
          ${button({ label: "Poll", action: "operation-get", variant: "ghost", icon: "↻" })}
        </div>
        ${state.operation ? jsonBlock(state.operation) : ""}
      </div>
    </div>
  `;
}

function renderChangelogWorkflow({ compact = false } = {}) {
  const changelog = state.changelog || {};
  const releases = changelog.releases || [];
  const current = changelog.current_version || updateCheckData().current_version || "";
  const selected = releases[0] || {};
  const markdown = releaseMarkdown(selected);
  return `
    <div class="${compact ? "changelog-compact" : "settings-dialog-grid"}">
      <div>
        <div class="form-row"><label>Version</label><select id="changelog-version">
          ${releases.map((release) => `<option value="${escapeHtml(release.version || release.tag || release.title)}">${escapeHtml(release.version || release.tag || release.title)}${release.version === current ? " (current)" : ""}</option>`).join("")}
        </select></div>
        ${releases.length ? `<div class="settings-preset-row">${releases.slice(0, 5).map((release) => chip(release.version || release.title || "release", release.version === current ? "active" : "label")).join("")}</div>` : uiState({ state: "empty", message: "暂无 changelog 元数据。", compact: true })}
      </div>
      <div>
        ${markdownViewer({ markdown, emptyMessage: "No changelog content." })}
      </div>
    </div>
  `;
}

function updateCheckData() {
  return state.update?.check || state.update || {};
}

function backupFileRows() {
  return state.backupFiles?.files || state.backupFiles?.items || [];
}

function sidebarDraft() {
  if (state.sidebarCustomizerDraft) return state.sidebarCustomizerDraft;
  const resolved = resolveSidebarPreferences(dashboardPreferences());
  return {
    mainItems: [...resolved.mainItems],
    moreItems: [...resolved.moreItems],
  };
}

function routeLabel(routeId) {
  return allSidebarRoutes().find((route) => route.id === routeId)?.label || routeId;
}

function routeIcon(routeId) {
  return allSidebarRoutes().find((route) => route.id === routeId)?.icon || "•";
}

function mergeApiBasePresets(preferences) {
  const custom = preferences.apiBasePresets || [];
  const seen = new Set();
  return [...DEFAULT_API_BASE_PRESETS, ...custom].filter((preset) => {
    const key = `${preset.name}:${preset.url}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function releaseMarkdown(release) {
  return release.body || release.notes || release.markdown || release.changelog || release.description || (release.title ? `## ${release.title}` : "");
}

function formatBytes(value) {
  const bytes = Number(value || 0);
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatDuration(seconds) {
  const value = Number(seconds || 0);
  if (value < 60) return `${value}s`;
  const minutes = Math.floor(value / 60);
  const remaining = value % 60;
  if (minutes < 60) return `${minutes}m ${remaining}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

function formatDate(value) {
  if (!value) return "-";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? String(value) : date.toLocaleString();
}
