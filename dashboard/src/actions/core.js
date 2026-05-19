import {
  api,
  apiBase,
  applyDashboardPreferences,
  checkDesktopAppUpdate,
  dashboardPreferences,
  installDesktopAppUpdate,
  openApi,
  probeDesktopBridge,
  restartDesktopBackend,
  setApiBase,
  setDashboardPreferences,
  setManagementToken,
  setOpenApiSecret,
} from "../api.js";
import { $, showToast } from "../dom.js";
import { setLocale, t } from "../i18n.js";
import { loadApiKeys, loadBackup, loadChatControls, loadConfig, loadConversations, loadCore, loadLogs, loadMessages, loadProjectSessions, loadUpdate } from "../loaders.js";
import { defaultSidebarCustomization, resolveSidebarPreferences } from "../routes.js";
import { state } from "../state.js";

const DEFAULT_T2I_TEMPLATE = `<!doctype html>
<html>
<head>
  <meta charset="utf-8"/>
  <title>New Template</title>
</head>
<body>
  <article>{{ text | safe }}</article>
  <footer>{{ version }}</footer>
</body>
</html>
`;

export async function handleCoreActions({ action, target }) {
    if (action === "refresh-core") await loadCore();
    if (action === "save-management-token") {
      if ($("#api-base")) setApiBase($("#api-base").value);
      saveDashboardPreferencesFromForm();
      setManagementToken($("#management-token").value);
      await loadCore();
      showToast("Management token 已保存");
    }
    if (action === "save-dashboard-preferences") {
      if ($("#api-base-preset")?.value) {
        $("#api-base").value = $("#api-base-preset").value;
      }
      if ($("#api-base")) setApiBase($("#api-base").value);
      saveDashboardPreferencesFromForm();
      await loadCore();
      showToast(t("messages.success.preferencesSaved"));
    }
    if (action === "clear-management-token") {
      setManagementToken("");
      showToast("Management token 已清除");
    }
    if (action === "settings-preset-apply") {
      const url = target.dataset.url || "";
      if ($("#api-base")) $("#api-base").value = url;
      setApiBase(url);
      showToast("API Base preset 已应用");
    }
    if (action === "settings-preset-add") {
      const name = $("#api-preset-name")?.value.trim();
      const url = $("#api-preset-url")?.value.trim();
      if (!name || !url) throw new Error("请填写 preset 名称和 URL");
      const preferences = dashboardPreferences();
      const apiBasePresets = [
        ...(preferences.apiBasePresets || []).filter((preset) => preset.name !== name),
        { name, url },
      ];
      setDashboardPreferences({ ...preferences, apiBasePresets });
      showToast("API Base preset 已保存");
    }
    if (action === "settings-preset-remove") {
      const preferences = dashboardPreferences();
      setDashboardPreferences({
        ...preferences,
        apiBasePresets: (preferences.apiBasePresets || []).filter((preset) => preset.name !== target.dataset.name),
      });
      showToast("API Base preset 已删除");
    }
    if (action === "settings-proxy-save") {
      const preferences = dashboardPreferences();
      setDashboardPreferences({
        ...preferences,
        githubProxyEnabled: $("#github-proxy-enabled")?.checked || false,
        githubProxyUrl: $("#github-proxy-url")?.value || "",
      });
      showToast("GitHub proxy 设置已保存");
    }
    if (action === "settings-proxy-apply") {
      if ($("#github-proxy-url")) $("#github-proxy-url").value = target.dataset.url || "";
      showToast("GitHub proxy preset 已填入");
    }
    if (action === "settings-proxy-test") {
      const proxyUrl = $("#github-proxy-url")?.value.trim() || "";
      if (!proxyUrl) throw new Error("请先填写 Proxy URL");
      const result = await api("/api/stat/test-ghproxy-connection", {
        method: "POST",
        body: JSON.stringify({ proxy_url: proxyUrl }),
      });
      state.operation = {
        kind: "ghproxy_test",
        proxy_url: proxyUrl,
        status: result.status || "ok",
        latency: result.data?.latency ?? result.latency ?? null,
      };
      showToast(state.operation.latency == null ? "Proxy 测试已完成" : `Proxy 可用：${state.operation.latency}ms`);
    }
    if (action === "settings-reset-theme") {
      const preferences = dashboardPreferences();
      setDashboardPreferences({
        ...preferences,
        theme: "light",
        sidebarCompact: false,
        primaryColor: "",
        secondaryColor: "",
      });
      setLocale("zh-CN");
      showToast(t("messages.success.themeReset"));
    }
    if (action === "settings-sidebar-open") {
      state.sidebarCustomizerDraft = sidebarDraftFromPreferences();
      state.settingsDialog = "sidebar-customizer";
    }
    if (action === "settings-sidebar-move-more") {
      moveSidebarItem(target.dataset.routeId, "mainItems", "moreItems");
    }
    if (action === "settings-sidebar-move-main") {
      moveSidebarItem(target.dataset.routeId, "moreItems", "mainItems");
    }
    if (action === "settings-sidebar-move-up") {
      reorderSidebarItem(target.dataset.routeId, target.dataset.list, -1);
    }
    if (action === "settings-sidebar-move-down") {
      reorderSidebarItem(target.dataset.routeId, target.dataset.list, 1);
    }
    if (action === "settings-sidebar-save") {
      const draft = ensureSidebarDraft();
      const preferences = dashboardPreferences();
      setDashboardPreferences({
        ...preferences,
        sidebarMainItems: draft.mainItems,
        sidebarMoreItems: draft.moreItems,
      });
      state.sidebarCustomizerDraft = null;
      state.settingsDialog = "";
      emitSidebarCustomizationChanged();
      showToast(t("messages.success.sidebarSaved"));
    }
    if (action === "settings-sidebar-reset") {
      const defaults = defaultSidebarCustomization();
      const preferences = dashboardPreferences();
      setDashboardPreferences({
        ...preferences,
        sidebarMainItems: [],
        sidebarMoreItems: [],
      });
      state.sidebarCustomizerDraft = { mainItems: [...defaults.mainItems], moreItems: [...defaults.moreItems] };
      emitSidebarCustomizationChanged();
      showToast(t("messages.success.sidebarReset"));
    }
    if (action === "settings-desktop-probe") {
      state.desktopBridge = await probeDesktopBridge();
      showToast(t("messages.success.desktopProbe"));
    }
    if (action === "settings-desktop-restart") {
      const result = await restartDesktopBackend();
      state.desktopBridge = {
        ...(state.desktopBridge || await probeDesktopBridge()),
        backendRestart: result,
        checkedAt: new Date().toISOString(),
      };
      if (!result.ok) throw new Error(result.reason || t("messages.errors.desktopUnavailable"));
      showToast("Desktop backend restart requested");
    }
    if (action === "settings-desktop-update-check") {
      const appUpdate = await checkDesktopAppUpdate();
      state.desktopBridge = {
        ...(state.desktopBridge || await probeDesktopBridge()),
        appUpdate,
        checkedAt: new Date().toISOString(),
      };
      showToast(t("messages.success.desktopProbe"));
    }
    if (action === "settings-desktop-update-install") {
      const appUpdateInstall = await installDesktopAppUpdate();
      state.desktopBridge = {
        ...(state.desktopBridge || await probeDesktopBridge()),
        appUpdateInstall,
        checkedAt: new Date().toISOString(),
      };
      if (!appUpdateInstall.ok) throw new Error(appUpdateInstall.reason || t("messages.errors.desktopUnavailable"));
      showToast("Desktop app update install requested");
    }
    if (action === "settings-open-backup") {
      await loadBackup();
      state.settingsDialog = "backup";
      showToast("Backup dialog 已打开");
    }
    if (action === "settings-open-migration") {
      await loadUpdate();
      state.settingsDialog = "migration";
      showToast("Migration dialog 已打开");
    }
    if (action === "settings-open-changelog") {
      await loadUpdate();
      state.settingsDialog = "changelog";
      showToast("Changelog dialog 已打开");
    }
    if (action === "settings-dialog-close") {
      state.settingsDialog = "";
    }
    if (action === "load-api-keys") {
      await loadApiKeys();
      showToast("API keys 已刷新");
    }
    if (action === "api-key-issue") {
      const checkedScopes = Array.from(document.querySelectorAll("[data-api-key-scope]:checked"))
        .map((scope) => scope.dataset.apiKeyScope)
        .filter(Boolean);
      const result = await api("/api/management/api-keys/issue", {
        method: "POST",
        body: JSON.stringify({
          key_id: $("#api-key-id")?.value.trim() || undefined,
          name: $("#api-key-name").value.trim(),
          secret: $("#api-key-secret")?.value.trim() || undefined,
          scopes: checkedScopes.length ? checkedScopes : $("#api-key-scopes").value.split(",").map((scope) => scope.trim()).filter(Boolean),
          created_by: "dashboard",
          expires_in_days: Number($("#api-key-expires")?.value || 30),
        }),
      });
      state.apiKeys = { api_keys: result.api_keys };
      state.operation = { issued_api_key: result.issued, secret: result.secret };
      showToast("API key 已签发，secret 只显示一次");
    }
    if (action === "api-key-revoke") {
      const result = await api("/api/management/api-keys/revoke", {
        method: "POST",
        body: JSON.stringify({ key_id: target.dataset.key }),
      });
      state.apiKeys = { api_keys: result.api_keys };
      showToast(result.revoked ? "API key 已撤销" : "API key 不存在");
    }
    if (action === "api-key-delete") {
      const result = await api("/api/management/api-keys/delete", {
        method: "POST",
        body: JSON.stringify({ key_id: target.dataset.key }),
      });
      state.apiKeys = { api_keys: result.api_keys };
      showToast(result.deleted ? "API key 已删除" : "API key 不存在");
    }
    if (action === "reload-config") {
      state.config = null;
      state.schema = null;
      await loadConfig();
      showToast("配置已重新读取");
    }
    if (action === "config-search-apply") {
      state.configSearch = $("#config-search")?.value.trim() || "";
      showToast(state.configSearch ? `筛选配置：${state.configSearch}` : "已清除配置筛选");
    }
    if (action === "config-manage-open") {
      showToast("ABConf 管理面板已在当前页展开");
    }
    if (action === "config-test-chat") {
      state.operation = {
        kind: "config_test_chat",
        config_id: currentConfigId(),
        message: "Standalone test chat is owned by the Chat parity task; this config context is ready for that workflow.",
      };
      showToast("当前配置已准备给测试聊天使用");
    }
    if (action === "config-mode-normal" || action === "config-mode-system") {
      const mode = action === "config-mode-system" ? "system" : "normal";
      await requestConfigContextSwitch({ mode });
    }
    if (action === "config-abconf-open") {
      await requestConfigContextSwitch({ confId: $("#config-abconf-select")?.value || "default", mode: "normal" });
    }
    if (action === "config-abconf-select") {
      await requestConfigContextSwitch({ confId: target.dataset.confId || "default", mode: "normal" });
    }
    if (action === "config-abconf-edit-fill") {
      state.configEditId = target.dataset.confId || "";
      state.configEditName = target.dataset.confName || "";
      showToast("ABConf 名称已填入编辑表单");
    }
    if (action === "config-unsaved-close") {
      state.configUnsavedPrompt = null;
      showToast("继续编辑当前配置");
    }
    if (action === "config-unsaved-discard") {
      const pending = state.configUnsavedPrompt?.target || {};
      state.configUnsavedPrompt = null;
      state.configDirty = false;
      await switchConfigContext(pending);
      showToast("已放弃未保存更改并切换");
    }
    if (action === "config-unsaved-save") {
      const pending = state.configUnsavedPrompt?.target || {};
      await applyConfigFromEditor();
      state.configUnsavedPrompt = null;
      await switchConfigContext(pending);
      showToast("已保存并切换配置上下文");
    }
    if (action === "config-abconf-create") {
      const result = await api("/api/management/config/abconfs/create", {
        method: "POST",
        body: JSON.stringify({
          name: $("#config-new-name")?.value.trim() || "Untitled ABConf",
          config: parseJsonDraft(readConfigEditorValue()),
        }),
      });
      state.selectedConfigId = result.conf_id;
      await loadConfig();
      showToast("ABConf 已创建");
    }
    if (action === "config-abconf-rename") {
      const id = $("#config-edit-id")?.value.trim();
      if (!id) throw new Error("请选择要更新的 ABConf");
      await api("/api/management/config/abconfs/update", {
        method: "POST",
        body: JSON.stringify({ id, name: $("#config-edit-name")?.value.trim() }),
      });
      state.configEditId = "";
      state.configEditName = "";
      await loadConfig();
      showToast("ABConf 名称已更新");
    }
    if (action === "config-abconf-delete") {
      await api("/api/management/config/abconfs/delete", {
        method: "POST",
        body: JSON.stringify({ id: target.dataset.confId }),
      });
      if (state.selectedConfigId === target.dataset.confId) {
        state.selectedConfigId = "default";
      }
      await loadConfig();
      showToast("ABConf 已删除");
    }
    if (action === "load-config-routes") {
      await loadConfig();
      showToast("UMOP routes 已刷新");
    }
    if (action === "config-route-upsert") {
      state.configRoutes = await api("/api/management/config/routes/upsert", {
        method: "POST",
        body: JSON.stringify({
          pattern: $("#config-route-pattern").value.trim(),
          config_id: $("#config-route-config-id").value.trim(),
        }),
      });
      state.operation = state.configRoutes;
      showToast("UMOP route 已保存");
    }
    if (action === "config-route-delete") {
      state.configRoutes = await api("/api/management/config/routes/delete", {
        method: "POST",
        body: JSON.stringify({ pattern: target.dataset.pattern }),
      });
      state.operation = state.configRoutes;
      showToast(state.configRoutes.changed ? "UMOP route 已删除" : "UMOP route 不存在");
    }
    if (action === "config-route-resolve") {
      state.operation = await api("/api/management/config/routes/resolve", {
        method: "POST",
        body: JSON.stringify({ umo: $("#config-route-umo").value.trim() }),
      });
      showToast("UMOP route 已解析");
    }
    if (action === "config-route-replace") {
      const parsed = parseJsonDraft($("#config-routes-json")?.value || "{}");
      const routes = Array.isArray(parsed)
        ? parsed
        : Object.entries(parsed).map(([pattern, config_id]) => ({ pattern, config_id }));
      state.configRoutes = await api("/api/management/config/routes/replace", {
        method: "POST",
        body: JSON.stringify({ routes }),
      });
      state.operation = state.configRoutes;
      showToast("UMOP routes 已批量替换");
    }
    if (action === "sync-config-form") {
      syncConfigFormToEditor();
      updateConfigDirtyFromDraft();
      showToast("分类表单已同步到 JSON");
    }
    if (action === "preview-config" || action === "apply-config") {
      const result = await applyConfigFromEditor({ previewOnly: action === "preview-config" });
      showToast(`${action === "preview-config" ? "预览" : "应用"}完成：${result.plan.changed_fields.length} 个字段变化`);
    }
    if (action === "config-editor-fullscreen") {
      state.configEditor = readConfigEditorValue();
      state.configEditorFullscreen = true;
      updateConfigDirtyFromDraft();
    }
    if (action === "config-editor-close") {
      state.configEditorFullscreen = false;
    }
    if (action === "config-editor-apply") {
      state.configEditor = $("#config-editor-fullscreen-text")?.value || state.configEditor;
      state.config = parseJsonDraft(state.configEditor);
      state.configEditorFullscreen = false;
      updateConfigDirtyFromDraft();
      showToast("全屏 JSON 已应用到草稿");
    }
    if (action === "config-scroll-t2i") {
      document.getElementById("t2i-template-editor")?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
    if (action === "t2i-template-load") {
      state.t2iSelectedTemplate = $("#t2i-template-select")?.value || state.t2iSelectedTemplate || "base";
      await loadT2iTemplateContent(state.t2iSelectedTemplate);
      showToast("T2I 模板已读取");
    }
    if (action === "t2i-template-new") {
      state.t2iSelectedTemplate = $("#t2i-template-new-name")?.value.trim() || "custom_template";
      state.t2iTemplateContent = DEFAULT_T2I_TEMPLATE;
      showToast("已创建新的 T2I 模板草稿");
    }
    if (action === "t2i-template-save") {
      await saveT2iTemplate();
      showToast("T2I 模板已保存");
    }
    if (action === "t2i-template-apply") {
      state.t2iSelectedTemplate = $("#t2i-template-select")?.value || state.t2iSelectedTemplate || "base";
      await api("/api/t2i/templates/set_active", {
        method: "POST",
        body: JSON.stringify({ name: state.t2iSelectedTemplate }),
      });
      state.t2iActiveTemplate = { status: "ok", data: { active_template: state.t2iSelectedTemplate } };
      showToast("T2I 模板已应用");
    }
    if (action === "t2i-template-delete") {
      const name = $("#t2i-template-select")?.value || state.t2iSelectedTemplate;
      await api(`/api/t2i/templates/${encodeURIComponent(name)}`, { method: "DELETE" });
      state.t2iSelectedTemplate = "base";
      await loadConfig();
      showToast("T2I 模板已删除");
    }
    if (action === "t2i-template-reset") {
      await api("/api/t2i/templates/reset_default", { method: "POST", body: JSON.stringify({}) });
      state.t2iSelectedTemplate = "base";
      await loadConfig();
      showToast("T2I 默认模板已重置");
    }
    if (action === "t2i-template-preview") {
      state.t2iTemplateContent = $("#t2i-template-content")?.value || state.t2iTemplateContent;
      showToast("T2I 预览已刷新");
    }
    if (action === "t2i-template-fullscreen") {
      state.t2iTemplateContent = $("#t2i-template-content")?.value || state.t2iTemplateContent;
      state.t2iTemplateFullscreen = true;
    }
    if (action === "t2i-template-close") {
      state.t2iTemplateFullscreen = false;
    }
    if (action === "t2i-template-apply-fullscreen") {
      state.t2iTemplateContent = $("#t2i-template-fullscreen-text")?.value || state.t2iTemplateContent;
      state.t2iTemplateFullscreen = false;
      showToast("全屏 T2I 模板已应用到草稿");
    }
    if (action === "chat-new" || action === "chat-new-session") {
      let sessionId = `webchat-${Date.now()}`;
      try {
        const created = await api("/api/chat/new_session");
        sessionId = created.data?.session_id || created.session_id || sessionId;
      } catch {
        // Local draft still lets the WebChat route submit a first message.
      }
      state.chat.conversationId = sessionId;
      state.chat.text = "";
      state.chat.imageUrls = "";
      state.chat.messagePartsJson = "";
      state.chat.replyMessageId = "";
      state.chat.replySelectedText = "";
      state.chat.replyTo = null;
      state.chat.stagedAttachments = [];
      state.messages = [];
      state.activeProjectId = "";
      state.chat.currentSessionProject = null;
      updateChatRouteHash();
      await loadChatControls();
      showToast("新会话已创建");
    }
    if (action === "chat-select-conversation" || action === "chat-select-session") {
      state.chat.conversationId = target.dataset.conversation || target.dataset.session || "demo";
      state.activeProjectId = "";
      state.chat.currentSessionProject = null;
      await loadMessages();
      updateChatRouteHash();
      showToast(`已切换到 ${state.chat.conversationId}`);
    }
    if (action === "chat-sessions-refresh") {
      await loadChatControls();
      showToast("会话列表已刷新");
    }
    if (action === "chat-delete-session") {
      const sessionId = target.dataset.session;
      await api(`/api/chat/delete_session?session_id=${encodeURIComponent(sessionId)}`);
      if (state.chat.conversationId === sessionId) {
        state.chat.conversationId = "demo";
        state.messages = [];
      }
      await loadChatControls();
      showToast("会话已删除");
    }
    if (action === "chat-batch-toggle") {
      state.chat.batchMode = !state.chat.batchMode;
      state.chat.batchSelectedSessionIds = [];
    }
    if (action === "chat-batch-select") {
      const sessionId = target.dataset.session;
      const selected = new Set(state.chat.batchSelectedSessionIds || []);
      if (selected.has(sessionId)) selected.delete(sessionId);
      else selected.add(sessionId);
      state.chat.batchSelectedSessionIds = [...selected];
    }
    if (action === "chat-batch-select-all") {
      const sessions = Array.isArray(state.chatSessions?.data)
        ? state.chatSessions.data
        : Array.isArray(state.chatSessions)
          ? state.chatSessions
          : [];
      state.chat.batchSelectedSessionIds = sessions.map((session) => session.session_id || session.conversation_id).filter(Boolean);
    }
    if (action === "chat-batch-delete") {
      const sessionIds = state.chat.batchSelectedSessionIds || [];
      if (!sessionIds.length) throw new Error("请选择要删除的会话");
      await api("/api/chat/batch_delete_sessions", {
        method: "POST",
        body: JSON.stringify({ session_ids: sessionIds }),
      });
      state.chat.batchSelectedSessionIds = [];
      state.chat.batchMode = false;
      await loadChatControls();
      showToast(`已删除 ${sessionIds.length} 个会话`);
    }
    if (action === "chat-rename-open") {
      state.chat.dialog = "rename";
      state.chat.renameSessionId = target.dataset.session || state.chat.conversationId;
      state.chat.renameTitle = target.dataset.title || "";
    }
    if (action === "chat-rename-save") {
      const sessionId = $("#chat-rename-session-id")?.value.trim() || state.chat.renameSessionId;
      await api("/api/chat/update_session_display_name", {
        method: "POST",
        body: JSON.stringify({
          session_id: sessionId,
          display_name: $("#chat-rename-title")?.value.trim() || sessionId,
        }),
      });
      state.chat.dialog = "";
      await loadChatControls();
      showToast("会话标题已更新");
    }
    if (action === "chat-select-project") {
      state.activeProjectId = target.dataset.project || "";
      state.messages = [];
      if (state.activeProjectId) {
        await loadProjectSessions("user", state.activeProjectId);
      }
      showToast(state.activeProjectId ? `已打开项目 ${state.activeProjectId}` : "已退出项目视图");
    }
    if (action === "chat-clear-project") {
      state.activeProjectId = "";
      state.projectSessions = null;
      state.chat.currentSessionProject = null;
      showToast("已退出项目视图");
    }
    if (action === "chat-provider-select" || action === "chat-settings-apply") {
      syncChatFormState();
      localStorage.setItem("chat_selectedProviderId", state.chat.selectedProviderId);
      localStorage.setItem("chat_selectedModelName", state.chat.selectedModelName || "");
      localStorage.setItem("chat_transportMode", state.chat.transportMode);
      localStorage.setItem("chat_sendShortcut", state.chat.sendShortcut);
      localStorage.setItem("chat_enableStreaming", JSON.stringify(state.chat.enableStreaming));
      showToast(state.chat.selectedProviderId ? `模型已选择：${state.chat.selectedProviderId}` : "已使用默认模型");
    }
    if (action === "chat-config-select") {
      state.chat.configId = $("#chat-config-id")?.value || $("#chat-config-select")?.value || "default";
      state.chat.selectedConfigId = state.chat.configId;
      localStorage.setItem("chat_configId", state.chat.configId);
      showToast(`配置上下文：${state.chat.configId}`);
    }
    if (action === "chat-config-apply") {
      syncChatFormState();
      const pattern = $("#chat-config-route-pattern")?.value.trim() || `webchat:FriendMessage:webchat!dashboard!${state.chat.conversationId}`;
      state.operation = await api("/api/config/umo_abconf_route/update", {
        method: "POST",
        body: JSON.stringify({ umo: pattern, conf_id: state.chat.selectedConfigId || state.chat.configId || "default" }),
      });
      showToast("会话配置路由已保存");
    }
    if (action === "chat-toggle-streaming") {
      state.chat.enableStreaming = !state.chat.enableStreaming;
      localStorage.setItem("chat_enableStreaming", JSON.stringify(state.chat.enableStreaming));
      showToast(state.chat.enableStreaming ? "Streaming 已开启" : "Streaming 已关闭");
    }
    if (action === "chat-transport-mode") {
      state.chat.transportMode = target.dataset.mode || $("#chat-transport-mode")?.value || "sse";
      localStorage.setItem("chat_transportMode", state.chat.transportMode);
      showToast(`传输模式：${state.chat.transportMode}`);
    }
    if (action === "chat-send-shortcut") {
      state.chat.sendShortcut = target.dataset.shortcut || $("#chat-send-shortcut")?.value || "shift_enter";
      localStorage.setItem("chat_sendShortcut", state.chat.sendShortcut);
      showToast(state.chat.sendShortcut === "enter" ? "Enter 发送已启用" : "Shift+Enter 发送已启用");
    }
    if (action === "chat-reply-message" || action === "chat-reply") {
      const message = state.messages[Number(target.dataset.messageIndex || -1)];
      state.chat.replyMessageId = target.dataset.messageId || message?.id || String(Number(target.dataset.messageIndex || 0) + 1);
      state.chat.replySelectedText = target.dataset.selectedText || textPreviewForMessage(message) || "quoted message";
      state.chat.replyTo = {
        messageId: state.chat.replyMessageId,
        selectedText: state.chat.replySelectedText,
      };
      showToast("引用已加入输入框");
    }
    if (action === "chat-clear-reply" || action === "chat-reply-clear") {
      state.chat.replyMessageId = "";
      state.chat.replySelectedText = "";
      state.chat.replyTo = null;
      showToast("引用已清除");
    }
    if (action === "chat-open-refs" || action === "chat-refs-open") {
      const message = state.messages[Number(target.dataset.messageIndex || -1)];
      const refs = message?.content?.refs || message?.refs || null;
      state.chat.refsSidebarRefs = refs;
      state.chat.selectedRefs = Array.isArray(refs) ? refs : [];
      state.chat.refsSidebarOpen = Boolean(refs);
    }
    if (action === "chat-part-refs-open") {
      const refs = JSON.parse(target.dataset.refs || "[]");
      state.chat.selectedRefs = Array.isArray(refs) ? refs : [];
      state.chat.refsSidebarOpen = true;
    }
    if (action === "chat-close-refs" || action === "chat-refs-close") {
      state.chat.refsSidebarOpen = false;
      state.chat.refsSidebarRefs = null;
      state.chat.selectedRefs = [];
    }
    if (action === "chat-live-open") {
      state.chat.liveModeOpen = true;
      state.chat.liveModeStatus = state.chat.liveModeStatus || "idle";
      state.chat.live = { ...(state.chat.live || {}), status: state.chat.liveModeStatus || "idle", statusText: "Astr Live" };
    }
    if (action === "chat-live-close") {
      state.chat.liveModeOpen = false;
      state.chat.liveModeStatus = "idle";
      state.chat.live = { ...(state.chat.live || {}), status: "idle", statusText: "Astr Live" };
    }
    if (action === "chat-live-connect") {
      state.chat.liveModeOpen = true;
      state.chat.liveModeStatus = "ready";
      state.chat.liveModeWsUrl = buildWsUrl("/api/live_chat/ws");
      state.chat.live = {
        ...(state.chat.live || {}),
        status: "ready",
        statusText: "WebSocket ready",
        messages: [...(state.chat.live?.messages || []), { type: "system", text: "Live WebSocket ready" }],
      };
      state.operation = {
        kind: "live_mode",
        websocket: state.chat.liveModeWsUrl,
        status: "client_ready",
      };
      showToast("Live Mode 控制面板已准备");
    }
    if (action === "chat-live-disconnect") {
      state.chat.liveModeStatus = "idle";
      state.chat.live = { ...(state.chat.live || {}), status: "idle", statusText: "Disconnected" };
      showToast("Live Mode 已断开");
    }
    if (action === "chat-live-interrupt" || action === "chat-live-speaking-start" || action === "chat-live-speaking-end") {
      const liveType = action === "chat-live-speaking-start" ? "start_speaking" : action === "chat-live-speaking-end" ? "end_speaking" : "interrupt";
      state.operation = { kind: "live_mode_event", type: liveType, conversation_id: state.chat.conversationId };
      state.chat.live = {
        ...(state.chat.live || {}),
        status: liveType,
        statusText: liveType.replaceAll("_", " "),
        messages: [...(state.chat.live?.messages || []), { type: "user", text: liveType }],
      };
      showToast("Live Mode 事件已记录");
    }
    if (action === "chat-stage-url") {
      const url = $("#chat-attachment-url")?.value.trim();
      if (!url) throw new Error("请填写附件 URL");
      state.chat.stagedAttachments = [
        ...(state.chat.stagedAttachments || []),
        {
          type: /\.(png|jpe?g|gif|webp|svg)$/i.test(url) ? "image" : "file",
          url,
          name: url.split("/").pop() || url,
        },
      ];
      if ($("#chat-attachment-url")) $("#chat-attachment-url").value = "";
      showToast("附件已加入预览");
    }
    if (action === "chat-upload-file") {
      const input = $("#chat-file-upload");
      const files = Array.from(input?.files || []);
      if (!files.length) throw new Error("请选择文件");
      const uploaded = [];
      for (const file of files) {
        uploaded.push(await uploadChatFile(file));
      }
      state.chat.stagedAttachments = [...(state.chat.stagedAttachments || []), ...uploaded];
      if (input) input.value = "";
      showToast(`已上传 ${uploaded.length} 个附件`);
    }
    if (action === "chat-attachment-remove") {
      const index = target.dataset.index || "";
      if (index.startsWith("legacy")) {
        state.chat.imageUrls = "";
      } else {
        const next = [...(state.chat.stagedAttachments || [])];
        next.splice(Number(index), 1);
        state.chat.stagedAttachments = next;
      }
    }
    if (action === "chat-image-preview") {
      state.chat.dialog = "image-preview";
      state.chat.previewImageUrl = target.dataset.url || target.getAttribute("src") || "";
    }
    if (action === "chat-project-dialog-open") {
      state.chat.projectDialogMode = target.dataset.mode || "create";
      state.chat.projectDialogTargetId = target.dataset.project || "";
      state.chat.dialog = "project";
    }
    if (action === "chat-projects-toggle") {
      state.chat.projectsExpanded = state.chat.projectsExpanded === false;
      localStorage.setItem("projectsExpanded", JSON.stringify(state.chat.projectsExpanded));
    }
    if (action === "chat-dialog-close" || action === "chatbox-fullscreen-toggle") {
      state.chat.dialog = "";
    }
    if (action === "chat-stop") {
      syncChatFormState();
      try {
        await api("/api/chat/stop", {
          method: "POST",
          body: JSON.stringify({ session_id: state.chat.conversationId }),
        });
      } catch {
        // OpenAPI stop remains available for ChatBox even if legacy chat stop is absent.
      }
      showToast("Stop request 已发送");
    }
    if (action === "chat-elicitation-respond") {
      const elicitationId = target.dataset.elicitation || $("#elicitation-id")?.value.trim();
      const result = target.dataset.result || "accept";
      state.realtime = {
        ...(state.realtime || {}),
        lastElicitation: await openApi("/api/openapi/elicitation/respond", {
          method: "POST",
          body: JSON.stringify({
            elicitation_id: elicitationId,
            result: { action: result, content: result === "accept" ? { confirmed: true } : {} },
          }),
        }),
      };
      await loadOpenApiRealtime();
      showToast("Elicitation response 已记录");
    }
    if (action === "send-chat") {
      syncChatFormState();
      const projectId = state.activeProjectId || "";
      const promptToSend = state.chat.text;
      const project = (state.projects?.projects || state.projects?.data || [])
        .find((item) => item.project_id === projectId);
      if (!state.chat.conversationId) {
        let sessionId = `webchat-${Date.now()}`;
        try {
          const created = await api("/api/chat/new_session");
          sessionId = created.data?.session_id || created.session_id || sessionId;
        } catch {
          // A local id still lets the message flow run against mocked Dashboard APIs.
        }
        state.chat.conversationId = sessionId;
      }
      const { imageUrls, messageParts } = chatPayloadFromForm();
      await api(`/api/webchat/${encodeURIComponent(state.chat.conversationId)}`, {
        method: "POST",
        body: JSON.stringify({
          sender_id: state.chat.senderId,
          text: promptToSend,
          image_urls: imageUrls,
          message_parts: Array.isArray(messageParts) ? messageParts : [messageParts],
          selected_provider: state.chat.selectedProviderId || null,
          selected_model: state.chat.selectedModelName || state.chat.selectedModel || null,
          enable_streaming: state.chat.enableStreaming,
        }),
      });
      state.chat.text = "";
      state.chat.imageUrls = "";
      state.chat.messagePartsJson = "";
      state.chat.replyMessageId = "";
      state.chat.replySelectedText = "";
      state.chat.replyTo = null;
      state.chat.stagedAttachments = [];
      if (projectId) {
        const now = new Date().toISOString();
        await api("/api/management/chat-projects/sessions/upsert", {
          method: "POST",
          body: JSON.stringify({
            session_id: state.chat.conversationId,
            platform_id: "webchat",
            creator: "user",
            display_name: textPreviewForMessage({ text: promptToSend }) || state.chat.conversationId,
            is_group: false,
            now,
          }),
        });
        await api("/api/management/chat-projects/add-session", {
          method: "POST",
          body: JSON.stringify({
            actor: "user",
            project_id: projectId,
            session_id: state.chat.conversationId,
          }),
        });
        state.chat.currentSessionProject = project || null;
        state.activeProjectId = "";
      }
      await new Promise((resolve) => setTimeout(resolve, 120));
      await loadMessages();
      await loadChatControls();
      await loadLogs();
      showToast("消息已提交到 runtime");
    }
    if (action === "openapi-stream-chat") {
      saveOpenApiSecretFromForm();
      syncChatFormState();
      const { imageUrls, messageParts } = chatPayloadFromForm();
      const requestId = $("#openapi-request-id")?.value.trim() || `${state.chat.conversationId}-${Date.now()}`;
      const result = await openApi("/api/openapi/chat", {
        method: "POST",
        body: JSON.stringify({
          conversation_id: state.chat.conversationId,
          sender_id: state.chat.senderId,
          text: state.chat.text,
          request_id: requestId,
          stream: true,
          message_parts: [
            ...imageUrls.map((url) => ({ type: "image", url })),
            ...(Array.isArray(messageParts) ? messageParts : [messageParts]),
          ],
          selected_provider: state.chat.selectedProviderId || null,
          selected_model: state.chat.selectedModelName || state.chat.selectedModel || null,
        }),
      });
      state.realtime = { ...(state.realtime || {}), last: result };
      await loadOpenApiRealtime();
      await new Promise((resolve) => setTimeout(resolve, 120));
      await loadMessages();
      showToast("OpenAPI stream request 已提交");
    }
    if (action === "openapi-realtime-refresh") {
      saveOpenApiSecretFromForm();
      await loadOpenApiRealtime();
      showToast("Realtime 状态已刷新");
    }
    if (action === "openapi-subscription-status") {
      saveOpenApiSecretFromForm();
      const requestId = $("#openapi-request-id").value.trim();
      state.realtime = {
        ...(state.realtime || {}),
        selected: await openApi(`/api/openapi/chat/subscriptions/${encodeURIComponent(requestId)}`),
      };
      showToast("Subscription 状态已读取");
    }
    if (action === "openapi-stop-chat") {
      saveOpenApiSecretFromForm();
      state.chat.conversationId = $("#conversation-id").value.trim() || "demo";
      const requestId = $("#openapi-request-id")?.value.trim();
      const result = await openApi("/api/openapi/chat/stop", {
        method: "POST",
        body: JSON.stringify({
          conversation_id: state.chat.conversationId,
          request_id: requestId || null,
        }),
      });
      state.realtime = { ...(state.realtime || {}), lastStop: result };
      await loadOpenApiRealtime();
      showToast("Stop request 已记录");
    }
    if (action === "openapi-elicitation-create") {
      saveOpenApiSecretFromForm();
      const result = await openApi("/api/openapi/elicitation", {
        method: "POST",
        body: JSON.stringify({
          elicitation_id: $("#elicitation-id").value.trim() || null,
          conversation_id: $("#conversation-id").value.trim() || null,
          request_id: $("#openapi-request-id")?.value.trim() || null,
          request: {
            kind: "form",
            message: $("#elicitation-message").value.trim() || "Approve action?",
            requested_schema: {
              properties: {
                confirmed: { type: "boolean" },
              },
              required: ["confirmed"],
            },
          },
        }),
      });
      state.realtime = { ...(state.realtime || {}), lastElicitation: result };
      await loadOpenApiRealtime();
      showToast("Elicitation 已创建");
    }
    if (action === "openapi-elicitation-respond") {
      saveOpenApiSecretFromForm();
      const result = await openApi("/api/openapi/elicitation/respond", {
        method: "POST",
        body: JSON.stringify({
          elicitation_id: $("#elicitation-id").value.trim(),
          result: {
            action: $("#elicitation-confirmed")?.checked ? "accept" : "decline",
            content: $("#elicitation-confirmed")?.checked ? { confirmed: true } : {},
          },
        }),
      });
      state.realtime = { ...(state.realtime || {}), lastElicitation: result };
      await loadOpenApiRealtime();
      showToast("Elicitation response 已记录");
    }
    if (action === "load-chat") {
      syncChatFormState();
      await Promise.all([loadMessages(), loadChatControls()]);
      showToast("历史已刷新");
    }
    if (action === "load-conversations") {
      await loadConversations("", conversationFiltersFromForm());
      showToast("Conversation 列表已刷新");
    }
    if (action === "conversation-filter-apply") {
      await loadConversations("", { ...conversationFiltersFromForm(), page: 1 });
      state.conversationSelectedKeys = [];
      showToast("Conversation 筛选已应用");
    }
    if (action === "conversation-page-prev" || action === "conversation-page-next") {
      const delta = action === "conversation-page-prev" ? -1 : 1;
      const page = Math.max(1, Number(state.conversationFilters?.page || 1) + delta);
      await loadConversations("", { ...conversationFiltersFromForm(), page });
      showToast(`Conversation page ${page}`);
    }
    if (action === "conversation-select") {
      const key = target.dataset.key;
      const selected = new Set(state.conversationSelectedKeys || []);
      if (selected.has(key)) selected.delete(key);
      else selected.add(key);
      state.conversationSelectedKeys = [...selected];
    }
    if (action === "conversation-select-all") {
      const current = conversationListFromState().map(conversationKeyForRecord);
      const selected = new Set(state.conversationSelectedKeys || []);
      const allSelected = current.length > 0 && current.every((key) => selected.has(key));
      for (const key of current) {
        if (allSelected) selected.delete(key);
        else selected.add(key);
      }
      state.conversationSelectedKeys = [...selected];
    }
    if (action === "conversation-dialog-close") {
      state.conversationDialog = "";
    }
    if (action === "conversation-view") {
      const detail = await api("/api/conversation/detail", {
        method: "POST",
        body: JSON.stringify({ user_id: target.dataset.user, cid: target.dataset.cid }),
      });
      state.conversationDetail = normalizeConversationActionRecord(detail.data || detail);
      state.conversationHistoryDraft = state.conversationDetail.history || "[]";
      state.conversationHistoryMode = "preview";
      state.conversationDialog = "history";
      showToast("Conversation 详情已加载");
    }
    if (action === "conversation-history-edit") {
      if (state.conversationDetail) {
        state.conversationHistoryDraft = $("#conversation-history-editor")?.value || state.conversationHistoryDraft || state.conversationDetail.history || "[]";
      }
      state.conversationHistoryMode = "edit";
    }
    if (action === "conversation-history-preview") {
      state.conversationHistoryDraft = $("#conversation-history-editor")?.value || state.conversationHistoryDraft || state.conversationDetail?.history || "[]";
      JSON.parse(state.conversationHistoryDraft || "[]");
      state.conversationHistoryMode = "preview";
    }
    if (action === "conversation-history-save") {
      if (!state.conversationDetail) throw new Error("请先加载 Conversation 详情");
      const draft = $("#conversation-history-editor")?.value || state.conversationHistoryDraft || state.conversationDetail.history || "[]";
      const history = JSON.parse(draft);
      state.operation = await api("/api/conversation/update_history", {
        method: "POST",
        body: JSON.stringify({
          user_id: state.conversationDetail.user_id,
          cid: state.conversationDetail.cid,
          history,
        }),
      });
      state.conversationDetail.history = JSON.stringify(history);
      state.conversationHistoryDraft = state.conversationDetail.history;
      state.conversationHistoryMode = "preview";
      await loadConversations("", state.conversationFilters);
      showToast("Conversation history 已保存");
    }
    if (action === "conversation-edit-open") {
      const record = findConversationRecord(target.dataset.user, target.dataset.cid) || {
        user_id: target.dataset.user,
        cid: target.dataset.cid,
      };
      state.conversationEditTarget = normalizeConversationActionRecord(record);
      state.conversationDialog = "edit";
    }
    if (action === "conversation-edit-save") {
      const userId = $("#conversation-edit-user-id")?.value.trim();
      const cid = $("#conversation-edit-cid")?.value.trim();
      state.operation = await api("/api/conversation/update", {
        method: "POST",
        body: JSON.stringify({
          user_id: userId,
          cid,
          title: $("#conversation-edit-title")?.value.trim() || null,
          persona_id: $("#conversation-edit-persona")?.value.trim() || null,
        }),
      });
      state.conversationDialog = "";
      await loadConversations("", state.conversationFilters);
      showToast("Conversation 信息已保存");
    }
    if (action === "conversation-delete-open") {
      state.conversationDeleteTarget = normalizeConversationActionRecord(findConversationRecord(target.dataset.user, target.dataset.cid) || {
        user_id: target.dataset.user,
        cid: target.dataset.cid,
      });
      state.conversationDialog = "delete";
    }
    if (action === "conversation-delete-confirm") {
      const targetRecord = state.conversationDeleteTarget;
      if (!targetRecord?.cid) throw new Error("请选择要删除的 Conversation");
      state.operation = await api("/api/conversation/delete", {
        method: "POST",
        body: JSON.stringify({ user_id: targetRecord.user_id, cid: targetRecord.cid }),
      });
      state.conversationSelectedKeys = (state.conversationSelectedKeys || []).filter((key) => key !== conversationKeyForRecord(targetRecord));
      state.conversationDialog = "";
      if (state.conversationDetail?.cid === targetRecord.cid) state.conversationDetail = null;
      await loadConversations("", state.conversationFilters);
      showToast("Conversation 已删除");
    }
    if (action === "conversation-batch-delete-open") {
      if (!(state.conversationSelectedKeys || []).length) throw new Error("请选择要删除的 Conversation");
      state.conversationDialog = "batch-delete";
    }
    if (action === "conversation-batch-delete-confirm") {
      const conversations = selectedConversationRecords();
      state.operation = await api("/api/conversation/delete", {
        method: "POST",
        body: JSON.stringify({
          conversations: conversations.map((conversation) => ({ user_id: conversation.user_id, cid: conversation.cid })),
        }),
      });
      state.conversationSelectedKeys = [];
      state.conversationDialog = "";
      await loadConversations("", state.conversationFilters);
      showToast(`批量删除 ${state.operation.data?.deleted_count ?? state.operation.deleted_count ?? conversations.length} 个 Conversation`);
    }
    if (action === "conversation-export-selected") {
      const conversations = selectedConversationRecords();
      if (!conversations.length) throw new Error("请选择要导出的 Conversation");
      const response = await fetch(`${apiBase()}${"/api/conversation/export"}`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          ...(localStorage.getItem("astrbot.managementToken") ? { Authorization: `Bearer ${localStorage.getItem("astrbot.managementToken")}` } : {}),
        },
        body: JSON.stringify({
          conversations: conversations.map((conversation) => ({ user_id: conversation.user_id, cid: conversation.cid })),
        }),
      });
      if (!response.ok) throw new Error(await response.text());
      const blob = await response.blob();
      if (globalThis.URL?.createObjectURL && globalThis.document?.createElement) {
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `astrbot_conversations_export_${Date.now()}.jsonl`;
        document.body.appendChild(link);
        link.click();
        link.remove();
        URL.revokeObjectURL(url);
      }
      state.operation = { kind: "conversation_export", count: conversations.length };
      showToast(`已导出 ${conversations.length} 个 Conversation`);
    }
    if (action === "conversation-upsert") {
      state.chat.conversationId = $("#conversation-id").value.trim() || "demo";
      const result = await api("/api/management/conversations/upsert", {
        method: "POST",
        body: JSON.stringify({
          platform_id: $("#conversation-platform-id").value.trim() || "webchat",
          conversation_id: state.chat.conversationId,
          title: $("#conversation-title").value.trim() || null,
          persona_id: $("#conversation-persona").value.trim() || null,
          set_current: $("#conversation-current").checked,
        }),
      });
      state.operation = result;
      await loadConversations($("#conversation-platform-id").value.trim() || "webchat");
      showToast("Conversation 已保存");
    }
    if (action === "conversation-rename") {
      state.chat.conversationId = target.dataset.conversation;
      state.operation = await api("/api/management/conversations/rename", {
        method: "POST",
        body: JSON.stringify({
          platform_id: target.dataset.platform,
          conversation_id: target.dataset.conversation,
          title: $("#conversation-title").value.trim() || target.dataset.conversation,
        }),
      });
      await loadConversations(target.dataset.platform);
      showToast("Conversation 已重命名");
    }
    if (action === "conversation-switch") {
      state.chat.conversationId = target.dataset.conversation;
      state.operation = await api("/api/management/conversations/current", {
        method: "POST",
        body: JSON.stringify({
          platform_id: target.dataset.platform,
          conversation_id: target.dataset.conversation,
        }),
      });
      await loadConversations(target.dataset.platform);
      await loadMessages();
      showToast("当前 Conversation 已切换");
    }
    if (action === "conversation-delete") {
      state.operation = await api("/api/management/conversations/delete", {
        method: "POST",
        body: JSON.stringify({
          platform_id: target.dataset.platform,
          conversation_id: target.dataset.conversation,
        }),
      });
      await loadConversations(target.dataset.platform);
      showToast(state.operation.deleted ? "Conversation 已删除" : "Conversation 不存在");
    }
    if (action === "conversation-batch-delete") {
      const platformId = $("#conversation-platform-id").value.trim() || "webchat";
      const conversationIds = $("#conversation-batch-ids").value
        .split(/\r?\n|,/)
        .map((value) => value.trim())
        .filter(Boolean);
      state.operation = await api("/api/management/conversations/batch-delete", {
        method: "POST",
        body: JSON.stringify({ platform_id: platformId, conversation_ids: conversationIds }),
      });
      await loadConversations(platformId);
      showToast(`批量删除 ${state.operation.deleted_count} 个 Conversation`);
    }
}

async function loadOpenApiRealtime() {
  const [subscriptions, elicitations] = await Promise.all([
    openApi("/api/openapi/chat/subscriptions"),
    openApi("/api/openapi/elicitation"),
  ]);
  state.realtime = {
    ...(state.realtime || {}),
    subscriptions: subscriptions.subscriptions || [],
    elicitations: elicitations.elicitations || [],
    unavailable: null,
  };
}

function syncChatFormState() {
  const conversationId = $("#conversation-id")?.value.trim();
  state.chat.conversationId = conversationId || state.chat.conversationId || (state.activeProjectId ? "" : "demo");
  state.chat.senderId = $("#sender-id")?.value.trim() || state.chat.senderId || "user";
  state.chat.text = $("#chat-text")?.value || state.chat.text || "";
  state.chat.imageUrls = $("#chat-image-urls")?.value || state.chat.imageUrls || "";
  state.chat.messagePartsJson = $("#chat-message-parts")?.value || state.chat.messagePartsJson || "";
  state.chat.selectedProviderId = $("#chat-provider-id")?.value || $("#chat-provider-select")?.value || state.chat.selectedProviderId || "";
  state.chat.selectedModelName = $("#chat-model-select")?.value || state.chat.selectedModelName || state.chat.selectedModel || "";
  state.chat.configId = $("#chat-config-id")?.value || $("#chat-config-select")?.value || state.chat.configId || "default";
  state.chat.selectedConfigId = state.chat.configId;
  state.chat.enableStreaming = $("#chat-enable-streaming") ? $("#chat-enable-streaming").checked : state.chat.enableStreaming !== false;
  state.chat.transportMode = $("#chat-transport-mode")?.value || state.chat.transportMode || "sse";
  state.chat.sendShortcut = $("#chat-send-shortcut")?.value || state.chat.sendShortcut || "shift_enter";
  const provider = [
    ...(Array.isArray(state.chatProviderList) ? state.chatProviderList : []),
    ...(state.providerCatalog?.providers || []),
  ].find((item) => item.id === state.chat.selectedProviderId);
  state.chat.selectedModel = provider?.model || state.chat.selectedModel || "";
}

function chatPayloadFromForm() {
  const imageUrls = (state.chat.imageUrls || "")
    .split(/\r?\n|,/)
    .map((value) => value.trim())
    .filter(Boolean);
  const parsedParts = state.chat.messagePartsJson.trim()
    ? JSON.parse(state.chat.messagePartsJson)
    : [];
  const messageParts = Array.isArray(parsedParts) ? [...parsedParts] : [parsedParts];
  for (const attachment of state.chat.stagedAttachments || []) {
    const url = attachment.url || (attachment.attachment_id ? `/api/chat/get_attachment?attachment_id=${encodeURIComponent(attachment.attachment_id)}` : "");
    if (!url) continue;
    if ((attachment.type || "").includes("image")) {
      messageParts.push({ type: "image", url });
    } else if ((attachment.type || "").includes("record") || (attachment.type || "").includes("audio")) {
      messageParts.push({ type: "record", url });
    } else if ((attachment.type || "").includes("video")) {
      messageParts.push({ type: "video", url });
    } else {
      messageParts.push({ type: "file", name: attachment.original_name || attachment.name || attachment.filename || "file", url });
    }
  }
  const reply = state.chat.replyTo || (state.chat.replyMessageId ? {
    messageId: state.chat.replyMessageId,
    selectedText: state.chat.replySelectedText || "",
  } : null);
  if (reply?.messageId) {
    messageParts.unshift({
      type: "reply",
      message_id: reply.messageId,
      selected_text: reply.selectedText || "",
    });
  }
  return { imageUrls, messageParts };
}

function conversationFiltersFromForm() {
  const existing = state.conversationFilters || {};
  return {
    platforms: ($("#conversation-filter-platforms")?.value || "")
      .split(/\r?\n|,/)
      .map((value) => value.trim())
      .filter(Boolean),
    messageTypes: $("#conversation-filter-message-type")?.value ? [$("#conversation-filter-message-type").value] : [],
    search: $("#conversation-filter-search")?.value.trim() || "",
    page: Number(existing.page || 1),
    pageSize: Number($("#conversation-page-size")?.value || existing.pageSize || 20),
  };
}

function conversationListFromState() {
  const payload = state.conversations?.data || state.conversations || {};
  return Array.isArray(payload.conversations)
    ? payload.conversations.map(normalizeConversationActionRecord)
    : [];
}

function normalizeConversationActionRecord(conversation = {}) {
  const platformId = conversation.platform_id || conversation.platform || "webchat";
  const cid = conversation.cid || conversation.conversation_id || conversation.session_id || conversation.id || "";
  return {
    ...conversation,
    platform_id: platformId,
    cid,
    conversation_id: cid,
    user_id: conversation.user_id || `${platformId}:FriendMessage:${cid}`,
    history: typeof conversation.history === "string" ? conversation.history : JSON.stringify(conversation.history || []),
  };
}

function conversationKeyForRecord(conversation = {}) {
  const normalized = normalizeConversationActionRecord(conversation);
  return `${normalized.user_id}\u001f${normalized.cid}`;
}

function findConversationRecord(userId, cid) {
  return conversationListFromState().find((conversation) => conversation.user_id === userId && conversation.cid === cid) || null;
}

function selectedConversationRecords() {
  const selected = new Set(state.conversationSelectedKeys || []);
  return conversationListFromState().filter((conversation) => selected.has(conversationKeyForRecord(conversation)));
}

function textPreviewForMessage(message) {
  if (!message) return "";
  if (message.text) return String(message.text).slice(0, 120);
  const parts = message.content?.message || message.message_parts || [];
  if (typeof parts === "string") return parts.slice(0, 120);
  if (!Array.isArray(parts)) return "";
  return parts
    .filter((part) => part?.type === "plain" || part?.type === "text")
    .map((part) => part.text || "")
    .join("")
    .slice(0, 120);
}

function buildWsUrl(path) {
  const base = apiBase();
  const source = base || globalThis.window?.location?.origin || "";
  const url = new URL(path, source || "http://127.0.0.1");
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  const token = localStorage.getItem("astrbot.managementToken") || localStorage.getItem("token") || "";
  if (token) {
    url.searchParams.set("token", token);
  }
  return url.toString();
}

async function uploadChatFile(file) {
  const formData = new FormData();
  formData.append("file", file);
  const token = localStorage.getItem("astrbot.managementToken") || "";
  const response = await fetch(`${apiBase()}${"/api/chat/post_file"}`, {
    method: "POST",
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    body: formData,
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(payload?.error || payload?.message || `${response.status} ${response.statusText}`);
  }
  const data = payload.data || payload;
  return {
    attachment_id: data.attachment_id,
    filename: data.filename || file.name,
    original_name: data.original_name || file.name,
    type: data.type || (file.type.startsWith("image/") ? "image" : "file"),
    url: data.url || URL.createObjectURL(file),
  };
}

function updateChatRouteHash() {
  const route = state.route === "chatbox" ? "chatbox" : "chat";
  if (globalThis.window?.location && state.chat.conversationId) {
    globalThis.window.location.hash = `#/${route}/${encodeURIComponent(state.chat.conversationId)}`;
  }
}

async function requestConfigContextSwitch(target) {
  const normalized = {
    mode: target.mode || state.configMode || "normal",
    confId: target.confId || state.selectedConfigId || "default",
  };
  if (
    normalized.mode === (state.configMode || "normal")
    && normalized.confId === (state.selectedConfigId || "default")
  ) {
    return;
  }
  if (updateConfigDirtyFromDraft()) {
    state.configUnsavedPrompt = { open: true, target: normalized };
    showToast("当前配置有未保存更改", "warn");
    return;
  }
  await switchConfigContext(normalized);
}

async function switchConfigContext({ mode = state.configMode || "normal", confId = state.selectedConfigId || "default" } = {}) {
  state.configMode = mode;
  state.routeFragment = mode;
  if (mode === "normal") {
    state.selectedConfigId = confId || "default";
  }
  if (globalThis.window?.location) {
    globalThis.window.location.hash = `#/config#${mode}`;
  }
  await loadConfig();
}

async function applyConfigFromEditor({ previewOnly = false } = {}) {
  if ($("#config-editor")?.value === state.configEditor) {
    syncConfigFormToEditor();
  }
  const configText = readConfigEditorValue();
  const config = parseJsonDraft(configText);
  const path = previewOnly ? "/api/management/config/preview" : "/api/management/config/apply";
  const result = await api(path, {
    method: "POST",
    body: JSON.stringify({ config, conf_id: currentConfigId() }),
  });
  state.config = result.config;
  state.configEditor = JSON.stringify(result.config, null, 2);
  state.operation = result.plan;
  if (!previewOnly) {
    state.configLastSavedSnapshot = state.configEditor;
    state.configDirty = false;
  } else {
    state.configDirty = state.configEditor !== state.configLastSavedSnapshot;
  }
  return result;
}

function currentConfigId() {
  return (state.configMode || "normal") === "system" ? "default" : state.selectedConfigId || "default";
}

function readConfigEditorValue() {
  return $("#config-editor")?.value || state.configEditor || "{}";
}

function updateConfigDirtyFromDraft() {
  state.configEditor = readConfigEditorValue();
  state.configDirty = state.configEditor !== (state.configLastSavedSnapshot || "");
  return state.configDirty;
}

function parseJsonDraft(text) {
  if (!text?.trim()) {
    return {};
  }
  return JSON.parse(text);
}

async function loadT2iTemplateContent(name) {
  const result = await api(`/api/t2i/templates/${encodeURIComponent(name)}`);
  state.t2iTemplateContent = result.data?.content || "";
}

async function saveT2iTemplate() {
  const name = ($("#t2i-template-new-name")?.value.trim() && state.t2iSelectedTemplate === $("#t2i-template-new-name")?.value.trim())
    ? $("#t2i-template-new-name").value.trim()
    : ($("#t2i-template-select")?.value || state.t2iSelectedTemplate || "base");
  const content = $("#t2i-template-content")?.value || state.t2iTemplateContent || DEFAULT_T2I_TEMPLATE;
  const existing = (state.t2iTemplates?.data || []).some((template) => template.name === name);
  const result = existing
    ? await api(`/api/t2i/templates/${encodeURIComponent(name)}`, {
      method: "PUT",
      body: JSON.stringify({ content }),
    })
    : await api("/api/t2i/templates/create", {
      method: "POST",
      body: JSON.stringify({ name, content }),
    });
  state.t2iSelectedTemplate = name;
  state.t2iTemplateContent = content;
  state.operation = result;
  const list = await api("/api/t2i/templates");
  state.t2iTemplates = list;
}

function saveOpenApiSecretFromForm() {
  state.openApiSecretDraft = $("#openapi-secret")?.value || "";
  setOpenApiSecret(state.openApiSecretDraft);
}

function saveDashboardPreferencesFromForm() {
  if ($("#dashboard-locale")?.value) {
    setLocale($("#dashboard-locale").value);
  }
  const previous = dashboardPreferences();
  setDashboardPreferences({
    theme: $("#dashboard-theme")?.value || "light",
    sidebarCompact: $("#sidebar-compact")?.checked || false,
    primaryColor: $("#theme-primary-color")?.value || "",
    secondaryColor: $("#theme-secondary-color")?.value || "",
    githubProxyEnabled: $("#github-proxy-enabled")?.checked || false,
    githubProxyUrl: $("#github-proxy-url")?.value || "",
    apiBasePresets: previous.apiBasePresets || [],
    sidebarMainItems: previous.sidebarMainItems || [],
    sidebarMoreItems: previous.sidebarMoreItems || [],
  });
  applyDashboardPreferences();
}

function sidebarDraftFromPreferences() {
  const resolved = resolveSidebarPreferences(dashboardPreferences());
  return {
    mainItems: [...resolved.mainItems],
    moreItems: [...resolved.moreItems],
  };
}

function ensureSidebarDraft() {
  if (!state.sidebarCustomizerDraft) {
    state.sidebarCustomizerDraft = sidebarDraftFromPreferences();
  }
  return state.sidebarCustomizerDraft;
}

function moveSidebarItem(routeId, fromKey, toKey) {
  const draft = ensureSidebarDraft();
  const id = String(routeId || "");
  const index = draft[fromKey].indexOf(id);
  if (index < 0) return;
  draft[fromKey].splice(index, 1);
  if (!draft[toKey].includes(id)) {
    draft[toKey].push(id);
  }
}

function reorderSidebarItem(routeId, listName, direction) {
  const draft = ensureSidebarDraft();
  const key = listName === "more" ? "moreItems" : "mainItems";
  const list = draft[key];
  const index = list.indexOf(String(routeId || ""));
  const nextIndex = index + direction;
  if (index < 0 || nextIndex < 0 || nextIndex >= list.length) return;
  const [item] = list.splice(index, 1);
  list.splice(nextIndex, 0, item);
}

function emitSidebarCustomizationChanged() {
  if (typeof globalThis.window?.dispatchEvent !== "function" || typeof CustomEvent !== "function") return;
  globalThis.window.dispatchEvent(new CustomEvent("sidebar-customization-changed"));
}

function syncConfigFormToEditor() {
  const editor = $("#config-editor");
  const fields = Array.from(document.querySelectorAll("[data-config-path]"))
    .filter((field) => !field.disabled && !field.dataset.configPath.includes("[]"));
  if (!editor || !fields.length) {
    return;
  }

  const base = parseConfigText(editor.value || state.configEditor) || state.config || {};
  for (const field of fields) {
    setPathValue(base, field.dataset.configPath, configFieldValue(field));
  }

  state.configEditor = JSON.stringify(base, null, 2);
  editor.value = state.configEditor;
}

function parseConfigText(text) {
  if (!text?.trim()) {
    return {};
  }
  return JSON.parse(text);
}

function configFieldValue(field) {
  const control = field.dataset.configControl;
  const valueType = field.dataset.configType;

  if (control === "toggle") {
    return field.checked;
  }
  if (control === "number") {
    const value = Number(field.value);
    if (!Number.isInteger(value)) {
      throw new Error(`${field.dataset.configPath} 必须是整数`);
    }
    return value;
  }
  if (control === "list" || control === "object") {
    const text = field.value.trim();
    if (!text) {
      return control === "list" ? [] : {};
    }
    return JSON.parse(text);
  }
  if (valueType === "optional_string" && field.value === "") {
    return null;
  }
  return field.value;
}

function setPathValue(target, path, value) {
  const parts = path.split(".");
  let cursor = target;
  for (const part of parts.slice(0, -1)) {
    if (!cursor[part] || typeof cursor[part] !== "object" || Array.isArray(cursor[part])) {
      cursor[part] = {};
    }
    cursor = cursor[part];
  }
  cursor[parts[parts.length - 1]] = value;
}
