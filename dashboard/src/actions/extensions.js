import { api, apiUrl } from "../api.js";
import { $, escapeHtml, showToast } from "../dom.js";
import {
  loadLogs,
  loadMarket,
  loadCommands,
  loadMcp,
  loadPluginLifecycle,
  loadPlatformCatalog,
  loadProviderCatalog,
  loadSkills,
  loadSkillsNeo,
  loadTools,
  loadTrace,
  loadTraceSettings,
} from "../loaders.js";
import { state } from "../state.js";
import { splitCsv } from "./forms.js";

let logStream = null;
const REDACTED_SECRET_VALUES = new Set(["__redacted__", "<redacted>"]);

export async function handleExtensionActions({ action, target }) {
    if (action === "load-providers") await loadProviderCatalog();
    if (action === "load-platforms") await loadPlatformCatalog();
    if (action === "load-tools") await loadTools();
    if (action === "load-commands") await loadCommands();
    if (action === "load-mcp") await loadMcp();
    if (action === "load-plugin-lifecycle") await loadPluginLifecycle();
    if (action === "load-market") await loadMarket();
    if (action === "load-skills") await loadSkills();
    if (action === "load-skills-neo") await loadSkillsNeo();
    if (action === "load-logs") await loadLogs();
    if (action === "load-trace") await Promise.all([loadTrace(), loadTraceSettings()]);
    if (action === "console-filter") {
      state.consoleSearch = $("#console-search")?.value.trim() || "";
    }
    if (action === "console-level-toggle") {
      const level = target.dataset.level || "";
      const current = new Set(Array.isArray(state.consoleLevels) && state.consoleLevels.length
        ? state.consoleLevels
        : ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]);
      if (current.has(level)) {
        current.delete(level);
      } else {
        current.add(level);
      }
      state.consoleLevels = [...current];
    }
    if (action === "console-autoscroll-toggle") {
      state.consoleAutoScroll = target.matches("input") ? target.checked : state.consoleAutoScroll === false;
    }
    if (action === "console-pip-open") {
      state.consolePipDialog = "install";
    }
    if (action === "console-pip-close") {
      state.consolePipDialog = "";
      state.consolePipPackage = "";
      state.consolePipMirror = "";
    }
    if (action === "console-pip-install") {
      state.consolePipPackage = $("#console-pip-package")?.value.trim() || "";
      state.consolePipMirror = $("#console-pip-mirror")?.value.trim() || "";
      if (!state.consolePipPackage) throw new Error("缺少参数 package 或不合法。");
      state.operation = await api("/api/update/pip-install", {
        method: "POST",
        body: JSON.stringify({
          package: state.consolePipPackage,
          mirror: state.consolePipMirror || null,
        }),
      });
      state.consolePipDialog = "";
      showToast(state.operation.message || "安装成功。");
    }
    if (action === "logs-stream-start") {
      if (logStream) logStream.close();
      const cursor = state.logs?.snapshot?.next_cursor;
      const params = new URLSearchParams({ limit: "25", interval_ms: "1000" });
      if (cursor) params.set("after", String(cursor));
      logStream = new EventSource(apiUrl(`/api/live-log?${params.toString()}`));
      state.logStreamStatus = "SSE 已连接，等待新日志";
      logStream.onmessage = (event) => {
        const payload = JSON.parse(event.data);
        appendConsolePayload(payload);
        state.logStreamStatus = `SSE 已连接，最新日志 #${payload.id || event.lastEventId || ""}`;
        const node = $("#console-stream-state");
        if (node) node.textContent = state.logStreamStatus;
      };
      logStream.onerror = () => {
        state.logStreamStatus = "SSE 连接中断";
        const node = $("#console-stream-state");
        if (node) node.textContent = state.logStreamStatus;
      };
      showToast("日志 SSE 已连接");
    }
    if (action === "logs-stream-stop") {
      if (logStream) logStream.close();
      logStream = null;
      state.logStreamStatus = "SSE 已停止";
      const node = $("#console-stream-state");
      if (node) node.textContent = state.logStreamStatus;
      showToast("日志 SSE 已停止");
    }
    if (action === "trace-settings-save") {
      state.traceSettings = await api("/api/management/trace/settings", {
        method: "POST",
        body: JSON.stringify({
          enabled: $("#trace-enabled").checked,
          capture_message_outline: $("#trace-outline").checked,
          max_events: Number($("#trace-max-events").value || 500),
          redact_fields: splitCsv($("#trace-redact-fields").value),
        }),
      });
      await Promise.all([loadTrace(), loadTraceSettings()]);
      showToast("Trace 设置已保存");
    }
    if (action === "trace-toggle-event") {
      const span = target.dataset.span || "";
      state.traceExpanded = {
        ...(state.traceExpanded || {}),
        [span]: state.traceExpanded?.[span] !== true,
      };
    }
    if (action === "extension-filter") {
      state.extensionPluginSearch = $("#plugin-search")?.value || "";
      state.extensionPluginStatusFilter = $("#plugin-status-filter")?.value || "all";
      state.marketSearch = $("#market-search")?.value || state.marketSearch || "";
      state.marketSortBy = $("#market-sort-by")?.value || state.marketSortBy || "default";
      state.marketSortOrder = $("#market-sort-order")?.value || state.marketSortOrder || "desc";
      state.commandSearch = $("#command-search")?.value || state.commandSearch || "";
      state.commandPluginFilter = $("#command-plugin-filter")?.value || state.commandPluginFilter || "all";
      state.commandPermissionFilter = $("#command-permission-filter")?.value || state.commandPermissionFilter || "all";
      state.commandStatusFilter = $("#command-status-filter")?.value || state.commandStatusFilter || "all";
      state.toolSearch = $("#tool-search")?.value || state.toolSearch || "";
    }
    if (action === "plugin-view-mode") {
      state.extensionPluginView = target.dataset.view === "list" ? "list" : "grid";
      window.localStorage?.setItem("pluginListViewMode", String(state.extensionPluginView === "list"));
    }
    if (action === "plugin-toggle-reserved") {
      state.extensionPluginShowReserved = !state.extensionPluginShowReserved;
      window.localStorage?.setItem("showReservedPlugins", String(state.extensionPluginShowReserved));
    }
    if (action === "market-random-toggle") {
      state.marketShowRandom = !state.marketShowRandom;
    }
    if (action === "market-sort") {
      state.marketSortBy = target.dataset.sort || state.marketSortBy || "default";
      state.marketSortOrder = target.dataset.order || (state.marketSortOrder === "desc" ? "asc" : "desc");
    }
    if (action === "plugin-doc-open") {
      const pluginId = target.dataset.plugin;
      const docKind = target.dataset.doc || "readme";
      state.extensionDoc = pluginDocument(pluginId, docKind);
      state.extensionDialog = "plugin-doc";
    }
    if (action === "plugin-doc-close") {
      state.extensionDialog = "";
      state.extensionDoc = null;
    }
    if (action === "plugin-config-open") {
      const plugin = pluginById(target.dataset.plugin);
      state.extensionDialog = "plugin-config";
      state.extensionDoc = {
        plugin_id: plugin?.plugin_id || target.dataset.plugin || "",
        title: `${plugin?.name || target.dataset.plugin || "Plugin"} config`,
        config: plugin?.config || {},
        filename: plugin?.config_files?.[0]?.filename || "config.json",
      };
    }
    if (action === "plugin-source-open") {
      const plugin = pluginById(target.dataset.plugin);
      state.extensionDialog = "plugin-source";
      state.extensionDoc = {
        title: `${plugin?.name || target.dataset.plugin || "Plugin"} source`,
        source: plugin?.source || {},
        capabilities: plugin?.capabilities || [],
        permissions: plugin?.permissions || [],
      };
    }
    if (action === "extension-dialog-close") {
      state.extensionDialog = "";
      state.extensionDoc = null;
      state.commandDetailsId = "";
      state.commandRenameId = "";
      state.toolDetailsName = "";
    }
    if (action === "toggle-tool") {
      await api("/api/management/tools/toggle", {
        method: "POST",
        body: JSON.stringify({ name: target.dataset.tool, active: target.dataset.active === "true" }),
      });
      await loadTools();
      showToast("工具状态已更新");
    }
    if (action === "command-update") {
      state.operation = await api("/api/management/commands/update", {
        method: "POST",
        body: JSON.stringify(commandFormPayload()),
      });
      await loadCommands();
      showToast("Command 已保存");
    }
    if (action === "command-toggle") {
      state.operation = await api("/api/management/commands/update", {
        method: "POST",
        body: JSON.stringify({
          plugin_name: target.dataset.plugin,
          handler_name: target.dataset.handler,
          enabled: target.dataset.enabled === "true",
        }),
      });
      await loadCommands();
      showToast("Command 状态已更新");
    }
    if (action === "command-permission") {
      state.operation = await api("/api/management/commands/update", {
        method: "POST",
        body: JSON.stringify({
          plugin_name: target.dataset.plugin,
          handler_name: target.dataset.handler,
          permission: target.dataset.permission,
        }),
      });
      await loadCommands();
      showToast("Command 权限已更新");
    }
    if (action === "command-details-open") {
      state.commandDetailsId = target.dataset.command || "";
      state.commandRenameId = "";
    }
    if (action === "command-rename-open") {
      state.commandRenameId = target.dataset.command || "";
      state.commandDetailsId = "";
    }
    if (action === "command-rename-save") {
      const command = commandByFullName(state.commandRenameId);
      if (!command) throw new Error("未找到 Command");
      state.operation = await api("/api/management/commands/update", {
        method: "POST",
        body: JSON.stringify({
          plugin_name: command.plugin_name,
          handler_name: command.handler_name,
          command: $("#command-rename-command").value.trim(),
          permission: $("#command-rename-permission").value,
          enabled: $("#command-rename-enabled").checked,
        }),
      });
      state.commandRenameId = "";
      await loadCommands();
      showToast("Command 已重命名");
    }
    if (action === "tool-details-open") {
      state.toolDetailsName = target.dataset.tool || "";
    }
    if (action === "mcp-upsert") {
      state.operation = await api("/api/management/mcp/servers/upsert", {
        method: "POST",
        body: JSON.stringify(mcpFormPayload()),
      });
      await loadMcp();
      showToast("MCP server 已保存");
    }
    if (action === "mcp-check-form") {
      state.operation = await api("/api/management/mcp/servers/check", {
        method: "POST",
        body: JSON.stringify({ server: mcpFormPayload().server }),
      });
      showToast("MCP 配置检查完成");
    }
    if (action === "mcp-check") {
      state.operation = await api("/api/management/mcp/servers/check", {
        method: "POST",
        body: JSON.stringify({ name: target.dataset.mcp }),
      });
      showToast("MCP 配置检查完成");
    }
    if (action === "mcp-sync") {
      state.operation = await api("/api/management/mcp/servers/sync", {
        method: "POST",
        body: JSON.stringify({ names: [target.dataset.mcp] }),
      });
      showToast("MCP bridge plan 已生成");
    }
    if (action === "mcp-delete") {
      state.operation = await api("/api/management/mcp/servers/delete", {
        method: "POST",
        body: JSON.stringify({ name: target.dataset.mcp }),
      });
      await loadMcp();
      showToast("MCP server 已删除");
    }
    if (action === "mcp-edit-json") {
      const server = (state.mcp?.servers || []).find((item) => item.name === target.dataset.mcp);
      if (!server) throw new Error("未找到 MCP server");
      state.mcpEditName = server.name;
      state.mcpJsonDraft = JSON.stringify(mcpServerJson(server), null, 2);
    }
    if (action === "mcp-json-template") {
      const type = target.dataset.template || "stdio";
      state.mcpEditName = "";
      state.mcpJsonDraft = JSON.stringify(mcpTemplate(type), null, 2);
    }
    if (action === "mcp-json-upsert") {
      const server = JSON.parse($("#mcp-json").value || "{}");
      const name = ($("#mcp-json-name").value || state.mcpEditName || "").trim();
      if (!name) throw new Error("MCP name 不能为空");
      state.operation = await api("/api/management/mcp/servers/upsert", {
        method: "POST",
        body: JSON.stringify({ name, server }),
      });
      state.mcpEditName = name;
      state.mcpJsonDraft = JSON.stringify(server, null, 2);
      await loadMcp();
      showToast("MCP JSON 配置已保存");
    }
    if (action === "mcp-sync-provider") {
      state.mcpSyncProvider = $("#mcp-sync-provider")?.value || state.mcpSyncProvider || "modelscope";
      state.operation = await api("/api/management/mcp/servers/sync", {
        method: "POST",
        body: JSON.stringify({ names: null, provider: state.mcpSyncProvider }),
      });
      showToast("MCP provider sync plan 已生成");
    }
    if (action === "provider-upsert") {
      state.operation = await api("/api/management/providers/upsert", {
        method: "POST",
        body: JSON.stringify({
          provider: providerFormPayload(),
          set_default: $("#provider-default").checked,
        }),
      });
      await loadProviderCatalog();
      showToast("Provider 已保存");
    }
    if (action === "provider-tab") {
      state.providerTab = target.dataset.tab || "chat_completion";
      state.providerDialog = "";
      state.providerEditId = "";
      state.providerModelSearch = "";
    }
    if (action === "provider-dialog-open") {
      state.providerDialog = "add-provider";
    }
    if (action === "provider-dialog-close") {
      state.providerDialog = "";
      state.providerEditId = "";
      state.providerTemplateDraft = null;
      state.providerAgentRunnerDialog = false;
    }
    if (action === "provider-agent-runner-close") {
      state.providerAgentRunnerDialog = false;
    }
    if (action === "provider-agent-runner-go") {
      state.providerAgentRunnerDialog = false;
      window.location.hash = "#/config";
    }
    if (action === "provider-source-select") {
      state.providerSelectedSourceId = target.dataset.source || "";
      state.providerModels = [];
      state.providerModelMetadata = {};
      state.providerModelSearch = "";
    }
    if (action === "provider-source-add") {
      const template = providerTemplate(target.dataset.template);
      if (!template) throw new Error("未找到 provider source 模板");
      const source = sourceFromTemplate(template);
      const catalog = ensureProviderCatalog();
      catalog.provider_sources = catalog.provider_sources || [];
      catalog.provider_sources.push(source);
      state.providerSelectedSourceId = source.id;
      state.providerModels = [];
      state.providerModelMetadata = {};
      state.providerModelSearch = "";
      showToast("Provider source 草稿已创建，请保存配置");
    }
    if (action === "provider-source-save") {
      const source = providerSourceFormPayload();
      await api("/api/config/provider_sources/update", {
        method: "POST",
        body: JSON.stringify({
          original_id: state.providerSelectedSourceId || source.id,
          config: source,
        }),
      });
      state.providerSelectedSourceId = source.id;
      await loadProviderCatalog();
      showToast("Provider source 已保存");
    }
    if (action === "provider-source-delete") {
      const sourceId = target.dataset.source || state.providerSelectedSourceId;
      if (!sourceId) throw new Error("请先选择 provider source");
      await api("/api/config/provider_sources/delete", {
        method: "POST",
        body: JSON.stringify({ id: sourceId }),
      });
      state.providerSelectedSourceId = "";
      state.providerModels = [];
      state.providerModelMetadata = {};
      await loadProviderCatalog();
      showToast("Provider source 已删除");
    }
    if (action === "provider-source-models") {
      let source = selectedProviderSource();
      if (!source) throw new Error("请先选择 provider source");
      const currentSource = providerSourceFormPayload();
      if (providerSourceChanged(source, currentSource)) {
        await api("/api/config/provider_sources/update", {
          method: "POST",
          body: JSON.stringify({
            original_id: state.providerSelectedSourceId || source.id,
            config: currentSource,
          }),
        });
        state.providerSelectedSourceId = currentSource.id;
        await loadProviderCatalog();
        source = selectedProviderSource() || currentSource;
      }
      const result = await api(`/api/config/provider_sources/models?source_id=${encodeURIComponent(source.id)}`);
      const data = sourceData(result);
      state.providerModels = data.models || [];
      state.providerModelMetadata = data.model_metadata || {};
      showToast("模型建议已读取");
    }
    if (action === "provider-model-search") {
      state.providerModelSearch = $("#provider-model-search")?.value.trim() || "";
    }
    if (action === "provider-model-add") {
      const source = selectedProviderSource();
      const model = target.dataset.model;
      if (!source) throw new Error("请先选择 provider source");
      if (!model) throw new Error("模型 ID 不能为空");
      if (modelAlreadyConfigured(source, model)) throw new Error("模型已存在");
      await api("/api/config/provider/new", {
        method: "POST",
        body: JSON.stringify(providerPayloadFromSource(source, model)),
      });
      await loadProviderCatalog();
      showToast(`模型 ${model} 已添加`);
    }
    if (action === "provider-manual-open") {
      if (!selectedProviderSource()) throw new Error("请先选择 provider source");
      state.providerDialog = "manual-model";
    }
    if (action === "provider-manual-add") {
      const source = selectedProviderSource();
      const model = $("#provider-manual-model")?.value.trim();
      if (!source) throw new Error("请先选择 provider source");
      if (!model) throw new Error("请输入模型 ID");
      if (modelAlreadyConfigured(source, model)) throw new Error("模型已存在");
      await api("/api/config/provider/new", {
        method: "POST",
        body: JSON.stringify(providerPayloadFromSource(source, model)),
      });
      state.providerDialog = "";
      await loadProviderCatalog();
      showToast(`模型 ${model} 已添加`);
    }
    if (action === "provider-template-select") {
      const template = providerTemplate(target.dataset.template);
      if (!template) throw new Error("未找到 provider 模板");
      if (providerCategory(template) === "chat_completion") {
        const source = sourceFromTemplate(template);
        const catalog = ensureProviderCatalog();
        catalog.provider_sources = catalog.provider_sources || [];
        catalog.provider_sources.push(source);
        state.providerTab = "chat_completion";
        state.providerSelectedSourceId = source.id;
        state.providerDialog = "";
        showToast("Provider source 草稿已创建，请保存配置");
      } else {
        state.providerTemplateDraft = { ...template };
        state.providerDialog = "provider-config";
      }
    }
    if (action === "provider-template-save") {
      const payload = providerEditPayload("provider-new", state.providerTemplateDraft || {});
      await api("/api/config/provider/new", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      state.providerDialog = "";
      state.providerTemplateDraft = null;
      await loadProviderCatalog();
      showToast("Provider 已保存");
    }
    if (action === "provider-check-form") {
      state.operation = await api("/api/management/providers/check", {
        method: "POST",
        body: JSON.stringify({ provider: providerFormPayload() }),
      });
      showToast("Provider 检查完成");
    }
    if (action === "provider-check") {
      const provider = configuredProvider(target.dataset.provider);
      if (!provider) throw new Error("Provider 不存在");
      if (providerCategory(provider) === "agent_runner") {
        state.providerAgentRunnerDialog = true;
        return;
      }
      if (provider.enable === false || provider.enabled === false) {
        recordProviderStatus(provider.id, {
          id: provider.id,
          name: provider.id,
          status: "unavailable",
          error: "该提供商未被用户启用",
        });
        showToast("该提供商未被用户启用", "error");
        return;
      }
      recordProviderStatus(provider.id, {
        id: provider.id,
        name: provider.id,
        status: "pending",
        error: null,
      });
      const start = performance.now();
      try {
        const result = await api(`/api/config/provider/check_one?id=${encodeURIComponent(provider.id)}`);
        const data = sourceData(result);
        const status = data.error ? "unavailable" : data.status || "available";
        state.operation = result;
        recordProviderStatus(provider.id, { id: provider.id, name: provider.id, ...data, status });
        const latency = Math.max(0, Math.round(performance.now() - start));
        showToast(status === "available" ? `模型 ${provider.id} 测试通过，延迟 ${latency} ms` : data.error || "Provider 检查完成", status === "available" ? "ok" : "error");
      } catch (error) {
        recordProviderStatus(provider.id, {
          id: provider.id,
          name: provider.id,
          status: "unavailable",
          error: error.message,
        });
        showToast(error.message || "Provider 检查失败", "error");
      }
    }
    if (action === "provider-delete") {
      state.operation = await api("/api/config/provider/delete", {
        method: "POST",
        body: JSON.stringify({ id: target.dataset.provider }),
      });
      await loadProviderCatalog();
      showToast("Provider 已删除");
    }
    if (action === "provider-copy") {
      const provider = configuredProvider(target.dataset.provider);
      if (!provider) throw new Error("Provider 不存在");
      const copy = providerCopyPayload(provider);
      await api("/api/config/provider/new", {
        method: "POST",
        body: JSON.stringify(copy),
      });
      await loadProviderCatalog();
      showToast(`Provider 已复制为 ${copy.id}`);
    }
    if (action === "provider-toggle") {
      const provider = configuredProvider(target.dataset.provider);
      if (!provider) throw new Error("Provider 不存在");
      const enabled = !(provider.enable !== false && provider.enabled !== false);
      await api("/api/config/provider/update", {
        method: "POST",
        body: JSON.stringify({
          id: provider.id,
          config: { ...provider, enable: enabled, enabled },
        }),
      });
      await loadProviderCatalog();
      showToast("Provider 状态已更新");
    }
    if (action === "provider-edit-open") {
      state.providerEditId = target.dataset.provider || "";
    }
    if (action === "provider-edit-save") {
      if (!state.providerEditId) throw new Error("Provider 不存在");
      const provider = configuredProvider(state.providerEditId);
      if (!provider) throw new Error("Provider 不存在");
      await api("/api/config/provider/update", {
        method: "POST",
        body: JSON.stringify({
          id: state.providerEditId,
          config: providerEditPayload("provider-edit", provider),
        }),
      });
      state.providerEditId = "";
      await loadProviderCatalog();
      showToast("Provider 已保存");
    }
    if (action === "provider-embedding-dim") {
      const prefix = target.dataset.prefix || (state.providerDialog === "provider-config" ? "provider-new" : "provider-edit");
      const base = prefix === "provider-new" ? state.providerTemplateDraft || {} : configuredProvider(state.providerEditId) || {};
      const result = await api("/api/config/provider/get_embedding_dim", {
        method: "POST",
        body: JSON.stringify({ provider_config: providerEditPayload(prefix, base) }),
      });
      const data = sourceData(result);
      const dimension = data.embedding_dimensions ?? data.dimension;
      if (!dimension) throw new Error("未获取到 embedding 维度");
      const field = $(`#${prefix}-dimensions`);
      if (field) field.value = String(dimension);
      persistProviderEmbeddingDimension(prefix, dimension);
      state.operation = result;
      showToast(`获取成功: ${dimension}`);
    }
    if (action === "provider-models") {
      state.operation = await api("/api/management/providers/models", {
        method: "POST",
        body: JSON.stringify({ provider_type: $("#provider-type").value }),
      });
      showToast("模型建议已读取");
    }
    if (action === "platform-dialog-open") {
      initializePlatformDraft();
    }
    if (action === "platform-dialog-close") {
      resetPlatformDialogState();
    }
    if (action === "platform-template-select") {
      const template = platformTemplate(target.dataset.template);
      if (!template) throw new Error("未找到 platform 模板");
      state.platformSelectedTemplate = template.name;
      state.platformDraft = platformPayloadFromTemplate(template);
    }
    if (action === "platform-config-section-toggle") {
      captureVisiblePlatformDraft();
      state.platformShowConfigSection = !state.platformShowConfigSection;
    }
    if (action === "platform-config-existing-mode") {
      captureVisiblePlatformDraft();
      state.platformConfigMode = "existing";
      state.platformNewConfigDraft = null;
    }
    if (action === "platform-config-new-mode") {
      captureVisiblePlatformDraft();
      state.platformConfigMode = "new";
      if (!state.platformNewConfigDraft) {
        const result = await api("/api/config/default");
        state.platformNewConfigDraft = sourceData(result).config || {};
      }
    }
    if (action === "platform-save-new") {
      const platform = platformEditPayload("platform-new", state.platformDraft || {});
      validatePlatformPayload(platform);
      const existing = configuredPlatform(platform.id);
      if (existing || platform.id === "webchat") {
        throw new Error(`Platform ID 已存在或保留：${platform.id}`);
      }
      state.operation = await api("/api/management/platforms/upsert", {
        method: "POST",
        body: JSON.stringify({ platform }),
      });
      await bindPlatformConfig(platform.id);
      resetPlatformDialogState();
      await loadPlatformCatalog();
      showToast("Platform 已保存并绑定配置文件");
    }
    if (action === "platform-edit-open") {
      const platform = configuredPlatform(target.dataset.platform);
      if (!platform) throw new Error("Platform 不存在");
      state.platformEditId = platform.id;
      state.platformOriginalEditId = platform.id;
      state.platformDraft = { ...platform, options: { ...(platform.options || {}) }, secrets: { ...(platform.secrets || {}) } };
      state.platformShowConfigSection = true;
      state.platformRouteEdit = false;
      state.platformRouteDrafts = routeDraftsForPlatform(platform.id);
    }
    if (action === "platform-save-edit") {
      if (!state.platformEditId) throw new Error("Platform 不存在");
      const base = configuredPlatform(state.platformEditId) || state.platformDraft || {};
      const platform = platformEditPayload("platform-edit", base);
      validatePlatformPayload(platform);
      const originalId = state.platformOriginalEditId || state.platformEditId;
      state.operation = await api("/api/management/platforms/upsert", {
        method: "POST",
        body: JSON.stringify({ platform }),
      });
      if (originalId && originalId !== platform.id) {
        await api("/api/management/platforms/delete", {
          method: "POST",
          body: JSON.stringify({ id: originalId }),
        });
      }
      await savePlatformRoutesInternal(originalId, platform.id);
      resetPlatformDialogState();
      await loadPlatformCatalog();
      showToast("Platform 已更新");
    }
    if (action === "platform-route-edit-toggle") {
      captureVisiblePlatformDraft();
      state.platformRouteDrafts = readPlatformRouteDrafts();
      state.platformRouteEdit = !state.platformRouteEdit;
    }
    if (action === "platform-route-add") {
      captureVisiblePlatformDraft();
      state.platformRouteDrafts = [
        ...readPlatformRouteDrafts(),
        { messageType: "*", sessionId: "*", configId: "default" },
      ];
      state.platformRouteEdit = true;
    }
    if (action === "platform-route-delete") {
      captureVisiblePlatformDraft();
      state.platformRouteDrafts = readPlatformRouteDrafts().filter((_, index) => index !== Number(target.dataset.index));
      state.platformRouteEdit = true;
    }
    if (action === "platform-route-up" || action === "platform-route-down") {
      captureVisiblePlatformDraft();
      const drafts = readPlatformRouteDrafts();
      const index = Number(target.dataset.index);
      const delta = action === "platform-route-up" ? -1 : 1;
      const next = index + delta;
      if (index >= 0 && next >= 0 && next < drafts.length) {
        [drafts[index], drafts[next]] = [drafts[next], drafts[index]];
      }
      state.platformRouteDrafts = drafts;
      state.platformRouteEdit = true;
    }
    if (action === "platform-console-toggle") {
      state.platformShowConsole = !platformConsoleVisible();
      if (typeof window !== "undefined") {
        window.localStorage.setItem("platformPage_showConsole", String(state.platformShowConsole));
      }
      if (state.platformShowConsole && !state.logs) {
        await loadLogs();
      }
    }
    if (action === "platform-error-open") {
      state.platformErrorId = target.dataset.platform || "";
    }
    if (action === "platform-error-close") {
      state.platformErrorId = "";
    }
    if (action === "platform-webhook-open") {
      state.platformWebhookUuid = target.dataset.webhook || "";
    }
    if (action === "platform-webhook-close") {
      state.platformWebhookUuid = "";
    }
    if (action === "platform-webhook-copy") {
      const value = $("#platform-webhook-url")?.value || "";
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(value);
      }
      showToast("Webhook URL 已复制");
    }
    if (action === "platform-upsert") {
      state.operation = await api("/api/management/platforms/upsert", {
        method: "POST",
        body: JSON.stringify({ platform: platformFormPayload() }),
      });
      await loadPlatformCatalog();
      showToast("Platform 已保存");
    }
    if (action === "platform-check-form") {
      const platform = state.platformEditId
        ? platformEditPayload("platform-edit", configuredPlatform(state.platformEditId) || {})
        : state.platformDraft
          ? platformEditPayload("platform-new", state.platformDraft)
          : platformFormPayload();
      state.operation = await api("/api/management/platforms/check", {
        method: "POST",
        body: JSON.stringify({ platform }),
      });
      showToast("Platform 检查完成");
    }
    if (action === "platform-check") {
      const platformId = target.dataset.platform;
      recordPlatformCheck(platformId, { status: "pending", message: "" });
      try {
        state.operation = await api("/api/management/platforms/check", {
          method: "POST",
          body: JSON.stringify({ id: platformId }),
        });
        recordPlatformCheck(platformId, { status: "available", message: state.operation.message || "ok" });
        showToast("Platform 检查完成");
      } catch (error) {
        recordPlatformCheck(platformId, { status: "unavailable", message: error.message });
        throw error;
      }
    }
    if (action === "platform-toggle") {
      const platform = configuredPlatform(target.dataset.platform);
      if (!platform) throw new Error("Platform 不存在");
      const enabled = !(platform.enabled !== false && platform.enable !== false);
      state.operation = await api("/api/management/platforms/upsert", {
        method: "POST",
        body: JSON.stringify({ platform: { ...platform, enabled, enable: enabled } }),
      });
      await loadPlatformCatalog();
      showToast("Platform 状态已更新");
    }
    if (action === "platform-delete") {
      state.operation = await api("/api/management/platforms/delete", {
        method: "POST",
        body: JSON.stringify({ id: target.dataset.platform }),
      });
      await loadPlatformCatalog();
      showToast("Platform 已删除");
    }
    if (action === "plugin-plan") {
      const plan = target.dataset.plan;
      const suffix = plan === "install" ? "install-plan" : plan === "update" ? "update-plan" : "uninstall-plan";
      state.operation = await api(`/api/management/plugin-market/${suffix}`, {
        method: "POST",
        body: JSON.stringify({ plugin_id: target.dataset.plugin, delete_config: false, delete_data: false }),
      });
      showToast("插件 plan 已生成");
    }
    if (action === "plugin-update-all-plan") {
      state.operation = await api("/api/management/plugin-market/update-all-plan");
      showToast("插件 update-all plan 已生成");
    }
    if (action === "plugin-update-all") {
      state.operation = await api("/api/management/plugin-market/update-all", {
        method: "POST",
        body: JSON.stringify({ ignore_compatibility: false }),
      });
      await loadMarket();
      showToast("插件 update-all 已执行");
    }
    if (action === "plugin-execute") {
      const plan = target.dataset.plan;
      state.operation = await api(`/api/management/plugin-market/${plan}`, {
        method: "POST",
        body: JSON.stringify({
          plugin_id: target.dataset.plugin,
          delete_config: plan === "uninstall",
          delete_data: false,
          ignore_compatibility: false,
        }),
      });
      await loadMarket();
      showToast(`插件${plan === "install" ? "安装" : plan === "update" ? "更新" : "卸载"}已执行`);
    }
    if (action === "plugin-lifecycle") {
      state.operation = await api("/api/management/plugins/lifecycle/action", {
        method: "POST",
        body: JSON.stringify({ plugin_id: target.dataset.plugin, action: target.dataset.lifecycle }),
      });
      await loadPluginLifecycle();
      showToast("插件 lifecycle 已更新");
    }
    if (action === "plugin-upload-plan") {
      const entries = $("#plugin-upload-entries").value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      state.operation = await api("/api/management/plugins/upload-plan", {
        method: "POST",
        body: JSON.stringify({ entries, overwrite: $("#plugin-upload-overwrite").checked }),
      });
      showToast("插件上传计划已生成");
    }
    if (action === "plugin-source-plan") {
      state.operation = await api("/api/management/plugins/source-plan", {
        method: "POST",
        body: JSON.stringify(pluginSourceFormPayload()),
      });
      showToast("插件来源计划已生成");
    }
    if (action === "plugin-config-save") {
      state.operation = await api("/api/management/plugins/config", {
        method: "POST",
        body: JSON.stringify({
          plugin_id: $("#plugin-config-id").value.trim(),
          config: JSON.parse($("#plugin-config-json").value || "{}"),
        }),
      });
      await loadPluginLifecycle();
      showToast("插件配置已保存");
    }
    if (action === "plugin-config-file-read") {
      state.operation = await api("/api/management/plugins/config-file/read", {
        method: "POST",
        body: JSON.stringify(pluginConfigFilePayload()),
      });
      if ($("#plugin-config-json")) {
        $("#plugin-config-json").value = JSON.stringify(state.operation.config, null, 2);
      }
      showToast("插件配置文件已读取");
    }
    if (action === "plugin-config-file-write") {
      state.operation = await api("/api/management/plugins/config-file/write", {
        method: "POST",
        body: JSON.stringify({
          ...pluginConfigFilePayload(),
          config: JSON.parse($("#plugin-config-json").value || "{}"),
        }),
      });
      await loadPluginLifecycle();
      showToast("插件配置文件已写入");
    }
    if (action === "plugin-config-file-delete") {
      state.operation = await api("/api/management/plugins/config-file/delete", {
        method: "POST",
        body: JSON.stringify(pluginConfigFilePayload()),
      });
      await loadPluginLifecycle();
      showToast(state.operation.deleted ? "插件配置文件已删除" : "插件配置文件不存在");
    }
    if (action === "skill-active") {
      await api("/api/management/skills/activation", {
        method: "POST",
        body: JSON.stringify({ name: target.dataset.skill, active: target.dataset.active === "true" }),
      });
      await loadSkills();
      showToast("技能状态已更新");
    }
    if (action === "skill-delete-plan") {
      state.operation = await api("/api/management/skills/delete-plan", {
        method: "POST",
        body: JSON.stringify({ name: target.dataset.skill }),
      });
      showToast("删除计划已生成");
    }
    if (action === "skill-delete") {
      state.operation = await api("/api/management/skills/delete", {
        method: "POST",
        body: JSON.stringify({ name: target.dataset.skill }),
      });
      await loadSkills();
      showToast("技能已删除");
    }
    if (action === "skill-install-plan") {
      const entries = $("#skill-entries").value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      state.operation = await api("/api/management/skills/install-plan", {
        method: "POST",
        body: JSON.stringify({ entries, overwrite: true }),
      });
      showToast("安装计划已生成");
    }
    if (action === "skill-install") {
      const entries = $("#skill-entries").value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      state.operation = await api("/api/management/skills/install", {
        method: "POST",
        body: JSON.stringify({ entries, overwrite: true }),
      });
      await loadSkills();
      showToast("技能已安装");
    }
    if (action === "skills-mode") {
      state.skillsMode = target.dataset.mode === "neo" ? "neo" : "local";
      if (state.skillsMode === "neo") await loadSkillsNeo();
    }
    if (action === "skill-download") {
      const name = target.dataset.skill || "";
      state.operation = {
        capability: "unavailable",
        action: "download",
        name,
        message: "当前目标后端尚未提供 /api/skills/download；UI 已保留下载入口并标注 capability。",
      };
      showToast("Skill 下载当前不可用", "warn");
    }
    if (action === "skill-neo-refresh") {
      await loadSkillsNeo();
      showToast("Neo Skills 状态已刷新");
    }
    if (action === "skill-neo-action") {
      const endpoint = target.dataset.endpoint;
      const payload = JSON.parse(target.dataset.payload || "{}");
      state.operation = await api(`/api/skills/neo/${endpoint}`, {
        method: "POST",
        body: JSON.stringify(payload),
      });
      await loadSkillsNeo();
      showToast("Neo Skills 操作已提交");
    }
    if (action === "skill-payload-open") {
      const payloadRef = target.dataset.payloadRef || "";
      state.skillsPayload = await api(`/api/skills/neo/payload?payload_ref=${encodeURIComponent(payloadRef)}`);
      state.extensionDialog = "skill-payload";
    }
}

function pluginById(pluginId) {
  return (state.pluginLifecycle?.plugins || []).find((plugin) => plugin.plugin_id === pluginId)
    || (state.market?.plugins || []).find((plugin) => plugin.plugin_id === pluginId)
    || null;
}

function pluginDocument(pluginId, docKind) {
  const lifecyclePlugin = (state.pluginLifecycle?.plugins || []).find((plugin) => plugin.plugin_id === pluginId);
  const marketPlugin = (state.market?.plugins || []).find((plugin) => plugin.plugin_id === pluginId);
  const plugin = marketPlugin || lifecyclePlugin || pluginById(pluginId);
  const marketDoc = docKind === "changelog" ? marketPlugin?.changelog : marketPlugin?.readme;
  const lifecycleDoc = docKind === "changelog" ? lifecyclePlugin?.changelog : lifecyclePlugin?.readme;
  const doc = marketDoc || lifecycleDoc;
  const markdown = doc?.markdown || doc?.content || doc?.body || marketPlugin?.description || lifecyclePlugin?.description || "";
  return {
    plugin_id: pluginId,
    kind: docKind,
    title: `${plugin?.name || pluginId || "Plugin"} ${docKind === "changelog" ? "Changelog" : "README"}`,
    repo_url: plugin?.repo_url || plugin?.repo || "",
    markdown: markdown || `No ${docKind} document is available from the current plugin management API.`,
    capability: markdown ? "runtime" : "unavailable",
  };
}

function commandByFullName(fullName) {
  return (state.commands?.commands || []).find((command) => command.handler_full_name === fullName)
    || null;
}

function mcpServerJson(server = {}) {
  const config = {
    active: server.active !== false,
    transport: server.transport || "stdio",
    sessionReadTimeoutSeconds: server.session_read_timeout_seconds || server.sessionReadTimeoutSeconds || 60,
    clientCapabilities: server.client_capabilities || { elicitation: { enabled: false }, sampling: { enabled: false } },
  };
  if (server.command) config.command = server.command;
  if (server.args?.length) config.args = server.args;
  if (server.url) config.url = server.url;
  return config;
}

function mcpTemplate(type = "stdio") {
  if (type === "sse" || type === "streamable_http") {
    return {
      active: true,
      transport: type,
      url: type === "sse" ? "https://example.invalid/sse" : "https://example.invalid/mcp",
      headers: {},
      sessionReadTimeoutSeconds: 60,
      clientCapabilities: { elicitation: { enabled: false }, sampling: { enabled: false } },
    };
  }
  return {
    active: true,
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem"],
    sessionReadTimeoutSeconds: 60,
    clientCapabilities: { elicitation: { enabled: false }, sampling: { enabled: false } },
  };
}

function sourceData(result) {
  return result?.data || result || {};
}

function appendConsolePayload(payload = {}) {
  if (payload.type === "trace") {
    state.trace = {
      ...(state.trace || {}),
      source_events: [...(state.trace?.source_events || []), payload].slice(-300),
    };
    return;
  }
  const sourceLogs = [...(state.logs?.source_logs || []), payload].slice(-200);
  state.logs = {
    ...(state.logs || {}),
    source_logs: sourceLogs,
  };
  const terminal = $("#console-terminal");
  if (!terminal) return;
  terminal.insertAdjacentHTML("beforeend", consolePayloadHtml(payload));
  if (terminal.dataset.autoScroll !== "false") {
    terminal.scrollTop = terminal.scrollHeight;
  }
}

function consolePayloadHtml(payload = {}) {
  const level = String(payload.level || "INFO").toUpperCase();
  const text = payload.data ?? payload.message ?? "";
  const source = payload.source || "";
  const target = payload.target ? ` -> ${payload.target}` : "";
  return `<pre class="console-log-line ${consoleLogKind(level)}"><span class="console-log-meta">[${escapeHtml(level)}] ${escapeHtml(source)}${escapeHtml(target)}</span><span>${escapeHtml(text)}</span></pre>`;
}

function consoleLogKind(level) {
  if (level === "ERROR" || level === "CRITICAL") return "error";
  if (level === "WARNING" || level === "WARN") return "warn";
  if (level === "INFO") return "ok";
  return "";
}

function ensureProviderCatalog() {
  state.providerCatalog = state.providerCatalog || {};
  if (state.providerCatalog.data) {
    state.providerCatalog = state.providerCatalog.data;
  }
  state.providerCatalog.config_schema = state.providerCatalog.config_schema || { provider: { config_template: {} } };
  state.providerCatalog.provider_sources = state.providerCatalog.provider_sources || [];
  state.providerCatalog.providers = state.providerCatalog.providers || [];
  return state.providerCatalog;
}

function providerTemplate(name) {
  const catalog = ensureProviderCatalog();
  return catalog.config_schema?.provider?.config_template?.[name] || null;
}

function providerTemplates() {
  return ensureProviderCatalog().config_schema?.provider?.config_template || {};
}

function providerCategory(provider = {}) {
  const type = provider.provider_type || provider.category || provider.type || "";
  if (["chat_completion", "agent_runner", "speech_to_text", "text_to_speech", "embedding", "rerank"].includes(type)) {
    return type;
  }
  if (type.includes("speech_to_text") || type.includes("stt")) return "speech_to_text";
  if (type.includes("text_to_speech") || type.includes("tts")) return "text_to_speech";
  if (type.includes("embedding")) return "embedding";
  if (type.includes("rerank")) return "rerank";
  if (["dify", "coze", "dashscope", "deerflow", "fastgpt"].includes(type)) return "agent_runner";
  return "chat_completion";
}

function sourceFromTemplate(template) {
  const source = {
    id: uniqueProviderSourceId(template.id || template.provider || "provider"),
    type: template.type || "openai_chat_completion",
    provider_type: "chat_completion",
    provider: template.provider || providerKind(template.type),
    enable: template.enable !== false,
    api_base: template.api_base || "",
    key: template.key || "",
    proxy: template.proxy || "",
    timeout_secs: template.timeout_secs || 120,
    custom_extra_body: template.custom_extra_body || {},
  };
  return source;
}

function uniqueProviderSourceId(baseId) {
  const catalog = ensureProviderCatalog();
  const existing = new Set((catalog.provider_sources || []).map((source) => source.id));
  if (!existing.has(baseId)) return baseId;
  let counter = 1;
  let candidate = `${baseId}_${counter}`;
  while (existing.has(candidate)) {
    counter += 1;
    candidate = `${baseId}_${counter}`;
  }
  return candidate;
}

function selectedProviderSource() {
  const catalog = ensureProviderCatalog();
  const sources = catalog.provider_sources || [];
  if (!sources.length) return null;
  return sources.find((source) => source.id === state.providerSelectedSourceId) || sources[0];
}

function configuredProvider(id) {
  const catalog = ensureProviderCatalog();
  return (catalog.providers || catalog.chat_providers || []).find((provider) => provider.id === id) || null;
}

function providerSourceFormPayload() {
  const id = requiredValue("provider-source-id", "Provider source ID");
  const key = $("#provider-source-key")?.value || "";
  return {
    id,
    type: requiredValue("provider-source-type", "Provider type"),
    provider_type: "chat_completion",
    provider: $("#provider-source-provider")?.value.trim() || providerKind($("#provider-source-type")?.value),
    enable: Boolean($("#provider-source-enabled")?.checked),
    api_base: emptyToNull($("#provider-source-api-base")?.value || ""),
    key: emptyToNull(key),
    api_key: emptyToNull(key),
    proxy: emptyToNull($("#provider-source-proxy")?.value || ""),
    timeout_secs: Math.max(1, Number($("#provider-source-timeout")?.value || 120)),
    custom_extra_body: parseJsonField("provider-source-extra", {}),
  };
}

function providerSourceChanged(source, next) {
  return JSON.stringify(normalizeProviderSource(source)) !== JSON.stringify(normalizeProviderSource(next));
}

function normalizeProviderSource(source = {}) {
  return {
    id: source.id || "",
    type: source.type || "openai_chat_completion",
    provider: source.provider || providerKind(source.type),
    enable: source.enable !== false && source.enabled !== false,
    api_base: source.api_base || null,
    key: source.key || source.api_key || null,
    proxy: source.proxy || null,
    timeout_secs: Number(source.timeout_secs || 120),
    custom_extra_body: source.custom_extra_body || {},
  };
}

function modelAlreadyConfigured(source, model) {
  const catalog = ensureProviderCatalog();
  const providers = catalog.providers || catalog.chat_providers || [];
  const expectedId = `${source.id}/${model}`;
  return providers.some((provider) => provider.id === expectedId || (providerSourceId(provider) === source.id && provider.model === model));
}

function providerSourceId(provider = {}) {
  return provider.provider_source_id || (provider.id || "").split("/")[0] || provider.id;
}

function providerPayloadFromSource(source, model) {
  const metadata = state.providerModelMetadata?.[model] || {};
  const modalities = metadata && Object.keys(metadata).length ? ["text"] : ["text", "image", "tool_use"];
  if (metadata?.modalities?.input?.includes("image") && !modalities.includes("image")) modalities.push("image");
  if (metadata?.tool_call && !modalities.includes("tool_use")) modalities.push("tool_use");
  const payload = {
    id: `${source.id}/${model}`,
    type: source.type || "openai_chat_completion",
    provider_type: "chat_completion",
    provider: source.provider || providerKind(source.type),
    provider_source_id: source.id,
    enable: false,
    enabled: false,
    model,
    modalities,
    api_base: source.api_base || null,
    timeout_secs: Number(source.timeout_secs || 120),
    custom_extra_body: source.custom_extra_body || {},
    max_context_tokens: metadata?.limit?.context || 0,
  };
  const key = source.key || source.api_key;
  if (key && !REDACTED_SECRET_VALUES.has(key)) {
    payload.key = key;
    payload.api_key = key;
  }
  return payload;
}

function providerCopyPayload(provider) {
  const copy = {
    ...provider,
    id: uniqueProviderId(`${provider.id}_copy`),
    enable: false,
    enabled: false,
  };
  if (REDACTED_SECRET_VALUES.has(copy.key)) delete copy.key;
  if (REDACTED_SECRET_VALUES.has(copy.api_key)) delete copy.api_key;
  return copy;
}

function uniqueProviderId(baseId) {
  const catalog = ensureProviderCatalog();
  const existing = new Set((catalog.providers || catalog.chat_providers || []).map((provider) => provider.id));
  if (!existing.has(baseId)) return baseId;
  let counter = 1;
  let candidate = `${baseId}_${counter}`;
  while (existing.has(candidate)) {
    counter += 1;
    candidate = `${baseId}_${counter}`;
  }
  return candidate;
}

function recordProviderStatus(providerId, status) {
  state.providerStatuses = {
    ...(state.providerStatuses || {}),
    [providerId]: status,
  };
}

function persistProviderEmbeddingDimension(prefix, dimension) {
  if (prefix === "provider-new") {
    state.providerTemplateDraft = {
      ...(state.providerTemplateDraft || {}),
      dimensions: dimension,
      embedding_dimensions: dimension,
    };
    return;
  }
  const catalog = ensureProviderCatalog();
  const providers = catalog.providers || catalog.chat_providers || [];
  const provider = providers.find((item) => item.id === state.providerEditId);
  if (provider) {
    provider.dimensions = dimension;
    provider.embedding_dimensions = dimension;
  }
}

function providerEditPayload(prefix, base = {}) {
  const key = $(`#${prefix}-key`)?.value || "";
  const extra = parseJsonField(`${prefix}-extra-json`, {});
  const payload = {
    ...base,
    ...extra,
    id: requiredValue(`${prefix}-id`, "Provider ID"),
    type: requiredValue(`${prefix}-type`, "Provider type"),
    provider_type: $(`#${prefix}-provider-type`)?.value || "chat_completion",
    enable: Boolean($(`#${prefix}-enabled`)?.checked),
    enabled: Boolean($(`#${prefix}-enabled`)?.checked),
    model: emptyToNull($(`#${prefix}-model`)?.value || ""),
    api_base: emptyToNull($(`#${prefix}-api-base`)?.value || ""),
    timeout_secs: Math.max(1, Number($(`#${prefix}-timeout`)?.value || 120)),
    dimensions: numberOrNull($(`#${prefix}-dimensions`)?.value || ""),
    provider_source_id: emptyToNull($(`#${prefix}-provider-source-id`)?.value || ""),
  };
  if (payload.dimensions) {
    payload.embedding_dimensions = payload.dimensions;
  } else {
    delete payload.dimensions;
    delete payload.embedding_dimensions;
  }
  if (key && !REDACTED_SECRET_VALUES.has(key)) {
    payload.key = key;
    payload.api_key = key;
  } else if (REDACTED_SECRET_VALUES.has(key) || REDACTED_SECRET_VALUES.has(base.key) || REDACTED_SECRET_VALUES.has(base.api_key)) {
    payload.key = "<redacted>";
    payload.api_key = "<redacted>";
  } else {
    delete payload.key;
    delete payload.api_key;
  }
  return payload;
}

function requiredValue(id, label) {
  const value = $(`#${id}`)?.value.trim() || "";
  if (!value) throw new Error(`${label} 不能为空`);
  return value;
}

function parseJsonField(id, fallback) {
  const value = $(`#${id}`)?.value.trim();
  if (!value) return fallback;
  try {
    return JSON.parse(value);
  } catch {
    throw new Error(`${id} 必须是有效 JSON`);
  }
}

function numberOrNull(value) {
  const trimmed = String(value || "").trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function providerKind(type = "") {
  return String(type || "openai").split("_")[0] || "openai";
}

function initializePlatformDraft() {
  const templates = platformTemplates();
  const template = templates.find((item) => item.name === state.platformSelectedTemplate) || templates[0];
  state.platformDialog = "add-platform";
  state.platformSelectedTemplate = template?.name || "";
  state.platformDraft = template ? platformPayloadFromTemplate(template) : null;
  state.platformConfigMode = "existing";
  state.platformSelectedConfigId = state.platformSelectedConfigId || "default";
  state.platformNewConfigName = "";
  state.platformNewConfigDraft = null;
  state.platformShowConfigSection = false;
  state.platformRouteEdit = false;
  state.platformRouteDrafts = [];
}

function resetPlatformDialogState() {
  state.platformDialog = "";
  state.platformDraft = null;
  state.platformEditId = "";
  state.platformOriginalEditId = "";
  state.platformConfigMode = "existing";
  state.platformNewConfigName = "";
  state.platformNewConfigDraft = null;
  state.platformShowConfigSection = false;
  state.platformRouteEdit = false;
  state.platformRouteDrafts = [];
}

function platformConsoleVisible() {
  if (state.platformShowConsole !== null) return Boolean(state.platformShowConsole);
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem("platformPage_showConsole") === "true";
}

function platformTemplates() {
  const catalog = state.platformCatalog?.data || state.platformCatalog || {};
  const metadataTemplates = state.platformMetadata?.platform_group?.metadata?.platform?.config_template;
  if (metadataTemplates && typeof metadataTemplates === "object") {
    return Object.entries(metadataTemplates).map(([name, template]) => platformTemplateFromObject(name, template));
  }
  const templates = (catalog.templates || []).map((template) => platformTemplateFromObject(template.label || template.platform_type, template));
  return templates.length ? templates : [
    platformTemplateFromObject("WebChat", { platform_type: "webchat", label: "WebChat" }),
    platformTemplateFromObject("Console", { platform_type: "console", label: "Console" }),
    platformTemplateFromObject("OneBot", { platform_type: "onebot", label: "OneBot" }),
    platformTemplateFromObject("Mock", { platform_type: "mock", label: "Mock" }),
  ];
}

function platformTemplate(name) {
  return platformTemplates().find((template) => template.name === name || template.type === name) || null;
}

function platformTemplateFromObject(name, template = {}) {
  const type = template.type || template.platform_type || template.id || name;
  return {
    name,
    label: template.label || name,
    type,
    runtime_supported: template.runtime_supported !== false,
    config: normalizePlatform({
      id: template.id || type,
      type,
      name: template.name || template.label || name,
      enabled: template.enabled ?? template.enable ?? true,
      options: template.options || defaultPlatformOptions(type),
      secrets: template.secrets || defaultPlatformSecrets(type),
    }),
  };
}

function defaultPlatformOptions(type) {
  if (type === "onebot" || type === "aiocqhttp") {
    return { ws_reverse_host: "0.0.0.0", ws_reverse_port: 6199 };
  }
  return {};
}

function defaultPlatformSecrets(type) {
  if (type === "telegram") return { telegram_token: "" };
  if (type === "onebot" || type === "aiocqhttp") return { ws_reverse_token: "" };
  return {};
}

function platformPayloadFromTemplate(template) {
  const base = normalizePlatform(template.config || {});
  const id = uniquePlatformId(base.id || base.type || "platform");
  return {
    ...base,
    id,
    name: base.name || template.label || id,
    options: { ...(base.options || {}) },
    secrets: { ...(base.secrets || {}) },
  };
}

function configuredPlatforms() {
  const config = state.platformConfig || {};
  const catalog = state.platformCatalog?.data || state.platformCatalog || {};
  const fullConfigs = config.platforms || config.platform || [];
  if (Array.isArray(fullConfigs) && fullConfigs.length) {
    return fullConfigs.map(normalizePlatform);
  }
  return (catalog.platforms || []).map(normalizePlatform);
}

function configuredPlatform(id) {
  return configuredPlatforms().find((platform) => platform.id === id) || null;
}

function normalizePlatform(platform = {}) {
  const type = platform.type || platform.platform_type || platform.platform || "";
  return {
    ...platform,
    id: platform.id || "",
    type,
    platform_type: type,
    enabled: platform.enabled ?? platform.enable ?? true,
    enable: platform.enable ?? platform.enabled ?? true,
    name: platform.name || "",
    options: platform.options || {},
    secrets: platform.secrets || {},
  };
}

function uniquePlatformId(baseId) {
  const existing = new Set(configuredPlatforms().map((platform) => platform.id));
  if (!existing.has(baseId)) return baseId;
  let counter = 1;
  let candidate = `${baseId}_${counter}`;
  while (existing.has(candidate)) {
    counter += 1;
    candidate = `${baseId}_${counter}`;
  }
  return candidate;
}

function captureVisiblePlatformDraft() {
  if ($("#platform-new-id")) {
    state.platformDraft = platformEditPayload("platform-new", state.platformDraft || {});
    state.platformSelectedConfigId = $("#platform-config-select")?.value || state.platformSelectedConfigId || "default";
    state.platformNewConfigName = $("#platform-new-config-name")?.value.trim() || state.platformNewConfigName || "";
    if ($("#platform-new-config-json")) {
      state.platformNewConfigDraft = parseJsonField("platform-new-config-json", {});
    }
    return;
  }
  if ($("#platform-edit-id")) {
    state.platformDraft = platformEditPayload("platform-edit", state.platformDraft || configuredPlatform(state.platformEditId) || {});
  }
}

function platformEditPayload(prefix, base = {}) {
  const enabled = Boolean($(`#${prefix}-enabled`)?.checked);
  return normalizePlatform({
    ...base,
    id: requiredValue(`${prefix}-id`, "Platform ID"),
    type: requiredValue(`${prefix}-type`, "Platform type"),
    name: emptyToNull($(`#${prefix}-name`)?.value || ""),
    enabled,
    enable: enabled,
    options: parseJsonField(`${prefix}-options-json`, {}),
    secrets: parseJsonField(`${prefix}-secrets-json`, {}),
  });
}

function validatePlatformPayload(platform) {
  if (!platform.id) throw new Error("Platform ID 不能为空");
  if (/[!:]/.test(platform.id)) throw new Error("Platform ID 不能包含 ! 或 :");
  if (!platform.type) throw new Error("Platform type 不能为空");
  if ((platform.type === "onebot" || platform.type === "aiocqhttp") && !String(platform.secrets?.ws_reverse_token || "").trim()) {
    showToast("OneBot 反向 WS token 为空，请确认部署环境安全。", "warn");
  }
}

async function bindPlatformConfig(platformId) {
  if (!state.platformShowConfigSection) return;
  let confId = $("#platform-config-select")?.value || state.platformSelectedConfigId || "default";
  if (state.platformConfigMode === "new") {
    const name = requiredValue("platform-new-config-name", "新配置名称");
    const config = parseJsonField("platform-new-config-json", {});
    const result = await api("/api/config/abconf/new", {
      method: "POST",
      body: JSON.stringify({ name, config }),
    });
    confId = sourceData(result).conf_id;
  }
  if (!confId) throw new Error("配置文件 ID 不能为空");
  await api("/api/config/umo_abconf_route/update", {
    method: "POST",
    body: JSON.stringify({ umo: `${platformId}:*:*`, conf_id: confId }),
  });
}

function normalizedPlatformRoutes() {
  const routes = state.platformRoutes || {};
  if (Array.isArray(routes.routes)) return routes.routes;
  const routing = routes.routing || {};
  return Object.entries(routing).map(([pattern, config_id]) => ({ pattern, config_id }));
}

function routeDraftsForPlatform(platformId) {
  const drafts = normalizedPlatformRoutes()
    .filter((route) => isUmoMatchPlatform(route.pattern || route.umo, platformId))
    .map((route) => routeDraftFromRoute(route));
  return drafts.length ? drafts : [{ messageType: "*", sessionId: "*", configId: "default" }];
}

function routeDraftFromRoute(route) {
  const pattern = route.pattern || route.umo || "";
  const parts = pattern.split(":");
  return {
    originalUmop: pattern,
    messageType: parts[1] || "*",
    sessionId: parts[2] || "*",
    configId: route.config_id || route.conf_id || "default",
  };
}

function readPlatformRouteDrafts() {
  const existing = state.platformRouteDrafts?.length ? state.platformRouteDrafts : [{ messageType: "*", sessionId: "*", configId: "default" }];
  return existing.map((route, index) => ({
    ...route,
    messageType: $(`#platform-route-${index}-message-type`)?.value || route.messageType || "*",
    sessionId: $(`#platform-route-${index}-session-id`)?.value.trim() || route.sessionId || "*",
    configId: $(`#platform-route-${index}-config-id`)?.value || route.configId || "default",
  }));
}

async function savePlatformRoutesInternal(originalPlatformId, newPlatformId) {
  const latest = await api("/api/config/umo_abconf_routes");
  const routing = { ...(sourceData(latest).routing || {}) };
  for (const umo of Object.keys(routing)) {
    if (isUmoMatchPlatform(umo, originalPlatformId) || isUmoMatchPlatform(umo, newPlatformId)) {
      delete routing[umo];
    }
  }
  for (const route of readPlatformRouteDrafts()) {
    if (!route.configId) continue;
    const messageType = route.messageType || "*";
    const sessionId = route.sessionId || "*";
    routing[`${newPlatformId}:${messageType}:${sessionId}`] = route.configId;
  }
  await api("/api/config/umo_abconf_route/update_all", {
    method: "POST",
    body: JSON.stringify({ routing }),
  });
}

function isUmoMatchPlatform(umo, platformId) {
  const parts = String(umo || "").split(":");
  if (parts.length !== 3) return false;
  return parts[0] === platformId || parts[0] === "*" || parts[0] === "";
}

function recordPlatformCheck(platformId, status) {
  state.platformChecks = {
    ...(state.platformChecks || {}),
    [platformId]: status,
  };
}

function providerFormPayload() {
  return {
    id: $("#provider-id").value.trim(),
    type: $("#provider-type").value,
    enabled: $("#provider-enabled").checked,
    model: emptyToNull($("#provider-model").value),
    api_base: emptyToNull($("#provider-api-base").value),
    api_key: emptyToNull($("#provider-api-key").value),
    timeout_secs: Math.max(1, Number($("#provider-timeout").value || 120)),
    mock_response: emptyToNull($("#provider-mock-response").value),
  };
}

function platformFormPayload() {
  return {
    id: $("#platform-id").value.trim(),
    type: $("#platform-type").value,
    enabled: $("#platform-enabled").checked,
    name: emptyToNull($("#platform-name").value),
  };
}

function commandFormPayload() {
  return {
    plugin_name: $("#command-plugin").value.trim(),
    handler_name: $("#command-handler").value.trim(),
    command: $("#command-command").value.trim(),
    response: $("#command-response").value,
    priority: Number($("#command-priority").value || 0),
    enabled: $("#command-enabled").checked,
    permission: $("#command-permission").value,
  };
}

function mcpFormPayload() {
  const transport = $("#mcp-transport").value;
  const args = $("#mcp-args").value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  const server = {
    active: $("#mcp-active").checked,
    transport,
    args,
    sessionReadTimeoutSeconds: Math.max(1, Number($("#mcp-timeout").value || 60)),
    clientCapabilities: {
      elicitation: { enabled: $("#mcp-elicitation").checked, timeoutSeconds: 300 },
      sampling: { enabled: $("#mcp-sampling").checked },
      roots: { paths: [] },
    },
  };
  const command = emptyToNull($("#mcp-command").value);
  const url = emptyToNull($("#mcp-url").value);
  if (command) server.command = command;
  if (url) server.url = url;
  return {
    name: $("#mcp-name").value.trim(),
    server,
  };
}

function pluginSourceFormPayload() {
  return {
    plugin_id: $("#plugin-source-id").value.trim(),
    kind: $("#plugin-source-kind").value,
    root_dir: emptyToNull($("#plugin-source-root").value),
    module_path: emptyToNull($("#plugin-source-module").value),
    reserved: $("#plugin-source-reserved").checked,
  };
}

function pluginConfigFilePayload() {
  return {
    plugin_id: $("#plugin-config-id").value.trim(),
    filename: $("#plugin-config-filename").value.trim() || "config.json",
  };
}

function emptyToNull(value) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}
