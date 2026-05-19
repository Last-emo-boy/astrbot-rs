import { expect, test } from "@playwright/test";

import { sourceRouteAuditCases } from "../parity-fixtures.js";

test.describe("source route UI parity audit", () => {
  for (const routeCase of sourceRouteAuditCases) {
    test(`${routeCase.path} renders with fixtures, screenshot smoke, and no console errors`, async ({ page }) => {
      const consoleErrors = [];
      const pageErrors = [];
      const unhandledApiRequests = [];

      page.on("console", (message) => {
        if (message.type() !== "error") return;
        // Playwright init scripts are intentionally blocked inside the sandboxed T2I
        // preview iframe; that is harness noise, not an app console error.
        if (message.text().includes("Blocked script execution in 'about:srcdoc'")) return;
        consoleErrors.push(message.text());
      });
      page.on("pageerror", (error) => {
        if (error.message.includes("localStorage") && error.message.includes("document is sandboxed")) return;
        pageErrors.push(error.message);
      });

      await page.addInitScript(({ token }) => {
        window.localStorage.setItem("astrbot.openapiSecret", "openapi-audit-token");
        if (token) {
          window.localStorage.setItem("astrbot.managementToken", "ui-parity-audit-token");
        } else {
          window.localStorage.removeItem("astrbot.managementToken");
        }
      }, { token: routeCase.token !== false });
      await installParityAuditMocks(page, unhandledApiRequests);

      await page.goto(routeCase.path);

      await expect(page.locator(routeCase.selector).first()).toBeVisible();
      const screenshot = await page.screenshot({ fullPage: true });
      expect(screenshot.length).toBeGreaterThan(routeCase.screenshotBytes || 3_000);
      expect(unhandledApiRequests).toEqual([]);
      expect(consoleErrors).toEqual([]);
      expect(pageErrors).toEqual([]);
    });
  }
});

async function installParityAuditMocks(page, unhandledApiRequests) {
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = requestJson(request);
    const fixture = fixtureForRequest(url, request.method(), body);
    if (fixture) {
      return fulfillJson(route, fixture.data, fixture.status);
    }

    unhandledApiRequests.push(`${request.method()} ${url.pathname}${url.search}`);
    return fulfillJson(route, { error: `unhandled ${url.pathname}` }, 404);
  });
}

function fixtureForRequest(url, method, body) {
  const path = url.pathname;
  if (path === "/api/management/status") {
    return json({
      providers: {
        chat_provider_count: 1,
        embedding_provider_count: 1,
        rerank_provider_count: 0,
        default_chat_provider_id: "openai/gpt-4.1-mini",
      },
      platforms: {
        platform_count: 1,
        platform_ids: ["webchat"],
        webchat_platform_count: 1,
      },
      plugins: { handler_count: 1 },
    });
  }
  if (path === "/api/management/dashboard/capabilities") {
    return json({
      services: [
        { id: "dashboard", label: "Dashboard", closure_level: "visual-parity", api_base: "/api/management/status", configured: true },
        { id: "subagent", label: "SubAgent", closure_level: "runtime", api_base: "/api/subagent/config", configured: true },
      ],
    });
  }
  if (path === "/api/management/stats") {
    return json({
      uptime_seconds: 3661,
      total_messages: 8,
      total_llm_calls: 2,
      total_tokens: 128,
      total_tts_events: 0,
      platform_counts: [{ platform_id: "webchat", platform_type: "webchat", count: 8 }],
      provider_usage: [{ provider_id: "openai/gpt-4.1-mini", calls: 2, total_tokens: 128 }],
      recent_events: [{ kind: "platform_message", timestamp: "2026-05-18T10:00:00Z", count: 8 }],
    });
  }
  if (path === "/api/management/config/schema") return json(configSchemaFixture());
  if (path === "/api/management/config/abconfs") return json({ info_list: configList() });
  if (path === "/api/management/config/routes") return json({ routes: [{ pattern: "webchat:group:ops-*", config_id: "ops" }] });
  if (path === "/api/management/config/current") return json({ config: currentConfigFixture() });
  if (path === "/api/management/config/abconfs/get") {
    return json({ abconf: { id: body.id || "ops", name: "Ops", config: currentConfigFixture() } });
  }
  if (path === "/api/t2i/templates") {
    return json(legacyOk([
      { name: "base", source: "builtin", is_default: true, active: true },
      { name: "ops_card", source: "user", is_default: false, active: false },
    ]));
  }
  if (path === "/api/t2i/templates/active") return json(legacyOk({ active_template: "base" }));
  if (path.startsWith("/api/t2i/templates/")) {
    return json(legacyOk({ name: decodeURIComponent(path.split("/").pop()), content: "<main>{{ text }}</main>" }));
  }
  if (path === "/api/config/provider/template") {
    return json(legacyOk(providerCatalogFixture()));
  }
  if (path === "/api/config/provider/list") {
    return json(legacyOk(providerCatalogFixture().providers));
  }
  if (path === "/api/management/platforms/catalog") {
    return json({ templates: [{ platform_type: "webchat", label: "WebChat", runtime_supported: true }], platforms: [] });
  }
  if (path === "/api/config/get") {
    return json(legacyOk({
      config: {
        platforms: [{
          id: "webchat",
          type: "webchat",
          name: "WebChat",
          enabled: true,
          options: { webhook_uuid: "uuid-1" },
          secrets: {},
        }],
      },
      metadata: {},
    }));
  }
  if (path === "/api/config/abconfs") return json(legacyOk({ info_list: configList() }));
  if (path === "/api/config/umo_abconf_routes") {
    return json(legacyOk({ routing: { "webchat:*:*": "default" }, routes: [{ pattern: "webchat:*:*", config_id: "default" }] }));
  }
  if (path === "/api/platform/stats") {
    return json(legacyOk({
      platforms: [{
        id: "webchat",
        type: "webchat",
        status: "running",
        unified_webhook: true,
        meta: { support_proactive_message: true, display_name: "WebChat" },
      }],
    }));
  }
  if (path === "/api/management/kb/catalog") {
    return json({ knowledge_bases: [knowledgeBaseFixture()] });
  }
  if (path === "/api/management/kb/get") {
    return json({ knowledge_base: knowledgeBaseFixture(body.kb_id || "kb-1") });
  }
  if (path === "/api/management/kb/document/list") {
    return json({ documents: [documentFixture(body.kb_id || "kb-1")] });
  }
  if (path === "/api/management/kb/document/get") {
    return json(documentFixture("kb-1", body.doc_id || "doc-1"));
  }
  if (path === "/api/management/kb/chunk/list") {
    return json({ chunks: [{ chunk_id: "chunk-1", doc_id: "doc-1", kb_id: "kb-1", chunk_index: 0, content: "AstrBot parity chunk" }] });
  }
  if (path === "/api/management/tools") {
    return json({ tools: [{ name: "weather.lookup", description: "Lookup weather", active: true, origin: "plugin" }] });
  }
  if (path === "/api/management/commands") {
    return json({ commands: [{ handler_full_name: "weather.main", plugin_name: "weather", effective_command: "/weather", enabled: true }], conflicts: [] });
  }
  if (path === "/api/management/mcp/servers") {
    return json({ servers: [{ name: "docs", active: true, transport: "stdio", command: "npx", args: ["server"], valid: true }], active_count: 1 });
  }
  if (path === "/api/session/list-rule") {
    return json(legacyOk({
      total: 1,
      page: 1,
      page_size: 10,
      available_personas: [{ name: "support", prompt: "Help" }],
      available_chat_providers: [{ id: "openai/gpt-4.1-mini", name: "OpenAI", model: "gpt-4.1-mini" }],
      available_stt_providers: [],
      available_tts_providers: [],
      available_plugins: [{ name: "weather", display_name: "Weather" }],
      available_kbs: [{ kb_id: "kb-1", kb_name: "Docs", emoji: "D" }],
      rules: [{
        umo: "webchat:GroupMessage:room-1",
        platform: "webchat",
        message_type: "GroupMessage",
        session_id: "room-1",
        rules: { session_service_config: { session_enabled: true, custom_name: "Ops Room" } },
      }],
    }));
  }
  if (path === "/api/session/groups") {
    return json(legacyOk({ groups: [{ id: "ops", name: "Ops", umos: ["webchat:GroupMessage:room-1"], umo_count: 1 }] }));
  }
  if (path === "/api/session/active-umos") {
    return json(legacyOk({ umos: ["webchat:GroupMessage:room-1"] }));
  }
  if (path === "/api/management/chat-projects") {
    return json({ projects: [projectFixture()] });
  }
  if (path === "/api/management/chat-projects/sessions") {
    return json({ sessions: [projectSessionFixture()] });
  }
  if (path === "/api/chat/sessions") {
    return json(legacyOk([{ session_id: "conversation-1", display_name: "Ops chat", platform_id: "webchat", updated_at: "unix:1" }]));
  }
  if (path === "/api/management/conversations") {
    return json({ conversations: [conversationFixture()] });
  }
  if (path === "/api/conversation/list") {
    return json(legacyOk({
      conversations: [conversationFixture()],
      pagination: { page: 1, page_size: 20, total: 1, total_pages: 1 },
    }));
  }
  if (path.startsWith("/api/webchat/") && path.endsWith("/messages")) {
    return json({ messages: [{ id: "m-1", created_at: "unix:1", content: { type: "bot", message: [{ type: "plain", text: "hello" }] } }] });
  }
  if (path === "/api/chat/get_session") {
    return json(legacyOk({ history: [{ role: "assistant", content: "hello" }], project: null }));
  }
  if (path === "/api/openapi/chat/subscriptions") {
    return json({ subscriptions: [{ request_id: "request-1", conversation_id: "conversation-1", status: "queued" }] });
  }
  if (path === "/api/openapi/elicitation") {
    return json({ elicitations: [{ elicitation_id: "approval-1", status: "pending" }] });
  }
  if (path === "/api/management/skills") {
    return json({ skills: [{ name: "writer", description: "Write", active: true, source_type: "local_only" }], sandbox_cache: { ready: true, count: 1 } });
  }
  if (path === "/api/skills/neo/candidates") return json(legacyOk([{ id: "cand-1", skill_key: "writer", status: "pending" }]));
  if (path === "/api/skills/neo/releases") return json(legacyOk([{ id: "rel-1", skill_key: "writer", stage: "stable", is_active: true }]));
  if (path === "/api/management/plugin-market") {
    return json({
      plugins: [{ plugin_id: "weather", name: "Weather", version: "1.3.0", installed: true, installed_version: "1.2.0", compatibility: { compatible: true } }],
      installed_plugins: [],
      operations: [],
    });
  }
  if (path === "/api/management/plugins/lifecycle") {
    return json({
      handlers: { handlers: [{ plugin_name: "weather", handler_name: "main", event_type: "Message", enabled: true }], handler_count: 1 },
      plugins: [{ plugin_id: "weather", name: "Weather", version: "1.2.0", description: "Forecast", state: "loaded", active: true, source: { kind: "python_compat" } }],
      operations: [],
    });
  }
  if (path === "/api/management/update/check") {
    return json({ check: { current_version: "v4.0.0", latest_version: "v4.0.1", has_new_version: true, dashboard_version: "v4.0.1" } });
  }
  if (path === "/api/management/update/releases") {
    return json({ releases: [{ version: "v4.0.1", title: "Dashboard parity" }] });
  }
  if (path === "/api/management/update/changelog") {
    return json({ current_version: "v4.0.0", releases: [{ version: "v4.0.1", title: "Dashboard parity", body: "# Dashboard parity" }] });
  }
  if (path === "/api/management/update/migration-check") return json({ check: { pending_storage_migrations: [] } });
  if (path === "/api/management/backup/files") {
    return json({ files: [{ filename: "backup.zip", size: 2048, created_at: "2026-05-18T10:00:00Z" }] });
  }
  if (path === "/api/management/logs") {
    return json({ snapshot: { entries: [{ id: 1, level: "INFO", message: "runtime ready", timestamp: "2026-05-18T10:00:00Z" }] } });
  }
  if (path === "/api/log-history") {
    return json(legacyOk({ logs: [{ id: 1, type: "log", level: "INFO", message: "runtime ready", time: 1779169818 }] }));
  }
  if (path === "/api/management/trace") {
    return json({ events: [{ trace_id: "trace-1", span_id: "span-1", action: "process", origin: "runtime", message_outline: "ok" }] });
  }
  if (path === "/api/management/trace/settings") {
    return json({ enabled: true, capture_message_outline: true, max_events: 500, redact_fields: ["api_key"] });
  }
  if (path === "/api/persona/list") {
    return json(legacyOk([{ id: "support", name: "support", prompt: "Help", folder_id: "ops" }]));
  }
  if (path === "/api/persona/folder/tree") {
    return json(legacyOk([{ id: "ops", name: "Ops", children: [] }]));
  }
  if (path === "/api/cron/jobs") {
    return json(legacyOk([{ job_id: "job-1", name: "active_agent_task", cron_expression: "0 9 * * *", session: "webchat:GroupMessage:room-1", enabled: true }]));
  }
  if (path === "/api/management/cron/jobs") {
    return json({ state: "running", scheduled_jobs: [{ job_id: "job-1", next_run_time: "2026-05-19T09:00:00+08:00" }], jobs: [] });
  }
  if (path === "/api/subagent/config") {
    return json(legacyOk({ main_enable: true, router_system_prompt: "Route work", agents: [{ name: "researcher", enabled: true, persona_id: "support", provider_id: "openai/gpt-4.1-mini", system_prompt: "Research", public_description: "Research tasks", tools: ["weather.lookup"] }] }));
  }
  if (path === "/api/management/subagents") {
    return json({ main_enable: true, remove_main_duplicate_tools: false, handoffs: [{ from: "main", to: "researcher", reason: "research" }], executions: [] });
  }
  if (path === "/api/subagent/available-tools") {
    return json(legacyOk([{ name: "weather.lookup", description: "Lookup weather", active: true }]));
  }
  if (path === "/api/management/api-keys") {
    return json({ api_keys: [{ id: "key-1", name: "Dashboard", prefix: "abk_demo", scopes: ["chat"], revoked: false }] });
  }
  if (method === "OPTIONS") return json({});
  return null;
}

function json(data, status = 200) {
  return { data, status };
}

function fulfillJson(route, data, status = 200) {
  return route.fulfill({
    status,
    contentType: "application/json; charset=utf-8",
    body: JSON.stringify(data),
  });
}

function requestJson(request) {
  const raw = request.postData();
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

function legacyOk(data, message = "") {
  return { status: "ok", message, data };
}

function configList() {
  return [{ id: "default", name: "default" }, { id: "ops", name: "Ops" }];
}

function currentConfigFixture() {
  return {
    webchat_server: { enabled: true, host: "127.0.0.1", port: 6185 },
    chat_providers: [],
  };
}

function configSchemaFixture() {
  return {
    schema: {
      version: 1,
      fields: [
        { path: "webchat_server.enabled", value_type: "bool", default_value: false, secret: false },
        { path: "webchat_server.host", value_type: "string", default_value: "127.0.0.1", secret: false },
        { path: "webchat_server.port", value_type: "integer", default_value: 6185, secret: false },
        { path: "chat_providers", value_type: "list", default_value: [], secret: false },
      ],
    },
    ui_metadata: {
      groups: [
        {
          id: "webchat",
          title: "WebChat",
          fields: [
            { path: "webchat_server.enabled", control: "toggle", secret: false },
            { path: "webchat_server.host", control: "text", secret: false },
            { path: "webchat_server.port", control: "number", secret: false },
          ],
        },
        {
          id: "providers",
          title: "Providers",
          fields: [{ path: "chat_providers", control: "list", secret: false }],
        },
      ],
    },
  };
}

function providerCatalogFixture() {
  return {
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
        },
      },
    },
    provider_sources: [{ id: "openai", type: "openai_chat_completion", provider_type: "chat_completion", provider: "openai", enable: true }],
    providers: [{
      id: "openai/gpt-4.1-mini",
      type: "openai_chat_completion",
      provider_type: "chat_completion",
      provider_source_id: "openai",
      provider: "openai",
      model: "gpt-4.1-mini",
      enable: true,
    }],
  };
}

function knowledgeBaseFixture(kbId = "kb-1") {
  return {
    kb_id: kbId,
    name: "Docs",
    description: "Project docs",
    emoji: "books",
    embedding_provider_id: "embedding",
    stats: { doc_count: 1, chunk_count: 1 },
  };
}

function documentFixture(kbId = "kb-1", docId = "doc-1") {
  return {
    doc_id: docId,
    kb_id: kbId,
    name: "Intro",
    file_type: "markdown",
    file_size: 2048,
    chunk_count: 1,
  };
}

function conversationFixture() {
  return {
    user_id: "webchat:FriendMessage:conversation-1",
    cid: "conversation-1",
    platform_id: "webchat",
    title: "Ops chat",
    created_at: 10,
    updated_at: 20,
    history: JSON.stringify([{ role: "user", content: "hello" }, { role: "assistant", content: "world" }]),
  };
}

function projectFixture() {
  return {
    project_id: "proj-1",
    title: "Research",
    emoji: "R",
    description: "Docs",
    updated_at: "unix:3",
  };
}

function projectSessionFixture() {
  return {
    session_id: "project-session-1",
    display_name: "Project chat",
    platform_id: "webchat",
    creator: "user",
    updated_at: "unix:3",
    is_group: false,
  };
}
