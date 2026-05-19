import { handleAction } from "./src/actions/handler.js";
import { handlePersonaDragOver, handlePersonaDragStart, handlePersonaDrop } from "./src/actions/personas-cron.js";
import { apiBase, applyDashboardPreferences, dashboardPreferences, managementToken } from "./src/api.js";
import { $, bindUiInteractions, escapeHtml, showToast } from "./src/dom.js";
import { locale, locales, setLocale, t } from "./src/i18n.js";
import { loadCore, routeBeforeRender } from "./src/loaders.js";
import {
  dashboardRouteById,
  guardDashboardRoute,
  localizedRouteGroups,
  localizedRoutes,
  routeStateFromLocation,
  routeStateFromRouteId,
  routeStateFromRouteInput,
} from "./src/routes.js";
import { renderChat, renderChatBox, renderConversation, renderProjects, renderSessions } from "./src/render/data.js";
import { renderConfig } from "./src/render/config.js";
import { renderMarket, renderPlatforms, renderPlugins, renderProviders, renderSkills, renderTools } from "./src/render/integrations.js";
import { renderKnowledge } from "./src/render/knowledge.js";
import { renderBackup, renderUpdate } from "./src/render/maintenance.js";
import { renderCron, renderConsole, renderPersonas, renderTrace } from "./src/render/operations.js";
import { renderOverview } from "./src/render/overview.js";
import { renderAbout, renderSettings } from "./src/render/settings.js";
import { renderSubAgent } from "./src/render/subagents.js";
import { state } from "./src/state.js";

function renderNav() {
  $("#nav").innerHTML = localizedRouteGroups()
    .map(
      (group) => `
        <div class="nav-group">
          <div class="nav-heading">${escapeHtml(group.title)}</div>
          ${group.routes
            .map(
              (route) => `
                <button type="button" class="${state.route === route.id ? "active" : ""}" data-route="${route.id}" title="${escapeHtml(route.label)}">
                  <span class="nav-icon">${route.icon}</span>
                  <span>${route.label}</span>
                </button>
              `,
            )
            .join("")}
        </div>
      `,
    )
    .join("");
}

function setRoute(routeId) {
  const next = routeStateFromRouteId(routeId);
  applyRouteState(next);
  window.location.hash = next.hash;
  render();
}

function render() {
  const routes = localizedRoutes();
  const route = routes.find((item) => item.id === state.route) || dashboardRouteById(state.route) || routes[0];
  const copy = routeCopyForCurrentSource(route);
  applyLayout(route.layout || state.routeLayout || "full");
  $("#page-title").textContent = copy.title;
  $("#page-subtitle").textContent = copy.subtitle;
  syncTopbarControls();
  renderNav();

  const renderers = {
    overview: renderOverview,
    chat: renderChat,
    chatbox: renderChatBox,
    conversation: renderConversation,
    config: renderConfig,
    providers: renderProviders,
    platforms: renderPlatforms,
    plugins: renderPlugins,
    market: renderMarket,
    skills: renderSkills,
    subagent: renderSubAgent,
    knowledge: renderKnowledge,
    sessions: renderSessions,
    projects: renderProjects,
    tools: renderTools,
    update: renderUpdate,
    backup: renderBackup,
    console: renderConsole,
    trace: renderTrace,
    personas: renderPersonas,
    cron: renderCron,
    settings: renderSettings,
    about: renderAbout,
    login: renderLogin,
  };
  $("#content").innerHTML = (renderers[state.route] || renderOverview)();
}

function routeCopyForCurrentSource(route) {
  if (state.route !== "overview") {
    return route;
  }
  if (state.routeSourcePath === "/dashboard/default") {
    return {
      ...route,
      title: "Default Dashboard",
      subtitle: "消息、平台、运行时间和资源使用统计",
    };
  }
  return {
    ...route,
    title: "Welcome",
    subtitle: "后端连接、平台与 Provider 引导",
  };
}

function applyLayout(layout) {
  const blank = layout === "blank";
  document.body.dataset.layout = blank ? "blank" : "full";
  toggleLayoutShellElement("[data-dashboard-topbar], .topbar", "topbar", blank, "dashboardTopbar");
  toggleLayoutShellElement("[data-dashboard-sidebar], .sidebar", "sidebar", blank, "dashboardSidebar");
}

function toggleLayoutShellElement(selector, className, hidden, datasetKey) {
  const element = document.querySelector(selector);
  if (!element) return;
  element.dataset[datasetKey] = "true";
  element.hidden = hidden;
  element.classList.toggle(className, !hidden);
}

function applyRouteState(routeState) {
  state.route = routeState.id;
  state.routePath = routeState.path;
  state.routeSourcePath = routeState.sourcePath || routeState.path;
  state.routeLayout = routeState.layout;
  state.routeParams = routeState.params || {};
  state.routeFragment = routeState.fragment || "";
  state.routeReturnUrl = routeState.returnUrl || state.routeReturnUrl || "";
  state.routeReplacementFor = routeState.replacementFor || "";
  state.routeNotFound = Boolean(routeState.notFound);
  if (routeState.id === "config") {
    state.configMode = routeState.fragment === "system" ? "system" : "normal";
  }
  if ((routeState.id === "chat" || routeState.id === "chatbox") && routeState.params?.conversationId) {
    state.chat.conversationId = routeState.params.conversationId;
  }
}

function renderLogin() {
  const preferences = dashboardPreferences();
  return `
    <div class="login-shell">
      <section class="login-panel">
        <div class="brand compact">
          <div class="brand-mark">A</div>
          <div>
            <div class="brand-title">AstrBot RS</div>
            <div class="brand-subtitle">Dashboard</div>
          </div>
        </div>
        <h1>登录 Dashboard</h1>
        <p class="empty">Return URL: ${escapeHtml(state.routeReturnUrl || "/welcome")}</p>
        <div class="form-row"><label>API Base</label><input id="api-base" value="${escapeHtml(apiBase())}" placeholder="同源留空，或填写 http://127.0.0.1:6185" /></div>
        <div class="form-row"><label>Management Token</label><input id="management-token" type="password" autocomplete="current-password" /></div>
        <div class="form-grid cols-2">
          <div class="form-row"><label>Theme</label><select id="dashboard-theme"><option value="light" ${preferences.theme === "dark" ? "" : "selected"}>Light</option><option value="dark" ${preferences.theme === "dark" ? "selected" : ""}>Dark</option></select></div>
          <label class="check-row"><input id="sidebar-compact" type="checkbox" ${preferences.sidebarCompact ? "checked" : ""} /> Compact sidebar</label>
        </div>
        <div class="actions">
          <button class="button" type="button" data-action="save-management-token">登录</button>
          <button class="button ghost" type="button" data-action="clear-management-token">清除</button>
        </div>
      </section>
    </div>
  `;
}

function renderError(error) {
  const unauthorized = error.message.startsWith("未授权");
  const token = managementToken();
  const preferences = dashboardPreferences();
  $("#content").innerHTML = `
    <div class="panel">
      <h2>${unauthorized ? "登录 Dashboard" : "请求失败"}</h2>
      <p class="empty">${escapeHtml(error.message)}</p>
      ${!token || ["secret", "changeme", "password", "admin"].includes(token.toLowerCase()) ? `<p class="empty">当前 Management Token 为空或接近默认凭据，请在服务端配置非默认 token 后再保存。</p>` : ""}
      <div class="form-row"><label>API Base</label><input id="api-base" value="${escapeHtml(apiBase())}" placeholder="同源留空，或填写 http://127.0.0.1:6185" /></div>
      <div class="form-row"><label>Management Token</label><input id="management-token" type="password" autocomplete="current-password" /></div>
      <div class="form-grid cols-2">
        <div class="form-row"><label>Theme</label><select id="dashboard-theme"><option value="light" ${preferences.theme === "dark" ? "" : "selected"}>Light</option><option value="dark" ${preferences.theme === "dark" ? "selected" : ""}>Dark</option></select></div>
        <label class="check-row"><input id="sidebar-compact" type="checkbox" ${preferences.sidebarCompact ? "checked" : ""} /> Compact sidebar</label>
      </div>
      <div class="actions">
        <button class="button" type="button" data-action="save-management-token">保存</button>
        <button class="button ghost" type="button" data-action="clear-management-token">清除</button>
      </div>
    </div>
  `;
}

async function refresh() {
  if (applyAuthGuard()) return;
  try {
    const route = dashboardRouteById(state.route);
    if (route?.requiresAuth) {
      await loadCore();
    } else if (managementToken()) {
      await loadCore().catch(() => null);
    }
    await routeBeforeRender(state.route);
    render();
  } catch (error) {
    if (error.message.startsWith("未授权")) {
      state.routeReturnUrl = state.routePath || "/welcome";
      const login = routeStateFromRouteInput(`/auth/login?returnUrl=${encodeURIComponent(state.routeReturnUrl)}`);
      applyRouteState(login);
      window.location.hash = login.hash;
      showToast(error.message, "error");
      render();
    } else {
      showToast(error.message, "error");
      renderError(error);
    }
  }
}

function applyAuthGuard() {
  const route = routeStateFromRouteId(state.route, {
    path: state.routePath,
    params: state.routeParams,
    fragment: state.routeFragment,
    returnUrl: state.routeReturnUrl,
  });
  route.sourcePath = state.routeSourcePath || route.sourcePath;
  const guarded = guardDashboardRoute(route, managementToken());
  if (guarded.action !== "redirect") return false;
  applyRouteState(guarded.target);
  window.location.hash = guarded.target.hash;
  render();
  return true;
}

document.addEventListener("click", async (event) => {
  const nav = event.target.closest("[data-route]");
  if (nav) {
    const next = nav.dataset.route;
    if (!confirmConfigNavigation(next)) {
      event.preventDefault();
      return;
    }
    try {
      applyRouteState(routeStateFromRouteId(next));
      setMobileNavOpen(false);
      if (applyAuthGuard()) return;
      await routeBeforeRender(state.route);
      setRoute(state.route);
    } catch (error) {
      showToast(error.message, "error");
      setRoute(next);
    }
    return;
  }
  await handleAction(event, render);
  if (event.target.closest('[data-action="save-management-token"]') && state.route === "login" && managementToken()) {
    const target = routeStateFromRouteInput(state.routeReturnUrl || "/welcome");
    applyRouteState(target);
    window.location.hash = target.hash;
  }
});

document.addEventListener("input", (event) => {
  if (handleProjectDialogTitleInput(event.target)) return;
  if (handleProviderModelSearchInput(event.target)) return;
  if (handleKnowledgeDocumentSearchInput(event.target)) return;
  markConfigDirty(event.target);
});

document.addEventListener("change", (event) => {
  if (event.target?.matches?.("#topbar-locale")) {
    setLocale(event.target.value);
    showToast(t("messages.success.localeChanged"));
    render();
    return;
  }
  markConfigDirty(event.target);
});

document.addEventListener("keydown", async (event) => {
  if (!event.target?.matches?.("#chat-text")) return;
  if (state.route !== "chat" && state.route !== "chatbox") return;
  if (event.key !== "Enter") return;
  const shortcut = state.chat.sendShortcut || "shift_enter";
  const shouldSend = event.ctrlKey || event.metaKey || (shortcut === "enter" ? !event.shiftKey : event.shiftKey);
  if (!shouldSend) return;
  event.preventDefault();
  if (event.target.value.trim() === "/astr_live_dev") {
    state.chat.text = "";
    state.chat.liveModeOpen = true;
    render();
    return;
  }
  document.querySelector('[data-action="send-chat"]')?.click();
});

document.addEventListener("dragstart", (event) => {
  handlePersonaDragStart(event);
});

document.addEventListener("dragover", (event) => {
  handlePersonaDragOver(event);
});

document.addEventListener("drop", async (event) => {
  try {
    if (await handlePersonaDrop(event)) {
      render();
    }
  } catch (error) {
    showToast(error.message, "error");
  }
});

window.addEventListener("beforeunload", (event) => {
  if (state.configDirty && state.route === "config") {
    event.preventDefault();
    event.returnValue = "";
  }
});

window.addEventListener("beforeunload", (event) => {
  if (!state.configDirty) return;
  event.preventDefault();
  event.returnValue = "";
});

window.addEventListener("hashchange", async () => {
  applyRouteState(routeStateFromLocation());
  if (applyAuthGuard()) return;
  try {
    await routeBeforeRender(state.route);
  } catch (error) {
    showToast(error.message, "error");
  }
  render();
});

$("#mobile-sidebar-toggle")?.addEventListener("click", () => {
  setMobileNavOpen(!document.body.classList.contains("nav-open"));
});

$("#drawer-scrim")?.addEventListener("click", () => {
  setMobileNavOpen(false);
});

window.addEventListener("sidebar-customization-changed", () => {
  renderNav();
});

applyDashboardPreferences();
bindUiInteractions(document);

$("#refresh-button").addEventListener("click", refresh);

await refresh();

function syncTopbarControls() {
  const language = $("#topbar-locale");
  if (language) {
    language.innerHTML = locales
      .map((item) => `<option value="${escapeHtml(item.id)}" ${locale() === item.id ? "selected" : ""}>${escapeHtml(item.flag)} ${escapeHtml(item.label)}</option>`)
      .join("");
    language.setAttribute("aria-label", t("core.common.language"));
  }
  const refreshButton = $("#refresh-button");
  if (refreshButton) {
    refreshButton.textContent = t("core.common.refresh");
  }
}

function setMobileNavOpen(open) {
  document.body.classList.toggle("nav-open", Boolean(open));
  const scrim = $("#drawer-scrim");
  if (scrim) {
    scrim.hidden = !open;
  }
}

function confirmConfigNavigation(nextRouteId) {
  if (state.route !== "config" || !state.configDirty || nextRouteId === "config") {
    return true;
  }
  if (typeof window.confirm !== "function") {
    return true;
  }
  return window.confirm("当前配置有未保存更改，确定要离开吗？");
}

function markConfigDirty(target) {
  if (state.route !== "config" || !target?.closest?.("[data-page='config']")) return;
  if (target.matches("#config-search, #config-abconf-select, #config-new-name, #config-edit-id, #config-edit-name, #config-route-pattern, #config-route-config-id, #config-route-umo, #t2i-template-select, #t2i-template-new-name")) {
    return;
  }
  state.configDirty = true;
}

function handleProviderModelSearchInput(target) {
  if (state.route !== "providers" || !target?.matches?.("#provider-model-search")) return false;
  state.providerModelSearch = target.value || "";
  render();
  return true;
}

function handleKnowledgeDocumentSearchInput(target) {
  if (state.route !== "knowledge" || !target?.matches?.("#kb-document-search")) return false;
  state.kbDocumentSearch = target.value || "";
  render();
  return true;
}

function handleProjectDialogTitleInput(target) {
  if (!target?.matches?.("#project-title")) return false;
  const dialog = target.closest("#chat-project-dialog");
  if (!dialog) return false;
  const saveButton = dialog.querySelector("#project-dialog-save");
  if (saveButton) {
    saveButton.disabled = !target.value.trim();
  }
  return false;
}
