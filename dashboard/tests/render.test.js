import assert from "node:assert/strict";
import test from "node:test";

const storage = new Map();

globalThis.window = {
  location: { hash: "" },
  localStorage: {
    getItem(key) {
      return storage.get(key) || null;
    },
    setItem(key, value) {
      storage.set(key, String(value));
    },
    removeItem(key) {
      storage.delete(key);
    },
  },
};

const classes = new Set();
globalThis.document = {
  documentElement: {
    style: {
      values: new Map(),
      setProperty(name, value) {
        this.values.set(name, value);
      },
      removeProperty(name) {
        this.values.delete(name);
      },
    },
  },
  body: {
    dataset: {},
    classList: {
      toggle(name, enabled) {
        if (enabled) {
          classes.add(name);
        } else {
          classes.delete(name);
        }
      },
      contains(name) {
        return classes.has(name);
      },
    },
  },
  querySelector() {
    return null;
  },
};

const { renderOverview } = await import("../src/render/overview.js");
const { renderKnowledge } = await import("../src/render/knowledge.js");
const { renderChat, renderChatBox, renderConversation, renderProjects, renderSessions } = await import("../src/render/data.js");
const { renderConfig } = await import("../src/render/config.js");
const { renderAbout, renderSettings } = await import("../src/render/settings.js");
const { renderMarket, renderPlatforms, renderPlugins, renderProviders, renderSkills, renderTools } = await import("../src/render/integrations.js");
const { renderConsole, renderCron, renderPersonas, renderTrace } = await import("../src/render/operations.js");
const { renderSubAgent } = await import("../src/render/subagents.js");
const {
  confirmDialog,
  dataTable,
  formField,
  markdownViewer,
  renderUiBaseShowcase,
  tabs,
  unsavedChangesDialog,
} = await import("../src/render/shared.js");
const { state } = await import("../src/state.js");
const {
  apiBase,
  apiUrl,
  desktopBridgeSnapshot,
  dashboardPreferences,
  probeDesktopBridge,
  setApiBase,
  setDashboardPreferences,
} = await import("../src/api.js");
const { locale, setLocale, t, validateI18nDictionaries } = await import("../src/i18n.js");
const { localizedRoutes } = await import("../src/routes.js");

test("overview render escapes dynamic service and metric content", () => {
  state.routeSourcePath = "/welcome";
  state.status = {
    providers: {
      chat_provider_count: 1,
      embedding_provider_count: 1,
      rerank_provider_count: 0,
      default_chat_provider_id: "<script>alert(1)</script>",
    },
    platforms: {
      platform_count: 1,
      platform_ids: ["webchat"],
      webchat_platform_count: 1,
    },
    plugins: { handler_count: 2 },
  };
  state.stats = {
    total_messages: 3,
    total_llm_calls: 1,
    total_tokens: 12,
    total_tts_events: 0,
    uptime_seconds: 75,
    log_count: 4,
    trace_count: 2,
    platform_counts: [{ platform_id: "webchat", platform_type: "webchat", count: 3 }],
    provider_usage: [{ provider_id: "mock", calls: 1, total_tokens: 12 }],
  };
  state.capabilities = {
    services: [
      {
        label: "<b>Status</b>",
        configured: true,
        closure_level: "runtime",
        api_base: "/api/management/status",
        notes: ["safe <note>"],
      },
      {
        label: "Plugin Market",
        configured: true,
        closure_level: "plan_only",
        api_base: "/api/management/plugin-market",
        notes: [],
      },
    ],
  };

  const html = renderOverview();

  assert.match(html, /1 runtime/);
  assert.match(html, /1 plan-only/);
  assert.match(html, /1m 15s/);
  assert.match(html, /bar-list/);
  assert.match(html, /bar-fill/);
  assert.match(html, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
  assert.match(html, /&lt;b&gt;Status&lt;\/b&gt;/);
  assert.doesNotMatch(html, /<script>alert/);
});

test("welcome render exposes onboarding resources announcement and capability section", () => {
  state.routeSourcePath = "/welcome";
  state.status = {
    providers: {
      chat_provider_count: 1,
      embedding_provider_count: 0,
      rerank_provider_count: 0,
      default_chat_provider_id: "openai",
    },
    platforms: {
      platform_count: 1,
      platform_ids: ["webchat"],
      webchat_platform_count: 1,
    },
    plugins: { handler_count: 3 },
  };
  state.stats = { uptime_seconds: 3605, platform_counts: [], provider_usage: [] };
  state.capabilities = { services: [] };
  state.welcomeAnnouncement = "# Notice\n- Use `safe` mode";

  const html = renderOverview();

  assert.match(html, /data-page="welcome"/);
  assert.match(html, /Getting Started/);
  assert.match(html, /Backend URL/);
  assert.match(html, /Resources/);
  assert.match(html, /language-text|markdown-body/);
  assert.match(html, /业务闭环看板/);
});

test("default dashboard render mirrors source stat cards and chart sections", () => {
  state.routeSourcePath = "/dashboard/default";
  state.status = {
    providers: {
      chat_provider_count: 1,
      embedding_provider_count: 1,
      rerank_provider_count: 0,
      default_chat_provider_id: "openai",
    },
    platforms: {
      platform_count: 2,
      platform_ids: ["webchat", "telegram"],
      webchat_platform_count: 1,
    },
    plugins: { handler_count: 4 },
  };
  state.stats = {
    generated_at_unix: 1779105600,
    uptime_seconds: 3661,
    total_messages: 8,
    platform_counts: [
      { platform_id: "webchat", platform_type: "webchat", count: 5 },
      { platform_id: "telegram", platform_type: "telegram", count: 3 },
    ],
    recent_events: [
      { kind: "platform_message", timestamp: "2026-05-18T10:00:00Z", count: 2 },
      { kind: "platform_message", timestamp: "2026-05-18T11:00:00Z", count: 6 },
    ],
  };
  state.dashboardStats = {
    memory: { process: 128, system: 4096 },
    cpu_percent: 12,
    daily_increase: 2,
  };
  state.dashboardNotice = { title: "Dashboard notice", content: "<safe>", type: "info" };

  const html = renderOverview();

  assert.match(html, /data-page="default-dashboard"/);
  assert.match(html, /Total Messages/);
  assert.match(html, /Online Platforms/);
  assert.match(html, /Running Time/);
  assert.match(html, /Memory Usage/);
  assert.match(html, /Message Trend/);
  assert.match(html, /Platform Stat/);
  assert.match(html, /Dashboard notice/);
  assert.match(html, /&lt;safe&gt;/);
  assert.doesNotMatch(html, /<safe>/);
});

test("knowledge render exposes retrieval controls and escapes results", () => {
  state.routeSourcePath = "/knowledge-base";
  state.routeParams = {};
  state.kb = {
    knowledge_bases: [
      {
        kb_id: "kb-1",
        name: "Docs",
        description: "Project docs",
        emoji: "books",
        embedding_provider_id: "embedding",
        rerank_provider_id: null,
        chunk_size: 256,
        chunk_overlap: 32,
        stats: { doc_count: 1, chunk_count: 1 },
      },
    ],
  };
  state.kbDetail = null;
  state.kbDocuments = {
    documents: [
      {
        doc_id: "doc-1",
        kb_id: "kb-1",
        name: "Intro",
        file_type: "markdown",
        file_size: 2048,
        chunk_count: 1,
      },
    ],
  };
  state.kbChunks = {
    chunks: [
      {
        chunk_id: "chunk-1",
        doc_id: "doc-1",
        kb_id: "kb-1",
        chunk_index: 0,
        char_count: 32,
        content: "<img src=x onerror=alert(1)>",
      },
    ],
  };
  state.kbRetrieval = {
    results: [
      {
        chunk_id: "chunk-1",
        kb_id: "kb-1",
        doc_id: "doc-1",
        doc_name: "Intro",
        score: 2,
        content: "<img src=x onerror=alert(1)>",
      },
    ],
  };

  const listHtml = renderKnowledge();
  assert.match(listHtml, /data-page="knowledge-list"/);
  assert.match(listHtml, /Docs/);
  assert.match(listHtml, /Project docs/);

  state.routeParams = { kbId: "kb-1" };
  state.kbDetail = { knowledge_base: state.kb.knowledge_bases[0] };
  state.kbActiveTab = "retrieval";

  const html = renderKnowledge();

  assert.match(html, /data-page="knowledge-detail"/);
  assert.match(html, /id="kb-query"/);
  assert.match(html, /id="kb-ingest-content"/);
  assert.match(html, /data-action="kb-ingest"/);
  assert.match(html, /data-action="kb-retrieve"/);
  assert.match(html, /chunk-1/);
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.doesNotMatch(html, /<img src=x/);

  state.kbActiveTab = "documents";
  const documentsHtml = renderKnowledge();
  assert.match(documentsHtml, /文档管理/);
  assert.match(documentsHtml, /data-action="kb-upload-dialog-open"/);
  assert.match(documentsHtml, /href="#\/knowledge-base\/kb-1\/document\/doc-1"/);

  state.routeParams = { kbId: "kb-1", docId: "doc-1" };
  state.kbDocumentDetail = state.kbDocuments.documents[0];
  const documentHtml = renderKnowledge();
  assert.match(documentHtml, /data-page="knowledge-document-detail"/);
  assert.match(documentHtml, /Intro/);
  assert.match(documentHtml, /Chunks/);
  assert.match(documentHtml, /data-action="kb-chunk-view-open"/);
  assert.match(documentHtml, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.doesNotMatch(documentHtml, /<img src=x/);
});

test("config render exposes categorized metadata controls", () => {
  state.configMode = "system";
  state.configDirty = true;
  state.configEditorFullscreen = true;
  state.configUnsavedPrompt = { open: true, target: { mode: "normal", confId: "ops" } };
  state.selectedConfigId = "ops";
  state.configAbconfs = {
    info_list: [
      { id: "default", name: "default" },
      { id: "ops", name: "Ops <Config>" },
    ],
  };
  state.schema = {
    schema: {
      version: 1,
      fields: [
        { path: "webchat_server.enabled", value_type: "bool", default_value: false, secret: false },
        { path: "webchat_server.host", value_type: "string", default_value: "127.0.0.1", secret: false },
        { path: "webchat_server.port", value_type: "integer", default_value: 6185, secret: false },
        { path: "chat_providers", value_type: "list", default_value: [], secret: false },
        { path: "chat_providers[].api_key", value_type: "optional_string", default_value: null, secret: true },
      ],
    },
    ui_metadata: {
      groups: [
        {
          id: "webchat",
          title: "WebChat <Config>",
          fields: [
            { path: "webchat_server.enabled", control: "toggle", secret: false },
            { path: "webchat_server.host", control: "text", secret: false },
            { path: "webchat_server.port", control: "number", secret: false },
            { path: "t2i.template", label: "T2I template", control: "object", _special: "t2i_template" },
          ],
        },
        {
          id: "provider",
          title: "Providers",
          fields: [
            { path: "chat_providers", control: "list", secret: false },
            { path: "chat_providers[].api_key", control: "password", secret: true },
          ],
        },
      ],
    },
  };
  state.configEditor = JSON.stringify({
    webchat_server: { enabled: true, host: "<script>alert(1)</script>", port: 7001 },
    chat_providers: [{ id: "mock" }],
  });
  state.configRoutes = {
    routes: [{ pattern: "webchat:group:room-*", config_id: "room-config" }],
  };
  state.t2iTemplates = {
    status: "ok",
    data: [
      { name: "base", is_default: true, active: false },
      { name: "ops_card", is_default: false, active: true },
    ],
  };
  state.t2iActiveTemplate = { status: "ok", data: { active_template: "ops_card" } };
  state.t2iSelectedTemplate = "ops_card";
  state.t2iTemplateContent = "<main>{{ text | safe }} {{ version }}</main>";

  const html = renderConfig();

  assert.match(html, /data-page="config"/);
  assert.match(html, /data-config-mode="system"/);
  assert.match(html, /data-action="config-mode-normal"/);
  assert.match(html, /data-action="config-mode-system"/);
  assert.match(html, /ABConf/);
  assert.match(html, /Ops &lt;Config&gt;/);
  assert.match(html, /Unsaved changes/);
  assert.match(html, /id="config-editor-dialog"/);
  assert.match(html, /id="config-unsaved-dialog"/);
  assert.match(html, /T2I Template Editor/);
  assert.match(html, /config-mode-switch/);
  assert.match(html, /data-action="sync-config-form"/);
  assert.match(html, /UMOP Routes/);
  assert.match(html, /data-action="config-route-upsert"/);
  assert.match(html, /id="config-routes-json"/);
  assert.match(html, /data-action="config-route-replace"/);
  assert.match(html, /id="t2i-template-editor"/);
  assert.match(html, /id="t2i-template-select"/);
  assert.match(html, /data-action="t2i-template-save"/);
  assert.match(html, /data-action="t2i-template-apply"/);
  assert.match(html, /data-action="t2i-template-reset"/);
  assert.match(html, /class="t2i-preview-frame"/);
  assert.match(html, /webchat:group:room-\*/);
  assert.match(html, /data-config-path="webchat_server\.enabled"/);
  assert.match(html, /data-config-path="webchat_server\.port"/);
  assert.match(html, /type="number"/);
  assert.match(html, /chat_providers\[\]\.api_key/);
  assert.match(html, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
  assert.match(html, /WebChat &lt;Config&gt;/);
  assert.doesNotMatch(html, /<script>alert/);
});

test("chatbox render exposes realtime openapi controls", () => {
  state.chat = {
    conversationId: "demo",
    senderId: "user",
    text: "hello",
    imageUrls: "",
    messagePartsJson: "",
  };
  state.messages = [];
  state.realtime = {
    subscriptions: [
      {
        request_id: "request-1",
        conversation_id: "demo",
        event_id: "webchat-event-1",
        key_id: "key-1",
        status: "queued",
        stop_requested: false,
      },
    ],
    elicitations: [{ elicitation_id: "approval-1", status: "pending" }],
  };

  const html = renderChatBox();

  assert.match(html, /id="openapi-secret"/);
  assert.match(html, /data-action="openapi-stream-chat"/);
  assert.match(html, /data-action="openapi-stop-chat"/);
  assert.match(html, /data-action="openapi-elicitation-create"/);
  assert.match(html, /request-1/);
});

test("chat render mirrors source chat shell controls and rich message parts", () => {
  state.chat = {
    conversationId: "conversation-1",
    senderId: "user",
    text: "hello",
    imageUrls: "",
    messagePartsJson: "",
    stagedAttachments: [{ type: "image", url: "https://example.test/a.png", name: "a.png" }],
    batchMode: true,
    batchSelectedSessionIds: ["conversation-1"],
    selectedConfigId: "ops",
    selectedProviderId: "openai/gpt-4.1-mini",
    selectedModelName: "gpt-4.1-mini",
    sendShortcut: "enter",
    transportMode: "websocket",
    enableStreaming: true,
    liveModeOpen: false,
    live: { status: "idle", messages: [], metrics: {} },
    refsSidebarOpen: true,
    selectedRefs: [{ title: "Doc", content: "Reference text", url: "https://example.test/doc" }],
    replyTo: { messageId: "m-1", selectedText: "quoted" },
    dialog: "rename",
    renameSessionId: "conversation-1",
    renameTitle: "Ops chat",
  };
  state.chatSessions = {
    data: [
      { session_id: "conversation-1", display_name: "Ops chat", platform_id: "webchat", updated_at: "unix:1" },
      { session_id: "conversation-2", display_name: "Second chat", platform_id: "webchat", updated_at: "unix:2" },
    ],
  };
  state.projects = { projects: [{ project_id: "proj-1", title: "Research", emoji: "R", description: "Docs" }] };
  state.projectSessions = { sessions: [] };
  state.activeProjectId = "";
  state.configAbconfs = { info_list: [{ id: "default", name: "default" }, { id: "ops", name: "Ops" }] };
  state.chatProviderList = [
    { id: "openai/gpt-4.1-mini", model: "gpt-4.1-mini", provider_type: "chat_completion", enable: true, models: ["gpt-4.1-mini"] },
  ];
  state.messages = [
    {
      id: "m-1",
      created_at: "unix:1",
      content: {
        type: "bot",
        reasoning: "I am thinking",
        refs: [{ title: "Search result", content: "snippet" }],
        agentStats: { token_usage: { output: 12 } },
        message: [
          { type: "plain", text: "# Hello\n- item" },
          { type: "tool_call", tool_calls: [{ id: "tc-1", name: "web_search", args: { q: "AstrBot" }, result: "{\"ok\":true}", ts: 1, finished_ts: 1.2 }] },
          { type: "tool_call", tool_calls: [{ id: "py-1", name: "astrbot_execute_python", args: { code: "print(1)" }, result: "1", ts: 2, finished_ts: 2.1 }] },
          { type: "elicitation", payload: { elicitation_id: "approval-1", message: "Approve action?" } },
          { type: "action_ref", refs: [{ title: "Action" }] },
        ],
      },
    },
  ];

  const html = renderChat();

  assert.match(html, /data-page="chat"/);
  assert.match(html, /chat-sidebar-panel/);
  assert.match(html, /data-action="chat-new-session"/);
  assert.match(html, /data-action="chat-batch-delete"/);
  assert.match(html, /data-action="chat-projects-toggle"/);
  assert.match(html, /data-action="chat-project-dialog-open"/);
  assert.match(html, /data-action="project-select"/);
  assert.match(html, /data-action="project-delete"/);
  assert.match(html, /Research/);
  assert.match(html, /id="chat-config-select"/);
  assert.match(html, /id="chat-provider-select"/);
  assert.match(html, /id="chat-model-select"/);
  assert.match(html, /id="chat-file-upload"/);
  assert.match(html, /data-action="chat-live-open"/);
  assert.match(html, /Reasoning/);
  assert.match(html, /web_search/);
  assert.match(html, /IPython/);
  assert.match(html, /Approve action/);
  assert.match(html, /Action refs/);
  assert.match(html, /chat-refs-sidebar/);
  assert.match(html, /chat-rename-dialog/);
  assert.doesNotMatch(html, /<script>/);
});

test("chat render exposes project view session workflow controls", () => {
  state.chat = {
    conversationId: "",
    senderId: "user",
    text: "first project message",
    imageUrls: "",
    messagePartsJson: "",
    stagedAttachments: [],
    batchMode: false,
    batchSelectedSessionIds: [],
    selectedConfigId: "default",
    selectedProviderId: "",
    selectedModelName: "",
    sendShortcut: "shift_enter",
    transportMode: "sse",
    enableStreaming: true,
    liveModeOpen: false,
    live: { status: "idle", messages: [], metrics: {} },
    refsSidebarOpen: false,
    selectedRefs: [],
    replyTo: null,
    currentSessionProject: null,
    projectDialogMode: "edit",
    projectDialogTargetId: "proj-1",
    projectsExpanded: true,
    dialog: "project",
    renameSessionId: "",
    renameTitle: "",
  };
  state.chatSessions = { data: [] };
  state.projects = {
    projects: [{ project_id: "proj-1", title: "Research", emoji: "R", description: "Docs" }],
  };
  state.projectSessions = {
    sessions: [
      {
        session_id: "project-session-1",
        display_name: "Project chat",
        platform_id: "webchat",
        creator: "user",
        updated_at: "unix:3",
      },
    ],
  };
  state.activeProjectId = "proj-1";
  state.configAbconfs = { info_list: [{ id: "default", name: "default" }] };
  state.chatProviderList = [];
  state.messages = [];

  const html = renderChat();

  assert.match(html, /data-project-view="proj-1"/);
  assert.match(html, /Docs/);
  assert.match(html, /first project message/);
  assert.match(html, /Project chat/);
  assert.match(html, /data-action="project-session-select"/);
  assert.match(html, /data-action="project-session-remove"/);
  assert.match(html, /data-action="chat-clear-project"/);
  assert.match(html, /id="chat-project-dialog"/);
  assert.match(html, /value="proj-1" hidden/);
  assert.match(html, /data-action="project-dialog-save"/);
  assert.match(html, /data-action="project-delete"/);
});

test("chat projects route exposes stable page marker and membership controls", () => {
  state.projectFilter = "research";
  state.activeProjectId = "proj-1";
  state.projects = {
    projects: [
      {
        project_id: "proj-1",
        title: "Research",
        emoji: "R",
        description: "Docs",
        updated_at: "unix:3",
      },
    ],
  };
  state.projectSessions = {
    sessions: [
      {
        session_id: "project-session-1",
        display_name: "Project chat",
        platform_id: "webchat",
        creator: "user",
        is_group: false,
      },
    ],
  };

  const html = renderProjects();

  assert.match(html, /data-page="chat-projects"/);
  assert.match(html, /Research/);
  assert.match(html, /Project chat/);
  assert.match(html, /id="project-filter"/);
  assert.match(html, /data-action="project-session-upsert"/);
  assert.match(html, /data-action="project-session-add"/);
  assert.match(html, /data-action="project-session-remove"/);
});

test("conversation render exposes source-style filter detail history and batch controls", () => {
  state.conversationFilters = {
    platforms: ["telegram"],
    messageTypes: ["GroupMessage"],
    search: "ops",
    page: 1,
    pageSize: 20,
  };
  state.conversationSelectedKeys = ["telegram:GroupMessage:room-1\u001fconversation-a"];
  state.conversations = {
    status: "ok",
    data: {
      conversations: [
        {
          user_id: "telegram:GroupMessage:room-1",
          cid: "conversation-a",
          platform_id: "telegram",
          title: "Ops room",
          persona_id: "support",
          created_at: 10,
          updated_at: 20,
          history: JSON.stringify([
            { role: "user", content: "hello" },
            { role: "assistant", content: [{ type: "text", text: "world" }] },
          ]),
        },
      ],
      pagination: { page: 1, page_size: 20, total: 1, total_pages: 1 },
    },
  };
  state.conversationDetail = state.conversations.data.conversations[0];
  state.conversationDialog = "history";
  state.conversationHistoryMode = "edit";
  state.conversationHistoryDraft = state.conversationDetail.history;

  const html = renderConversation();

  assert.match(html, /data-page="conversation"/);
  assert.match(html, /id="conversation-filter-platforms"/);
  assert.match(html, /id="conversation-filter-message-type"/);
  assert.match(html, /data-action="conversation-filter-apply"/);
  assert.match(html, /data-action="conversation-export-selected"/);
  assert.match(html, /data-action="conversation-batch-delete-open"/);
  assert.match(html, /telegram:GroupMessage:room-1/);
  assert.match(html, /conversation-a/);
  assert.match(html, /data-action="conversation-view"/);
  assert.match(html, /conversation-history-dialog/);
  assert.match(html, /id="conversation-history-editor"/);
  assert.match(html, /data-action="conversation-history-save"/);
  assert.match(html, /conversation-edit-dialog/);
  assert.match(html, /conversation-delete-dialog/);
  assert.doesNotMatch(html, /<script>/);
});

test("session management render exposes source filter batch group and editor workflow", () => {
  state.sessionFilter = "ops";
  state.sessionPage = 1;
  state.sessionPageSize = 10;
  state.activeUmo = "webchat:GroupMessage:room-1";
  state.sessionSelectedUmos = ["webchat:GroupMessage:room-1"];
  state.sessionAvailableUmos = ["webchat:GroupMessage:room-1", "webchat:FriendMessage:user-1"];
  state.sessionGroups = {
    status: "ok",
    data: {
      groups: [
        {
          id: "ops",
          name: "Ops",
          umos: ["webchat:GroupMessage:room-1"],
          umo_count: 1,
        },
      ],
    },
  };
  state.sessions = {
    status: "ok",
    data: {
      total: 1,
      page: 1,
      page_size: 10,
      available_rule_keys: ["session_service_config", "session_plugin_config", "kb_config"],
      available_personas: [{ name: "support", prompt: "Help" }],
      available_chat_providers: [{ id: "openai", name: "OpenAI", model: "gpt-4.1-mini" }],
      available_stt_providers: [{ id: "whisper", name: "Whisper", model: "whisper-1" }],
      available_tts_providers: [{ id: "edge", name: "Edge", model: "edge-tts" }],
      available_plugins: [{ name: "weather", display_name: "Weather" }],
      available_kbs: [{ kb_id: "kb-1", kb_name: "Docs", emoji: "D" }],
      rules: [
        {
          umo: "webchat:GroupMessage:room-1",
          platform: "webchat",
          message_type: "GroupMessage",
          session_id: "room-1",
          rules: {
            session_service_config: {
              session_enabled: true,
              llm_enabled: false,
              tts_enabled: true,
              custom_name: "Ops Room",
              persona_id: "support",
            },
            session_plugin_config: { enabled_plugins: ["weather"], disabled_plugins: [] },
            kb_config: { kb_ids: ["kb-1"], top_k: 5, enable_rerank: false },
            provider_perf_chat_completion: "openai",
            provider_perf_speech_to_text: "whisper",
            provider_perf_text_to_speech: "edge",
          },
        },
      ],
    },
  };

  state.sessionDialog = "editor";
  state.sessionEditUmo = "webchat:GroupMessage:room-1";
  const editorHtml = renderSessions();
  assert.match(editorHtml, /data-page="session-management"/);
  assert.match(editorHtml, /session-management-page/);
  assert.match(editorHtml, /id="session-filter"/);
  assert.match(editorHtml, /data-action="session-select-all"/);
  assert.match(editorHtml, /data-action="session-batch-delete-open"/);
  assert.match(editorHtml, /data-action="session-rule-edit-open"/);
  assert.match(editorHtml, /data-action="session-quick-name-open"/);
  assert.match(editorHtml, /id="session-rule-editor-dialog"/);
  assert.match(editorHtml, /id="session-editor-chat-provider"/);
  assert.match(editorHtml, /id="session-editor-enabled-plugins"/);
  assert.match(editorHtml, /id="session-editor-kb-ids"/);
  assert.match(editorHtml, /data-action="session-rule-save-kb"/);
  assert.match(editorHtml, /custom_group:ops/);
  assert.match(editorHtml, /data-action="session-group-add-selected"/);
  assert.doesNotMatch(editorHtml, /<script>/);

  state.sessionDialog = "group";
  state.sessionGroupDialogMode = "edit";
  state.sessionGroupTargetId = "ops";
  state.sessionGroupDraftUmos = ["webchat:GroupMessage:room-1"];
  const groupHtml = renderSessions();
  assert.match(groupHtml, /id="session-group-dialog"/);
  assert.match(groupHtml, /transfer-list/);
  assert.match(groupHtml, /data-action="session-group-add-umo"/);
  assert.match(groupHtml, /data-action="session-group-save"/);

  state.sessionDialog = "quick-name";
  state.sessionQuickNameTarget = "webchat:GroupMessage:room-1";
  const quickHtml = renderSessions();
  assert.match(quickHtml, /id="session-quick-name-dialog"/);
  assert.match(quickHtml, /data-action="session-quick-name-save"/);
});

test("console and trace render source-style live controls filters and redacted fields", () => {
  state.consoleAutoScroll = true;
  state.consoleLevels = ["INFO", "ERROR"];
  state.consoleSearch = "runtime";
  state.consolePipDialog = "install";
  state.logs = {
    source_logs: [
      {
        id: 1,
        type: "log",
        time: 1779169818,
        level: "INFO",
        data: "runtime <ready>",
        source: "Runtime",
        target: "event-1",
      },
      {
        id: 2,
        type: "log",
        time: 1779169819,
        level: "DEBUG",
        data: "debug hidden",
        source: "Runtime",
      },
    ],
  };

  const consoleHtml = renderConsole();
  assert.match(consoleHtml, /data-page="console"/);
  assert.match(consoleHtml, /平台日志/);
  assert.match(consoleHtml, /data-action="logs-stream-start"/);
  assert.match(consoleHtml, /data-action="console-level-toggle"/);
  assert.match(consoleHtml, /id="console-search"/);
  assert.match(consoleHtml, /id="console-terminal"/);
  assert.match(consoleHtml, /id="console-pip-dialog"/);
  assert.match(consoleHtml, /runtime &lt;ready&gt;/);
  assert.doesNotMatch(consoleHtml, /runtime <ready>/);
  assert.doesNotMatch(consoleHtml, /debug hidden/);

  state.traceSettings = {
    enabled: true,
    capture_message_outline: true,
    max_events: 100,
    redact_fields: ["authorization"],
  };
  state.traceExpanded = { "span-1": true };
  state.trace = {
    source_events: [
      {
        type: "trace",
        time: 1779169820,
        span_id: "span-1",
        name: "pipeline.process",
        action: "astr_agent_prepare",
        umo: "webchat:FriendMessage:user-1",
        sender_name: "Alice",
        message_outline: "hello <trace>",
        fields: { authorization: "[REDACTED]", provider: "mock" },
      },
    ],
  };

  const traceHtml = renderTrace();
  assert.match(traceHtml, /data-page="trace"/);
  assert.match(traceHtml, /记录中/);
  assert.match(traceHtml, /id="trace-redact-fields"/);
  assert.match(traceHtml, /data-action="trace-toggle-event"/);
  assert.match(traceHtml, /astr_agent_prepare/);
  assert.match(traceHtml, /\[REDACTED\]/);
  assert.match(traceHtml, /hello &lt;trace&gt;/);
  assert.doesNotMatch(traceHtml, /hello <trace>/);

  state.consolePipDialog = "";
  state.consoleSearch = "";
  state.traceExpanded = {};
});

test("persona render exposes source-style folder tree cards form preview and dialogs", () => {
  state.personaSearch = "";
  state.personaFolderId = null;
  state.personaFolderTree = [
    {
      folder_id: "ops",
      name: "Ops",
      parent_id: null,
      description: "Operations",
      sort_order: 0,
      children: [
        {
          folder_id: "incident",
          name: "Incident",
          parent_id: "ops",
          description: "Incident response",
          sort_order: 0,
          children: [],
        },
      ],
    },
  ];
  state.personas = {
    personas: [
      {
        persona_id: "support",
        system_prompt: "Help users safely <unsafe>",
        custom_error_message: "Cannot comply",
        begin_dialogs: ["hello", "hi"],
        tools: null,
        skills: ["writer"],
        folder_id: null,
        sort_order: 0,
      },
      {
        persona_id: "incident-lead",
        system_prompt: "Lead incident response",
        begin_dialogs: [],
        tools: ["diagnostics"],
        skills: [],
        folder_id: "ops",
        sort_order: 0,
      },
    ],
    folders: [
      { folder_id: "ops", name: "Ops", parent_id: null, description: "Operations", sort_order: 0 },
      { folder_id: "incident", name: "Incident", parent_id: "ops", description: "Incident response", sort_order: 0 },
    ],
  };
  state.tools = { tools: [{ name: "diagnostics", description: "Run checks", origin: "plugin", origin_name: "ops" }] };
  state.skills = { skills: [{ name: "writer", description: "Write updates", active: true }] };
  state.mcp = { servers: [{ name: "docs", tools: ["diagnostics"] }] };
  state.personaDialog = "form";
  state.personaEditId = "support";
  state.personaDialogPairCount = 1;

  const html = renderPersonas();

  assert.match(html, /data-page="personas"/);
  assert.match(html, /persona-folder-sidebar/);
  assert.match(html, /data-drop-folder="ops"/);
  assert.match(html, /draggable="true"/);
  assert.match(html, /data-action="persona-create-open"/);
  assert.match(html, /data-action="persona-folder-create-open"/);
  assert.match(html, /id="persona-form-dialog"/);
  assert.match(html, /id="persona-form-prompt"/);
  assert.match(html, /name="persona-tools-mode"/);
  assert.match(html, /name="persona-skills-mode"/);
  assert.match(html, /data-persona-dialog-pair="0"/);
  assert.match(html, /persona-quick-preview/);
  assert.match(html, /Help users safely &lt;unsafe&gt;/);
  assert.doesNotMatch(html, /Help users safely <unsafe>/);

  state.personaDialog = "preview";
  state.personaPreviewId = "support";
  const previewHtml = renderPersonas();
  assert.match(previewHtml, /id="persona-preview-dialog"/);
  assert.match(previewHtml, /Custom error message/);
  assert.match(previewHtml, /全部 工具/);

  state.personaDialog = "move";
  state.personaMoveType = "persona";
  state.personaMoveId = "support";
  const moveHtml = renderPersonas();
  assert.match(moveHtml, /id="persona-move-dialog"/);
  assert.match(moveHtml, /id="persona-move-target"/);

  state.personaDialog = "";
});

test("cron render exposes source-style jobs table form platform stats and actions", () => {
  state.operation = null;
  state.cron = {
    state: "running",
    scheduled_jobs: [{ job_id: "daily-summary", schedule_key: "0 8 * * *", enabled: true }],
    proactive_platforms: [{ id: "webchat", name: "webchat", display_name: "WebChat" }],
    jobs: [
      {
        job_id: "daily-summary",
        name: "Daily <Summary>",
        job_type: "active_agent",
        cron_expression: "0 8 * * *",
        timezone: "Asia/Shanghai",
        session: "webchat:demo",
        note: "Summarize <unsafe>",
        enabled: true,
        persistent: true,
        run_once: false,
        next_run_time: "",
        last_run_at: "2026-05-19T08:00:00Z",
      },
    ],
  };
  state.cronDialog = "form";
  state.cronFormMode = "edit";
  state.cronEditId = "daily-summary";

  const html = renderCron();

  assert.match(html, /data-page="cron"/);
  assert.match(html, /支持主动消息的平台：WebChat\(webchat\)/);
  assert.match(html, /data-action="cron-create-open"/);
  assert.match(html, /data-action="cron-toggle"/);
  assert.match(html, /data-action="cron-edit-open"/);
  assert.match(html, /data-action="cron-run"/);
  assert.match(html, /data-action="cron-delete-open"/);
  assert.match(html, /id="cron-form-dialog"/);
  assert.match(html, /id="cron-form-session"/);
  assert.match(html, /Scheduler Snapshot/);
  assert.match(html, /Daily &lt;Summary&gt;/);
  assert.match(html, /Summarize &lt;unsafe&gt;/);
  assert.doesNotMatch(html, /Daily <Summary>/);

  state.cronDialog = "delete";
  state.cronDeleteId = "daily-summary";
  const deleteHtml = renderCron();
  assert.match(deleteHtml, /id="cron-delete-dialog"/);
  assert.match(deleteHtml, /data-action="cron-delete-confirm"/);

  state.cronDialog = "";
  state.cronEditId = "";
  state.cronDeleteId = "";
});

test("subagent render exposes source-style config handoff preview and execution bridge", () => {
  state.capabilities = {
    services: [
      {
        id: "subagent",
        label: "SubAgent",
        configured: true,
        closure_level: "runtime",
        api_base: "/api/subagent/config",
        notes: ["source-compatible facade"],
      },
    ],
  };
  state.subagents = {
    main_enable: true,
    remove_main_duplicate_tools: true,
    router_system_prompt: "Route <safely>",
    agents: [
      {
        name: "researcher",
        enabled: true,
        persona_id: "support",
        provider_id: "openai/gpt-4.1-mini",
        system_prompt: "Use citations",
        public_description: "Research <unsafe>",
        tools: ["diagnostics"],
      },
    ],
    handoffs: [
      {
        tool_name: "transfer_to_researcher",
        agent_name: "researcher",
        description: "Research <unsafe>",
        tools: ["diagnostics"],
        all_tools: false,
      },
    ],
    executions: [
      {
        run_id: "subagent-run-1",
        agent_name: "researcher",
        handoff_tool: "transfer_to_researcher",
        status: "completed",
        output: "done <unsafe>",
      },
    ],
    available_tools: [{ name: "diagnostics", description: "Run checks", handler_module_path: "ops" }],
    providers: [{ id: "openai/gpt-4.1-mini", provider_type: "chat_completion", name: "OpenAI" }],
    personas: [{ persona_id: "support", name: "Support" }],
  };

  const html = renderSubAgent();

  assert.match(html, /data-page="subagent"/);
  assert.match(html, /SubAgent 编排/);
  assert.match(html, /data-action="subagent-refresh"/);
  assert.match(html, /data-action="subagent-save"/);
  assert.match(html, /data-action="subagent-add"/);
  assert.match(html, /data-action="subagent-remove"/);
  assert.match(html, /id="subagent-main-enable"/);
  assert.match(html, /id="subagent-router-prompt"/);
  assert.match(html, /class="subagent-card"/);
  assert.match(html, /subagent-handoff-table/);
  assert.match(html, /transfer_to_researcher/);
  assert.match(html, /subagent-execution-table/);
  assert.match(html, /data-action="subagent-execute"/);
  assert.match(html, /Research &lt;unsafe&gt;/);
  assert.match(html, /done &lt;unsafe&gt;/);
  assert.doesNotMatch(html, /Research <unsafe>/);
  assert.doesNotMatch(html, /done <unsafe>/);
});

test("settings render exposes source-style network api key backup update controls", () => {
  state.status = { ok: true };
  state.capabilities = {
    services: [
      { label: "Backup", closure_level: "runtime", api_base: "/api/management/backup" },
    ],
  };
  state.apiKeys = {
    api_keys: [
      {
        key_id: "key-1",
        name: "Dashboard",
        key_prefix: "abk_dash",
        scopes: ["chat", "file"],
        created_by: "admin",
        active: true,
      },
    ],
  };
  state.update = { check: { current_version: "v4.0.0", latest_version: "v4.0.1", has_new_version: true } };
  state.changelog = {
    current_version: "v4.0.0",
    releases: [{ version: "v4.0.1", title: "Patch", body: "# Patch\n- Fixed <unsafe>" }],
  };
  state.migration = { check: { pending_storage_migrations: ["001"], legacy_data_migration_needed: true } };
  state.backupFiles = { files: [{ filename: "backup.zip", size_bytes: 2048, astrbot_version: "v4.0.0" }] };
  state.settingsDialog = "backup";

  const html = renderSettings();

  assert.match(html, /data-page="settings"/);
  assert.match(html, /网络/);
  assert.match(html, /使用 GitHub 代理/);
  assert.match(html, /data-action="settings-proxy-test"/);
  assert.match(html, /API Keys/);
  assert.match(html, /data-action="api-key-issue"/);
  assert.match(html, /id="settings-sidebar"/);
  assert.match(html, /data-action="settings-sidebar-open"/);
  assert.match(html, /id="settings-desktop-bridge"/);
  assert.match(html, /data-action="settings-desktop-probe"/);
  assert.match(html, /备份/);
  assert.match(html, /id="backup-dialog"/);
  assert.match(html, /data-action="backup-export"/);
  assert.match(html, /Migration/);
  assert.match(html, /data-action="settings-open-changelog"/);
  assert.match(html, /&lt;unsafe&gt;/);
  assert.doesNotMatch(html, /<unsafe>/);

  state.settingsDialog = "sidebar-customizer";
  state.sidebarCustomizerDraft = { mainItems: ["overview", "chat"], moreItems: ["settings"] };
  const customizerHtml = renderSettings();
  assert.match(customizerHtml, /id="sidebar-customizer-dialog"/);
  assert.match(customizerHtml, /data-action="settings-sidebar-move-more"/);
  assert.match(customizerHtml, /data-action="settings-sidebar-save"/);
  state.sidebarCustomizerDraft = null;
});

test("about render mirrors source brand links version and system information", () => {
  state.route = "about";
  state.routePath = "/about";
  state.routeSourcePath = "/about";
  state.routeReplacementFor = "";
  state.status = { ok: true };
  state.stats = { uptime_seconds: 3661 };
  state.capabilities = {
    services: [
      { label: "Dashboard", closure_level: "runtime", api_base: "/api/management" },
    ],
  };
  state.update = {
    check: {
      current_version: "v4.0.0",
      dashboard_version: "v4.0.1",
      has_new_version: false,
      dashboard_has_new_version: true,
    },
  };
  state.changelog = {
    current_version: "v4.0.0",
    releases: [{ version: "v4.0.1", title: "Patch", body: "# Patch\n- Fixed <unsafe>" }],
  };
  state.settingsDialog = "changelog";

  const html = renderAbout();

  assert.match(html, /data-page="about"/);
  assert.match(html, /src="\/assets\/images\/astrbot_logo_mini\.webp"/);
  assert.match(html, /AstrBot/);
  assert.match(html, /A project out of interests and loves/);
  assert.match(html, /https:\/\/github\.com\/AstrBotDevs\/AstrBot/);
  assert.match(html, /https:\/\/github\.com\/AstrBotDevs\/AstrBot\/issues/);
  assert.match(html, /README/);
  assert.match(html, /data-action="settings-open-changelog"/);
  assert.match(html, /v4\.0\.0/);
  assert.match(html, /v4\.0\.1/);
  assert.match(html, /AGPL v3/);
  assert.match(html, /1h 1m/);
  assert.match(html, /id="changelog-dialog"/);
  assert.match(html, /&lt;unsafe&gt;/);
  assert.doesNotMatch(html, /<unsafe>/);
  state.settingsDialog = "";
});

test("about render marks legacy Alkaid plugin routes as explicit replacements", () => {
  state.route = "about";
  state.routePath = "/alkaid/long-term-memory";
  state.routeSourcePath = "/alkaid/long-term-memory";
  state.routeReplacementFor = "legacy-alkaid";
  state.capabilities = {
    services: [
      { label: "Knowledge Base", closure_level: "runtime", api_base: "/api/management/knowledge-base" },
    ],
  };

  const html = renderAbout();

  assert.match(html, /data-page="legacy-alkaid-replacement"/);
  assert.match(html, /legacy Alkaid plugin UI 不在 RS Dashboard runtime parity 范围/);
  assert.match(html, /href="#\/knowledge-base"/);
  assert.match(html, /href="#\/alkaid\/knowledge-base"/);
  assert.match(html, /\/alkaid\/long-term-memory/);
  assert.match(html, /\/alkaid\/other/);
  assert.match(html, /\/api\/plug\/alkaid\/ltm\/graph\/search/);
  state.routeReplacementFor = "";
});

test("provider render mirrors source provider tabs sources models and dialogs", () => {
  state.status = {
    providers: {
      chat_provider_count: 1,
      speech_to_text_provider_count: 0,
      text_to_speech_provider_count: 0,
      embedding_provider_count: 1,
      rerank_provider_count: 0,
      default_chat_provider_id: "openai/gpt-4.1-mini",
      default_embedding_provider_id: "embedding",
    },
  };
  state.providerTab = "chat_completion";
  state.providerSelectedSourceId = "openai";
  state.providerModels = ["gpt-4.1-mini", "o3-mini"];
  state.providerModelMetadata = {
    "gpt-4.1-mini": {
      modalities: { input: ["text", "image"] },
      tool_call: true,
      limit: { context: 1048576 },
    },
  };
  state.providerStatuses = {
    "openai/gpt-4.1-mini": { status: "available", error: null },
  };
  state.providerCatalog = {
    config_schema: {
      provider: {
        config_template: {
          OpenAI: {
            id: "openai",
            type: "openai_chat_completion",
            provider_type: "chat_completion",
            provider: "openai",
            api_base: "https://api.openai.com/v1",
          },
          "OpenAI Embedding": {
            id: "embedding",
            type: "openai_embedding",
            provider_type: "embedding",
            provider: "openai",
          },
        },
      },
    },
    provider_sources: [
      {
        id: "openai",
        type: "openai_chat_completion",
        provider_type: "chat_completion",
        provider: "openai",
        api_base: "https://api.openai.com/v1",
        enable: true,
      },
    ],
    providers: [
      {
        id: "openai/gpt-4.1-mini",
        type: "openai_chat_completion",
        provider_type: "chat_completion",
        provider_source_id: "openai",
        provider: "openai",
        model: "gpt-4.1-mini",
        enable: true,
      },
      {
        id: "embedding",
        type: "openai_embedding",
        provider_type: "embedding",
        model: "text-embedding-3-small",
        dimensions: 1024,
        voice: "alloy",
        enable: true,
      },
    ],
  };

  const html = renderProviders();

  assert.match(html, /data-page="providers"/);
  assert.match(html, /provider-page/);
  assert.match(html, /模型提供商/);
  assert.match(html, /提供商源/);
  assert.match(html, /provider-source-id/);
  assert.match(html, /provider-model-search/);
  assert.match(html, /获取模型列表/);
  assert.match(html, /自定义模型/);
  assert.match(html, /openai\/gpt-4\.1-mini/);
  assert.match(html, /默认/);
  assert.match(html, /vision/);
  assert.match(html, /tool/);
  assert.match(html, /可用/);
  assert.match(html, /data-action="provider-copy"/);
  assert.match(html, /provider-add-dialog/);

  state.providerModelSearch = "o3";
  const filteredHtml = renderProviders();
  assert.match(filteredHtml, /o3-mini/);
  assert.doesNotMatch(filteredHtml, /provider-model-row configured/);
  state.providerModelSearch = "";

  state.providerTab = "embedding";
  state.providerEditId = "embedding";
  const embeddingHtml = renderProviders();
  assert.match(embeddingHtml, /嵌入\(Embedding\)/);
  assert.match(embeddingHtml, /OpenAI Embedding/);
  assert.match(embeddingHtml, /text-embedding-3-small/);
  assert.match(embeddingHtml, /dim 1024/);
  assert.match(embeddingHtml, /data-action="provider-toggle"/);
  assert.match(embeddingHtml, /检测维度/);
  assert.match(embeddingHtml, /Adapter-specific JSON/);
  assert.match(embeddingHtml, /voice/);
  state.providerEditId = "";
});

test("platform render mirrors source platform cards templates abconf routes and console", () => {
  state.status = {
    platforms: {
      platform_count: 1,
      platform_ids: ["onebot-main"],
      webchat_platform_count: 0,
      onebot_platform_count: 1,
      recording_sink_count: 1,
    },
  };
  state.stats = {
    platform_counts: [{ platform_id: "onebot-main", platform_type: "onebot", count: 12 }],
  };
  state.logs = {
    snapshot: {
      entries: [{ timestamp: "2026-05-18T10:00:00Z", level: "INFO", message: "platform ready" }],
    },
  };
  state.platformCatalog = {
    templates: [
      { platform_type: "console", label: "Console", runtime_supported: true },
      { platform_type: "onebot", label: "OneBot", runtime_supported: true },
    ],
  };
  state.platformConfig = {
    callback_api_base: "https://bot.example",
    platforms: [
      {
        id: "onebot-main",
        type: "onebot",
        name: "OneBot Main",
        enabled: true,
        options: { webhook_uuid: "uuid-1", ws_reverse_port: 6199 },
        secrets: { ws_reverse_token: "<redacted>" },
      },
    ],
  };
  state.platformAbconfs = {
    info_list: [
      { id: "default", name: "Default" },
      { id: "ops", name: "Ops" },
    ],
  };
  state.platformRoutes = {
    routing: {
      "onebot-main:*:*": "default",
      "onebot-main:GroupMessage:ops-*": "ops",
    },
  };
  state.platformRuntimeStats = {
    platforms: [
      {
        id: "onebot-main",
        status: "error",
        error_count: 2,
        unified_webhook: true,
        last_error: { message: "boom", timestamp: "2026-05-18T10:00:00Z", traceback: "stack" },
      },
    ],
  };
  state.platformChecks = { "onebot-main": { status: "available", message: "ok" } };
  state.platformDialog = "add-platform";
  state.platformSelectedTemplate = "Console";
  state.platformDraft = {
    id: "console-ops",
    type: "console",
    name: "Console Ops",
    enabled: true,
    options: {},
    secrets: {},
  };
  state.platformShowConfigSection = true;
  state.platformConfigMode = "new";
  state.platformNewConfigName = "ops";
  state.platformNewConfigDraft = { providers: [] };
  state.platformShowConsole = true;
  state.platformErrorId = "onebot-main";
  state.platformWebhookUuid = "uuid-1";

  const html = renderPlatforms();

  assert.match(html, /data-page="platforms"/);
  assert.match(html, /platform-page/);
  assert.match(html, /平台适配器/);
  assert.match(html, /Online Platforms/);
  assert.match(html, /OneBot Main/);
  assert.match(html, /2 条路由/);
  assert.match(html, /Webhook/);
  assert.match(html, /配置可构建/);
  assert.match(html, /platform ready/);
  assert.match(html, /platform-add-dialog/);
  assert.match(html, /platform-template-card active/);
  assert.match(html, /Options JSON/);
  assert.match(html, /platform-new-config-json/);
  assert.match(html, /https:\/\/bot\.example\/api\/platform\/webhook\/uuid-1/);
  assert.match(html, /boom/);

  state.platformDialog = "";
  state.platformErrorId = "";
  state.platformWebhookUuid = "";
  state.platformEditId = "onebot-main";
  state.platformRouteEdit = true;
  state.platformRouteDrafts = [
    { messageType: "*", sessionId: "*", configId: "default" },
    { messageType: "GroupMessage", sessionId: "ops-*", configId: "ops" },
  ];
  const editHtml = renderPlatforms();
  assert.match(editHtml, /platform-edit-dialog/);
  assert.match(editHtml, /UMO 路由/);
  assert.match(editHtml, /platform-route-0-message-type/);
  assert.match(editHtml, /data-action="platform-route-add"/);

  state.platformEditId = "";
  state.platformRouteEdit = false;
});

test("extension renders installed market tools mcp skills parity surfaces", () => {
  state.extensionPluginSearch = "";
  state.extensionPluginStatusFilter = "all";
  state.extensionPluginShowReserved = true;
  state.extensionPluginView = "grid";
  state.extensionDialog = "";
  state.extensionDoc = null;
  state.marketSearch = "";
  state.marketSortBy = "name";
  state.marketSortOrder = "asc";
  state.commandSearch = "";
  state.commandPluginFilter = "all";
  state.commandPermissionFilter = "all";
  state.commandStatusFilter = "all";
  state.toolSearch = "";
  state.commandDetailsId = "";
  state.commandRenameId = "";
  state.toolDetailsName = "";
  state.skillsMode = "local";
  state.pluginLifecycle = {
    handlers: {
      handler_count: 1,
      handlers: [{ plugin_name: "weather", handler_name: "main", event_type: "Message", priority: 10, enabled: true }],
    },
    plugins: [
      {
        plugin_id: "weather",
        name: "Weather <Plugin>",
        version: "1.2.0",
        description: "Forecast <unsafe>",
        state: "loaded",
        active: true,
        source: { kind: "python_compat", root_dir: "plugins/weather", reserved: false },
        capabilities: ["handler", "tool"],
        permissions: ["network"],
        config: { city: "Shanghai" },
        config_files: [{ filename: "config.json" }],
      },
    ],
    operations: [],
  };
  state.market = {
    plugins: [
      {
        plugin_id: "weather",
        name: "Weather",
        version: "1.3.0",
        repo_url: "https://example.com/weather",
        installed: true,
        installed_version: "1.2.0",
        compatibility: { compatible: true },
        package: { source: { kind: "repository" } },
        readme: { markdown: "# Weather\n- Safe <docs>" },
      },
    ],
    installed_plugins: [],
    operations: [],
  };
  state.tools = {
    tools: [{
      name: "weather.lookup",
      description: "Lookup <city>",
      active: true,
      origin_name: "weather",
      origin: "plugin",
      source: "plugin:weather",
      user_toggle_allowed: true,
      parameters: { type: "object" },
    }],
  };
  state.commands = {
    commands: [{
      handler_full_name: "weather.main",
      plugin_name: "weather",
      handler_name: "main",
      command_type: "command",
      original_command: "weather",
      current_fragment: "weather",
      effective_command: "/weather",
      aliases: ["forecast"],
      permission: "admin",
      enabled: true,
      description: "Weather command",
      response: "ok",
      priority: 10,
    }],
    conflicts: [{ command: "/weather", handlers: ["weather.main", "other.main"] }],
  };
  state.mcp = {
    active_count: 1,
    servers: [{
      name: "docs",
      active: true,
      transport: "stdio",
      command: "npx",
      args: ["-y", "server"],
      valid: true,
      session_read_timeout_seconds: 60,
      client_capabilities: {},
    }],
  };
  state.skills = {
    skills: [{
      name: "writer",
      description: "Write <safe>",
      path: "skills/writer/SKILL.md",
      active: true,
      source_type: "local_only",
      source_label: "Local",
      local_exists: true,
      sandbox_exists: false,
    }],
    sandbox_cache: { ready: true, count: 1 },
  };
  state.skillsNeo = {
    candidates: { data: [{ id: "cand-1", skill_key: "writer", status: "pending", payload_ref: "payload-1" }] },
    releases: { data: [{ id: "rel-1", skill_key: "writer", stage: "stable", is_active: true }] },
  };

  const installed = renderPlugins();
  assert.match(installed, /data-page="extension-installed"/);
  assert.match(installed, /extension-card-grid/);
  assert.match(installed, /Weather &lt;Plugin&gt;/);
  assert.match(installed, /Forecast &lt;unsafe&gt;/);
  assert.match(installed, /data-action="plugin-config-open"/);
  assert.match(installed, /Upload Plan/);
  assert.doesNotMatch(installed, /Forecast <unsafe>/);

  state.extensionDialog = "plugin-doc";
  state.extensionDoc = {
    title: "Weather README",
    markdown: "# Weather\n- Safe <docs>",
    capability: "runtime",
    repo_url: "https://example.com/weather",
  };
  const docHtml = renderPlugins();
  assert.match(docHtml, /id="plugin-doc-dialog"/);
  assert.match(docHtml, /Weather README/);
  assert.match(docHtml, /Safe &lt;docs&gt;/);

  state.extensionDialog = "";
  state.extensionDoc = null;
  const marketHtml = renderMarket();
  assert.match(marketHtml, /data-page="extension-marketplace"/);
  assert.match(marketHtml, /随机推荐/);
  assert.match(marketHtml, /data-action="plugin-execute" data-plan="install"/);
  assert.match(marketHtml, /source-warning/);

  const toolsHtml = renderTools();
  assert.match(toolsHtml, /data-page="extension-tools"/);
  assert.match(toolsHtml, /Command conflicts/);
  assert.match(toolsHtml, /data-action="command-rename-open"/);
  assert.match(toolsHtml, /data-action="tool-details-open"/);
  assert.match(toolsHtml, /MCP Servers/);
  assert.match(toolsHtml, /id="mcp-json"/);
  assert.match(toolsHtml, /Sync Provider/);

  const skillsHtml = renderSkills();
  assert.match(skillsHtml, /data-page="extension-skills"/);
  assert.match(skillsHtml, /Batch Upload Plan/);
  assert.match(skillsHtml, /writer/);
  assert.match(skillsHtml, /Download/);
  assert.match(skillsHtml, /Write &lt;safe&gt;/);

  state.skillsMode = "neo";
  const neoHtml = renderSkills();
  assert.match(neoHtml, /Neo Skills/);
  assert.match(neoHtml, /cand-1/);
  assert.match(neoHtml, /rel-1/);
  assert.match(neoHtml, /data-action="skill-neo-action"/);
});

test("shared ui helpers cover dialog tabs table form and markdown states", () => {
  const dialog = confirmDialog({
    title: "<Danger>",
    message: "Delete <plugin>?",
    open: true,
  });
  assert.match(dialog, /role="dialog"/);
  assert.match(dialog, /&lt;Danger&gt;/);
  assert.match(dialog, /Delete &lt;plugin&gt;\?/);
  assert.doesNotMatch(dialog, /<plugin>/);

  const unsaved = unsavedChangesDialog({
    message: "Leave <page>?",
    hints: ["discard", "stay", "close"],
  });
  assert.match(unsaved, /data-persistent="true"/);
  assert.match(unsaved, /Leave &lt;page&gt;\?/);
  assert.match(unsaved, /ui-hint-row/);

  const tabHtml = tabs({
    id: "config-tabs",
    items: [
      { id: "normal", label: "Normal", body: "Visible" },
      { id: "advanced", label: "Advanced", body: "Hidden" },
    ],
  });
  assert.match(tabHtml, /role="tablist"/);
  assert.match(tabHtml, /aria-selected="true"/);
  assert.match(tabHtml, /ui-window-item/);

  const table = dataTable({
    columns: [{ key: "name", label: "Name" }],
    rows: [{ id: "row-1", name: "<script>alert(1)</script>" }],
  });
  assert.match(table, /data-row-key="row-1"/);
  assert.match(table, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
  assert.doesNotMatch(table, /<script>alert/);

  const controls = [
    formField({ label: "Provider", id: "provider", type: "autocomplete", value: "openai", options: ["openai"] }),
    formField({ label: "Models", id: "models", type: "combobox", value: ["gpt-5.2"], options: ["gpt-5.2"] }),
    formField({ label: "Upload", id: "file", type: "file", accept: ".json" }),
    formField({ label: "JSON", id: "json", type: "json", value: { ok: true } }),
    formField({ label: "Enabled", id: "enabled", type: "switch", value: true }),
  ].join("");
  assert.match(controls, /list="provider-options"/);
  assert.match(controls, /role="combobox"/);
  assert.match(controls, /type="file"/);
  assert.match(controls, /data-editor="monaco-fallback"/);
  assert.match(controls, /ui-switch/);

  const markdown = markdownViewer({ markdown: "# Title\n- `safe`\n\n```js\nalert('<x>')\n```" });
  assert.match(markdown, /markdown-body/);
  assert.match(markdown, /language-js/);
  assert.match(markdown, /&lt;x&gt;/);

  const preview = renderUiBaseShowcase();
  assert.match(preview, /Shared UI Base/);
  assert.match(preview, /ui-state loading/);
  assert.match(preview, /ui-state empty/);
  assert.match(preview, /ui-state error/);
});

test("route i18n supports zh-CN en-US and ru-RU locales", () => {
  const validation = validateI18nDictionaries();
  assert.equal(validation.isValid, true, JSON.stringify(validation.errors, null, 2));
  assert.ok(validation.totalKeys > 80);

  setLocale("en");
  assert.equal(locale(), "en-US");
  assert.equal(localizedRoutes().find((route) => route.id === "overview").label, "Console");
  assert.equal(t("features.settings.desktop.lastProbe", { time: "now" }), "Last probe: now");

  setLocale("ru");
  assert.equal(locale(), "ru-RU");
  assert.equal(localizedRoutes().find((route) => route.id === "overview").label, "Консоль");

  setLocale("zh");
  assert.equal(locale(), "zh-CN");
  assert.equal(localizedRoutes().find((route) => route.id === "overview").label, "控制台");
});

test("desktop bridge helpers expose browser fallback and runtime probe", async () => {
  delete globalThis.window.astrbotDesktop;
  delete globalThis.window.astrbotAppUpdater;
  const fallback = desktopBridgeSnapshot();
  assert.equal(fallback.bridgePresent, false);
  assert.equal(fallback.fallbackReason, "desktop bridge unavailable");

  globalThis.window.astrbotDesktop = {
    isDesktop: false,
    isDesktopRuntime: async () => true,
    getBackendState: async () => ({ running: true, spawning: false, restarting: false, canManage: true }),
    restartBackend: async () => ({ ok: true, reason: null }),
  };
  const probed = await probeDesktopBridge();
  assert.equal(probed.bridgePresent, true);
  assert.equal(probed.isDesktop, true);
  assert.equal(probed.backendState.canManage, true);
  delete globalThis.window.astrbotDesktop;
});

test("legacy locale aliases normalize to source locale codes", () => {
  setLocale("en");
  assert.equal(locale(), "en-US");
  assert.equal(localizedRoutes().find((route) => route.id === "overview").label, "Console");

  setLocale("ru");
  assert.equal(locale(), "ru-RU");
  assert.equal(localizedRoutes().find((route) => route.id === "overview").label, "Консоль");

  setLocale("zh");
  assert.equal(localizedRoutes().find((route) => route.id === "overview").label, "控制台");
});

test("dashboard preferences and api base are persisted and applied", () => {
  setApiBase(" http://127.0.0.1:6185/ ");
  assert.equal(apiBase(), "http://127.0.0.1:6185");
  assert.equal(apiUrl("/api/management/status"), "http://127.0.0.1:6185/api/management/status");

  setDashboardPreferences({ theme: "dark", sidebarCompact: true, primaryColor: "#123456", secondaryColor: "#0f766e" });
  assert.deepEqual(dashboardPreferences(), {
    theme: "dark",
    sidebarCompact: true,
    primaryColor: "#123456",
    secondaryColor: "#0f766e",
    githubProxyEnabled: false,
    githubProxyUrl: "",
    apiBasePresets: [],
    sidebarMainItems: [],
    sidebarMoreItems: [],
  });
  assert.equal(document.body.dataset.theme, "dark");
  assert.equal(document.body.classList.contains("sidebar-compact"), true);
  assert.equal(document.documentElement.style.values.get("--primary"), "#123456");
});
