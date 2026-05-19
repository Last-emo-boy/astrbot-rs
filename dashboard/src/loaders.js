import { api, openApi, safeApi } from "./api.js";
import { setConnection } from "./dom.js";
import { state } from "./state.js";
export async function loadCore() {
  try {
    const [status, capabilities, stats] = await Promise.all([
      api("/api/management/status"),
      api("/api/management/dashboard/capabilities"),
      safeApi("/api/management/stats", {
        total_messages: 0,
        total_llm_calls: 0,
        total_tokens: 0,
        total_tts_events: 0,
        uptime_seconds: 0,
        platform_counts: [],
        provider_usage: [],
        recent_events: [],
      }),
    ]);
    state.status = status;
    state.capabilities = capabilities;
    state.stats = stats;
    setConnection("ok", "已连接");
  } catch (error) {
    setConnection("error", "连接失败");
    throw error;
  }
}

export async function loadConfig() {
  state.configMode = state.routeFragment === "system" ? "system" : "normal";
  const [schema, abconfs, routes, t2iTemplates, t2iActive] = await Promise.all([
    api("/api/management/config/schema"),
    safeApi("/api/management/config/abconfs", { info_list: [{ id: "default", name: "default" }] }),
    safeApi("/api/management/config/routes", { routes: [] }),
    safeApi("/api/t2i/templates", { status: "ok", data: [{ name: "base", is_default: true }] }),
    safeApi("/api/t2i/templates/active", { status: "ok", data: { active_template: "base" } }),
  ]);
  state.schema = schema;
  state.configAbconfs = abconfs;
  state.configRoutes = routes;
  state.t2iTemplates = t2iTemplates;
  state.t2iActiveTemplate = t2iActive;

  const availableIds = (abconfs.info_list || []).map((item) => item.id);
  if (!availableIds.includes(state.selectedConfigId)) {
    state.selectedConfigId = availableIds[0] || "default";
  }

  const selectedId = state.configMode === "system" ? "default" : state.selectedConfigId || "default";
  const current = selectedId === "default"
    ? await api("/api/management/config/current")
    : await api("/api/management/config/abconfs/get", {
      method: "POST",
      body: JSON.stringify({ id: selectedId }),
    });
  state.config = current.config || current.abconf?.config || {};
  state.configEditor = JSON.stringify(state.config, null, 2);
  state.configLastSavedSnapshot = state.configEditor;
  state.configDirty = false;

  const activeTemplate = t2iActive?.data?.active_template || t2iActive?.active_template || "base";
  state.t2iSelectedTemplate = state.t2iSelectedTemplate || activeTemplate;
  const selectedTemplate = state.t2iSelectedTemplate || activeTemplate;
  const template = await safeApi(`/api/t2i/templates/${encodeURIComponent(selectedTemplate)}`, {
    status: "ok",
    data: { name: selectedTemplate, content: "" },
  });
  state.t2iTemplateContent = template?.data?.content || "";
}

export async function loadProviderCatalog() {
  const legacy = await safeApi("/api/config/provider/template", {
    status: "ok",
    data: {
      config_schema: { provider: { config_template: {} } },
      provider_sources: [],
      providers: [],
    },
  });
  state.providerCatalog = legacy.data || legacy;
  const providers = state.providerCatalog?.providers || [];
  const firstChatProvider = providers.find((provider) => provider.provider_type === "chat_completion" && provider.enable !== false);
  if (!state.chat.selectedProviderId && firstChatProvider?.id) {
    state.chat.selectedProviderId = firstChatProvider.id;
    state.chat.selectedModel = firstChatProvider.model || "";
  }
}

export async function loadPlatformCatalog() {
  const [catalog, config, abconfs, routes, runtimeStats] = await Promise.all([
    safeApi("/api/management/platforms/catalog", {
      platforms: [],
      templates: [],
    }),
    safeApi("/api/config/get", {
      status: "ok",
      data: { config: { platforms: [] }, metadata: {} },
    }),
    safeApi("/api/config/abconfs", {
      status: "ok",
      data: { info_list: [{ id: "default", name: "default" }] },
    }),
    safeApi("/api/config/umo_abconf_routes", {
      status: "ok",
      data: { routing: {}, routes: [] },
    }),
    safeApi("/api/platform/stats", {
      status: "ok",
      data: { platforms: [] },
    }),
  ]);
  state.platformCatalog = catalog?.data || catalog;
  state.platformConfig = config?.data?.config || config?.config || {};
  state.platformMetadata = config?.data?.metadata || config?.metadata || {};
  state.platformAbconfs = abconfs?.data || abconfs;
  state.platformRoutes = routes?.data || routes;
  state.platformRuntimeStats = runtimeStats?.data || runtimeStats;
}

export async function loadKnowledge() {
  state.kb = await safeApi("/api/management/kb/catalog", { knowledge_bases: [] });
  const kbId = state.routeParams?.kbId;
  const docId = state.routeParams?.docId;
  if (kbId) {
    await loadKnowledgeDetail(kbId);
    await loadKnowledgeDocuments(kbId);
  } else {
    state.kbDetail = null;
    state.kbDocuments = null;
  }
  if (docId) {
    await loadKnowledgeDocument(docId);
    await loadKnowledgeChunks(docId);
  } else {
    state.kbDocumentDetail = null;
    state.kbChunks = null;
  }
}

export async function loadKnowledgeDetail(kbId) {
  state.kbDetail = await safeApi(
    "/api/management/kb/get",
    { knowledge_base: null },
    { method: "POST", body: JSON.stringify({ kb_id: kbId }) },
  );
}

export async function loadKnowledgeDocuments(kbId) {
  state.kbDocuments = await safeApi(
    "/api/management/kb/document/list",
    { documents: [] },
    { method: "POST", body: JSON.stringify({ kb_id: kbId }) },
  );
}

export async function loadKnowledgeDocument(docId) {
  state.kbDocumentDetail = await safeApi(
    "/api/management/kb/document/get",
    null,
    { method: "POST", body: JSON.stringify({ doc_id: docId }) },
  );
}

export async function loadKnowledgeChunks(docId) {
  state.kbChunks = await safeApi(
    "/api/management/kb/chunk/list",
    { chunks: [] },
    { method: "POST", body: JSON.stringify({ doc_id: docId }) },
  );
}

export async function loadKnowledgeUploadTask(taskId) {
  state.kbUploadTask = await safeApi(`/api/management/kb/upload/progress/${encodeURIComponent(taskId)}`, {
    task: null,
  });
}

export async function loadTools() {
  state.tools = await safeApi("/api/management/tools", { tools: [] });
}

export async function loadCommands() {
  state.commands = await safeApi("/api/management/commands", { commands: [], conflicts: [] });
}

export async function loadMcp() {
  state.mcp = await safeApi("/api/management/mcp/servers", { servers: [], active_count: 0 });
}

export async function loadSessions() {
  const params = new URLSearchParams({
    page: String(state.sessionPage || 1),
    page_size: String(state.sessionPageSize || 10),
  });
  if (state.sessionFilter) params.set("search", state.sessionFilter);
  const [rules, groups, activeUmos] = await Promise.all([
    safeApi(`/api/session/list-rule?${params.toString()}`, {
      status: "ok",
      data: { rules: [], total: 0, page: 1, page_size: 10, available_rule_keys: [] },
    }),
    safeApi("/api/session/groups", { status: "ok", data: { groups: [] } }),
    safeApi("/api/session/active-umos", { status: "ok", data: { umos: [] } }),
  ]);
  state.sessions = rules;
  state.sessionGroups = groups;
  state.sessionAvailableUmos = activeUmos?.data?.umos || activeUmos?.umos || [];
  if (!state.activeUmo && state.sessionAvailableUmos.length) {
    state.activeUmo = state.sessionAvailableUmos[0];
  }
}

export async function loadProjects(actor = "user") {
  state.projects = await safeApi(
    "/api/management/chat-projects",
    { projects: [] },
    { method: "POST", body: JSON.stringify({ actor }) },
  );
}

export async function loadConversations(platformId = "", options = {}) {
  const filters = {
    ...(state.conversationFilters || {}),
    ...options,
  };
  if (platformId && !filters.platforms?.length) {
    filters.platforms = [platformId];
  }
  const params = new URLSearchParams({
    page: String(filters.page || 1),
    page_size: String(filters.pageSize || filters.page_size || 20),
  });
  if (filters.platforms?.length) params.set("platforms", filters.platforms.join(","));
  if (filters.messageTypes?.length) params.set("message_types", filters.messageTypes.join(","));
  if (filters.search) params.set("search", filters.search);
  state.conversationFilters = {
    platforms: filters.platforms || [],
    messageTypes: filters.messageTypes || [],
    search: filters.search || "",
    page: Number(filters.page || 1),
    pageSize: Number(filters.pageSize || filters.page_size || 20),
  };
  state.conversations = await safeApi(`/api/conversation/list?${params.toString()}`, {
    status: "ok",
    data: {
      conversations: [],
      pagination: { page: state.conversationFilters.page, page_size: state.conversationFilters.pageSize, total: 0, total_pages: 1 },
    },
  });
}

export async function loadChatControls(actor = "user", platformId = "webchat") {
  const [chatSessions, conversations, projects, providerCatalog, providerList, configOptions, configRoutes] = await Promise.all([
    safeApi("/api/chat/sessions", { status: "ok", data: [] }),
    safeApi(
      "/api/management/conversations",
      { conversations: [] },
      { method: "POST", body: JSON.stringify({ platform_id: platformId }) },
    ),
    safeApi(
      "/api/management/chat-projects",
      { projects: [] },
      { method: "POST", body: JSON.stringify({ actor }) },
    ),
    safeApi("/api/config/provider/template", {
      status: "ok",
      data: {
        config_schema: { provider: { config_template: {} } },
        provider_sources: [],
        providers: [],
      },
    }),
    safeApi("/api/config/provider/list?provider_type=chat_completion", { status: "ok", data: [] }),
    safeApi("/api/management/config/abconfs", { info_list: [{ id: "default", name: "default" }] }),
    safeApi("/api/config/umo_abconf_routes", { status: "ok", data: { routing: {}, routes: [] } }),
  ]);
  state.chatSessions = chatSessions;
  state.conversations = conversations;
  state.projects = projects;
  state.providerCatalog = providerCatalog?.data || providerCatalog;
  state.chatProviderList = providerList?.data || providerList;
  state.chatConfigOptions = configOptions;
  state.configAbconfs = configOptions?.data || configOptions;
  state.configRoutes = configRoutes?.data || configRoutes;
  applyChatConfigRoute(state.configRoutes, state.chat.conversationId);

  const providers = Array.isArray(state.chatProviderList)
    ? state.chatProviderList
    : state.providerCatalog?.providers || [];
  const selected = providers.find((provider) => provider.id === state.chat.selectedProviderId);
  const firstChatProvider = providers.find((provider) => provider.provider_type === "chat_completion" && provider.enable !== false);
  const active = selected || firstChatProvider;
  if (active) {
    state.chat.selectedProviderId = active.id || state.chat.selectedProviderId;
    state.chat.selectedModel = active.model || state.chat.selectedModel || "";
    state.chat.selectedModelName = active.model || state.chat.selectedModelName || "";
  }
}

export async function loadOpenApiRealtime() {
  try {
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
  } catch (error) {
    state.realtime = {
      ...(state.realtime || {}),
      subscriptions: state.realtime?.subscriptions || [],
      elicitations: state.realtime?.elicitations || [],
      unavailable: error.message,
    };
  }
}

function applyChatConfigRoute(routes, conversationId) {
  if (!conversationId) return;
  const routing = routes?.routing || routes?.data?.routing || {};
  const routeRows = Array.isArray(routes?.routes) ? routes.routes : Array.isArray(routes?.data?.routes) ? routes.data.routes : [];
  const candidates = [
    `webchat:FriendMessage:webchat!dashboard!${conversationId}`,
    `webchat:FriendMessage:${conversationId}`,
    `webchat:*:webchat!dashboard!${conversationId}`,
    `webchat:*:${conversationId}`,
  ];

  let resolved = candidates.map((key) => routing[key]).find(Boolean);
  if (!resolved) {
    const matchedRow = routeRows.find((row) => candidates.includes(row.umo || row.pattern || row.key || ""));
    resolved = matchedRow?.conf_id || matchedRow?.config_id || matchedRow?.configId;
  }
  if (!resolved) return;

  const availableIds = configInfoList(state.chatConfigOptions).map((item) => item.id);
  if (availableIds.length && !availableIds.includes(resolved)) return;
  state.chat.configId = resolved;
  state.chat.selectedConfigId = resolved;
}

function configInfoList(configOptions) {
  if (Array.isArray(configOptions?.info_list)) return configOptions.info_list;
  if (Array.isArray(configOptions?.data?.info_list)) return configOptions.data.info_list;
  if (Array.isArray(configOptions)) return configOptions;
  return [];
}

export async function loadProjectSessions(actor, projectId) {
  state.projectSessions = await safeApi(
    "/api/management/chat-projects/sessions",
    { sessions: [] },
    { method: "POST", body: JSON.stringify({ actor, project_id: projectId }) },
  );
}

export async function loadSkills() {
  state.skills = await safeApi("/api/management/skills", { skills: [], sandbox_cache: null });
}

export async function loadSkillsNeo() {
  const [candidates, releases] = await Promise.all([
    safeApi("/api/skills/neo/candidates", { status: "unavailable", data: [], candidates: [] }),
    safeApi("/api/skills/neo/releases", { status: "unavailable", data: [], releases: [] }),
  ]);
  state.skillsNeo = { candidates, releases };
}

export async function loadMarket() {
  state.market = await safeApi("/api/management/plugin-market", { plugins: [] });
}

export async function loadPluginLifecycle() {
  state.pluginLifecycle = await safeApi("/api/management/plugins/lifecycle", {
    handlers: { handlers: [], handler_count: 0 },
    plugins: [],
    operations: [],
  });
}

export async function loadUpdate() {
  const [update, releases, changelog, migration] = await Promise.all([
    safeApi("/api/management/update/check", { check: null }),
    safeApi("/api/management/update/releases", { releases: [] }),
    safeApi("/api/management/update/changelog", { releases: [] }),
    safeApi("/api/management/update/migration-check", { check: null }),
  ]);
  state.update = update;
  state.releases = releases;
  state.changelog = changelog;
  state.migration = migration;
}

export async function loadBackup() {
  state.backupFiles = await safeApi("/api/management/backup/files", { files: [] });
  if (!state.backupTask) {
    state.backupTask = null;
  }
}

export async function loadLogs() {
  const [managementLogs, legacyHistory] = await Promise.all([
    safeApi("/api/management/logs?limit=100", { snapshot: { entries: [] } }),
    safeApi("/api/log-history", { status: "ok", data: { logs: [] } }),
  ]);
  const rows = sourceLogHistoryRows(legacyHistory);
  state.logs = {
    ...managementLogs,
    source_logs: rows.filter((row) => row.type !== "trace"),
    source_traces: rows.filter((row) => row.type === "trace"),
    unavailable: managementLogs?.unavailable || legacyHistory?.unavailable || "",
  };
}

export async function loadTrace() {
  const [managementTrace, legacyHistory] = await Promise.all([
    safeApi("/api/management/trace", { events: [] }),
    safeApi("/api/log-history", { status: "ok", data: { logs: [] } }),
  ]);
  state.trace = {
    ...managementTrace,
    source_events: sourceLogHistoryRows(legacyHistory).filter((row) => row.type === "trace"),
    unavailable: managementTrace?.unavailable || legacyHistory?.unavailable || "",
  };
  state.traceSettings = state.trace.settings || state.traceSettings;
}

export async function loadTraceSettings() {
  state.traceSettings = await safeApi("/api/management/trace/settings", {
    enabled: true,
    capture_message_outline: true,
    max_events: 500,
    redact_fields: [],
  });
}

export async function loadPersonas(folderId = state.personaFolderId) {
  state.personaFolderId = folderId || null;
  const [personasResponse, folderTreeResponse] = await Promise.all([
    safeApi("/api/persona/list", { status: "ok", data: [] }),
    safeApi("/api/persona/folder/tree", { status: "ok", data: [] }),
  ]);
  const personas = sourcePayloadArray(personasResponse);
  const folderTree = sourcePayloadArray(folderTreeResponse);
  state.personaFolderTree = folderTree;
  state.personas = {
    personas,
    folders: flattenPersonaFolders(folderTree),
    unavailable: personasResponse.unavailable || folderTreeResponse.unavailable || null,
  };
}

export async function loadCron() {
  const [sourceJobs, managementCron, platformStats] = await Promise.all([
    safeApi("/api/cron/jobs", { status: "ok", data: [] }),
    safeApi(
      "/api/management/cron/jobs",
      { state: "unavailable", jobs: [], scheduled_jobs: [] },
      { method: "POST", body: JSON.stringify({}) },
    ),
    safeApi("/api/platform/stats", {
      status: "ok",
      data: { platforms: [] },
    }),
  ]);
  const platforms = platformStats?.data?.platforms || platformStats?.platforms || [];
  state.cron = {
    status: sourceJobs?.status || "ok",
    message: sourceJobs?.message || "",
    unavailable: sourceJobs?.unavailable || managementCron?.unavailable || platformStats?.unavailable || "",
    state: managementCron?.state || "unknown",
    jobs: sourcePayloadArray(sourceJobs).map(normalizeCronJob),
    scheduled_jobs: managementCron?.scheduled_jobs || [],
    proactive_platforms: platforms
      .filter((platform) => platform?.meta?.support_proactive_message)
      .map((platform) => ({
        id: platform.id || platform?.meta?.id || "unknown",
        name: platform?.meta?.name || platform.type || "",
        display_name: platform?.meta?.display_name || platform.display_name || platform.name || "",
      })),
  };
}

export async function loadSubagents() {
  const [sourceConfig, catalog, availableTools, providers, personas] = await Promise.all([
    safeApi("/api/subagent/config", {
      status: "ok",
      data: defaultSubagentConfig(),
    }),
    safeApi("/api/management/subagents", {
      main_enable: false,
      remove_main_duplicate_tools: false,
      agents: [],
      handoffs: [],
      executions: [],
    }),
    safeApi("/api/subagent/available-tools", { status: "ok", data: [] }),
    safeApi("/api/config/provider/list?provider_type=chat_completion", { status: "ok", data: [] }),
    safeApi("/api/persona/list", { status: "ok", data: [] }),
  ]);
  const config = normalizeSubagentConfig(sourceConfig?.data || sourceConfig);
  state.subagents = {
    ...catalog,
    ...config,
    agents: config.agents,
    handoffs: catalog?.handoffs || [],
    executions: catalog?.executions || [],
    available_tools: sourcePayloadArray(availableTools).map(normalizeAvailableTool),
    providers: sourcePayloadArray(providers),
    personas: sourcePayloadArray(personas),
    unavailable: sourceConfig?.unavailable || catalog?.unavailable || availableTools?.unavailable || "",
  };
}

export async function loadApiKeys() {
  state.apiKeys = await safeApi("/api/management/api-keys", { api_keys: [] });
}

export async function loadMessages() {
  if (!state.chat.conversationId) {
    state.messages = [];
    return;
  }
  const [webchatMessages, sourceSession] = await Promise.all([
    safeApi(`/api/webchat/${encodeURIComponent(state.chat.conversationId)}/messages`, { messages: [] }),
    safeApi(`/api/chat/get_session?session_id=${encodeURIComponent(state.chat.conversationId)}`, { status: "ok", data: { history: [], project: null } }),
  ]);
  const sourceData = sourceSession?.data || sourceSession;
  const webchatRows = Array.isArray(webchatMessages.messages) ? webchatMessages.messages : [];
  const sourceRows = Array.isArray(sourceData.history) ? sourceData.history : [];
  state.messages = webchatRows.length ? webchatRows : sourceRows;
  if (sourceData.project) {
    state.chat.currentSessionProject = sourceData.project;
  }
}

export async function routeBeforeRender(routeId) {
  if (routeId === "config") await loadConfig();
  if (routeId === "providers") await loadProviderCatalog();
  if (routeId === "platforms") await loadPlatformCatalog();
  if (routeId === "knowledge") await loadKnowledge();
  if (routeId === "tools") await Promise.all([loadTools(), loadCommands(), loadMcp()]);
  if (routeId === "sessions") await loadSessions();
  if (routeId === "projects") await loadProjects();
  if (routeId === "chat") await Promise.all([loadMessages(), loadChatControls()]);
  if (routeId === "chatbox") await Promise.all([loadMessages(), loadChatControls(), loadOpenApiRealtime()]);
  if (routeId === "conversation") await Promise.all([loadMessages(), loadConversations()]);
  if (routeId === "skills") await Promise.all([loadSkills(), loadSkillsNeo()]);
  if (routeId === "plugins") await Promise.all([loadPluginLifecycle(), loadMarket()]);
  if (routeId === "market") await Promise.all([loadMarket(), loadPluginLifecycle()]);
  if (routeId === "update") await loadUpdate();
  if (routeId === "about") await loadUpdate();
  if (routeId === "backup") await loadBackup();
  if (routeId === "console") await loadLogs();
  if (routeId === "trace") await Promise.all([loadTrace(), loadTraceSettings()]);
  if (routeId === "personas") await Promise.all([loadPersonas(), loadTools(), loadSkills(), loadMcp()]);
  if (routeId === "cron") await loadCron();
  if (routeId === "subagent") await loadSubagents();
  if (routeId === "settings") await Promise.all([loadApiKeys(), loadUpdate(), loadBackup()]);
}

function sourcePayloadArray(response) {
  if (Array.isArray(response)) return response;
  if (Array.isArray(response?.data)) return response.data;
  if (Array.isArray(response?.personas)) return response.personas;
  if (Array.isArray(response?.folders)) return response.folders;
  return [];
}

function sourceLogHistoryRows(response) {
  if (Array.isArray(response)) return response;
  if (Array.isArray(response?.logs)) return response.logs;
  if (Array.isArray(response?.data?.logs)) return response.data.logs;
  return [];
}

function normalizeCronJob(job = {}) {
  const payload = job.payload || {};
  const spec = job.schedule?.spec || {};
  const cronSpec = spec.cron || spec.Cron || {};
  const runOnceSpec = spec.run_once || spec.RunOnce || {};
  const runOnce = Boolean(job.run_once || runOnceSpec.run_at);
  return {
    ...job,
    job_id: job.job_id || job.id || "",
    name: job.name || "active_agent_task",
    job_type: job.job_type || job.type || job.kind || "active_agent",
    cron_expression: job.cron_expression || cronSpec.expression || "",
    timezone: job.timezone || job.schedule?.timezone || "",
    session: job.session || payload.session || "",
    note: job.note || payload.note || job.description || "",
    run_once: runOnce,
    run_at: job.run_at || payload.run_at || runOnceSpec.run_at || "",
    enabled: job.enabled !== false,
    persistent: job.persistent !== false,
    next_run_time: job.next_run_time || "",
    last_run_at: job.last_run_at || "",
  };
}

function defaultSubagentConfig() {
  return {
    main_enable: false,
    remove_main_duplicate_tools: false,
    router_system_prompt: "",
    agents: [],
  };
}

function normalizeSubagentConfig(config = {}) {
  return {
    main_enable: Boolean(config.main_enable || config.enable),
    remove_main_duplicate_tools: Boolean(config.remove_main_duplicate_tools),
    router_system_prompt: String(config.router_system_prompt || ""),
    agents: (Array.isArray(config.agents) ? config.agents : []).map(normalizeSubagentAgent),
  };
}

function normalizeSubagentAgent(agent = {}) {
  return {
    name: String(agent.name || ""),
    enabled: agent.enabled !== false,
    persona_id: String(agent.persona_id || ""),
    provider_id: String(agent.provider_id || ""),
    system_prompt: String(agent.system_prompt || ""),
    public_description: String(agent.public_description || agent.description || ""),
    tools: Array.isArray(agent.tools) ? agent.tools.map(String).filter(Boolean) : null,
  };
}

function normalizeAvailableTool(tool = {}) {
  return {
    name: String(tool.name || ""),
    description: String(tool.description || ""),
    parameters: tool.parameters || {},
    active: tool.active !== false,
    handler_module_path: String(tool.handler_module_path || tool.origin_name || tool.origin || ""),
  };
}

function flattenPersonaFolders(nodes = []) {
  const flattened = [];
  const visit = (items) => {
    for (const item of items || []) {
      const { children, ...folder } = item || {};
      flattened.push(folder);
      visit(children || []);
    }
  };
  visit(nodes);
  return flattened;
}
