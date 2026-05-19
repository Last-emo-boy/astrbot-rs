import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "chat-token");
    window.localStorage.setItem("astrbot.openapiSecret", "ak_openapi");
  });
  await installChatMocks(page);
});

test("chat route supports sidebar history provider config attachments send stop refs and live mode", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "mobile", "mobile coverage is handled by the chatbox smoke");
  await page.goto("/chat/conversation-1");

  await expect(page.locator('[data-page="chat"]')).toBeVisible();
  await expect(page.locator(".chat-sidebar-panel")).toContainText("Ops chat");
  await expect(page.locator(".chat-project-list")).toContainText("Research");
  await expect(page.locator("#chat-config-select")).toHaveValue("ops");
  await expect(page.locator("#chat-provider-select")).toContainText("openai/gpt-4.1-mini");
  await expect(page.locator(".message-markdown")).toContainText("Hello from history");
  await expect(page.locator(".tool-call-compact")).toContainText("web_search");
  await expect(page.locator(".ipython-tool-block")).toContainText("print(1)");

  await page.locator('[data-action="chat-reply"]').first().click();
  await expect(page.locator(".chat-reply-preview")).toContainText("Hello from history");

  await page.locator('[data-action="chat-refs-open"]').first().click();
  await expect(page.locator(".chat-refs-sidebar")).toContainText("Source ref");
  await page.locator('[data-action="chat-refs-close"]').click();

  await page.locator("#chat-attachment-url").fill("https://example.test/image.png");
  await page.locator('[data-action="chat-stage-url"]').click();
  await expect(page.locator(".chat-attachment-chip")).toContainText("image.png");
  await page.locator('[data-action="chat-image-preview"]').first().click();
  await expect(page.locator("#chat-image-preview-dialog")).toBeVisible();
  await page.locator('#chat-image-preview-dialog [data-action="chat-dialog-close"]').click();

  await page.locator("#chat-file-upload").setInputFiles({
    name: "note.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("hello file"),
  });
  await page.locator('[data-action="chat-upload-file"]').click();
  await expect(page.locator("#toast")).toContainText("已上传 1 个附件");

  await page.locator("#chat-text").fill("new message");
  await page.locator('[data-action="chat-config-apply"]').click();
  await expect(page.locator("#toast")).toContainText("会话配置路由已保存");
  await page.locator('[data-action="send-chat"]').click();
  await expect(page.locator(".chat-message-list")).toContainText("new message");

  await page.locator('[data-action="chat-stop"]').click();
  await expect(page.locator("#toast")).toContainText("Stop request");

  await page.locator('[data-action="chat-live-open"]').click();
  await expect(page.locator(".chat-live-mode")).toBeVisible();
  await page.locator('[data-action="chat-live-connect"]').click();
  await expect(page.locator(".chat-live-orb")).toContainText("WebSocket ready");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

test("chatbox deep link supports standalone openapi stream stop elicitation and mobile layout", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/chatbox/conversation-2");

  await expect(page.locator('[data-page="chatbox"]')).toBeVisible();
  await expect(page.locator(".topbar")).toHaveCount(0);
  await expect(page.locator("#conversation-id")).toHaveValue("conversation-2");
  await expect(page.locator(".chat-message-list")).toContainText("ChatBox history");

  await page.locator("#chat-text").fill("stream me");
  await page.locator('[data-action="openapi-stream-chat"]').click();
  await expect(page.locator("#toast")).toContainText("OpenAPI stream request 已提交");
  await expect(page.locator(".chat-openapi-panel")).toContainText("request-1");

  await page.locator('[data-action="openapi-stop-chat"]').click();
  await expect(page.locator("#toast")).toContainText("Stop request 已记录");

  await page.locator('[data-action="openapi-elicitation-create"]').click();
  await expect(page.locator("#toast")).toContainText("Elicitation 已创建");
  await page.locator('[data-action="openapi-elicitation-respond"]').click();
  await expect(page.locator("#toast")).toContainText("Elicitation response 已记录");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});

async function installChatMocks(page) {
  const sessions = [
    { session_id: "conversation-1", display_name: "Ops chat", platform_id: "webchat", updated_at: "unix:2" },
    { session_id: "conversation-2", display_name: "ChatBox session", platform_id: "webchat", updated_at: "unix:1" },
  ];
  const projects = { projects: [{ project_id: "proj-1", title: "Research", emoji: "R", description: "Docs" }] };
  const providerList = [
    {
      id: "openai/gpt-4.1-mini",
      model: "gpt-4.1-mini",
      provider_type: "chat_completion",
      enable: true,
      models: ["gpt-4.1-mini"],
      model_metadata: { modalities: { input: ["text", "image"] }, tool_call: true, reasoning: true },
    },
  ];
  const messagesByConversation = {
    "conversation-1": [
      richMessage("m-1", "Hello from history"),
    ],
    "conversation-2": [
      { id: "m-2", text: "ChatBox history", message_parts: [{ type: "plain", text: "ChatBox history" }] },
    ],
  };
  const realtime = {
    subscriptions: [{ request_id: "request-1", conversation_id: "conversation-2", status: "queued", stop_requested: false }],
    elicitations: [{ elicitation_id: "approval-1", status: "pending", request: { message: "Approve action?" } }],
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    let body = {};
    try {
      body = request.postDataJSON?.() || {};
    } catch {
      body = {};
    }

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0 },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 0, handlers: [] },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "chat", label: "Chat", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 2, platform_counts: [], provider_usage: [], recent_events: [] });
    }
    if (url.pathname === "/api/management/logs") {
      return fulfillJson(route, { snapshot: { entries: [] } });
    }
    if (url.pathname.startsWith("/api/webchat/") && url.pathname.endsWith("/messages")) {
      const conversationId = decodeURIComponent(url.pathname.split("/")[3]);
      return fulfillJson(route, {
        conversation_id: conversationId,
        messages: messagesByConversation[conversationId] || [],
      });
    }
    if (url.pathname.startsWith("/api/webchat/") && request.method() === "POST") {
      const conversationId = decodeURIComponent(url.pathname.split("/")[3]);
      messagesByConversation[conversationId] = [
        ...(messagesByConversation[conversationId] || []),
        {
          id: `sent-${Date.now()}`,
          text: body.text,
          message_parts: [{ type: "plain", text: body.text }, ...(body.message_parts || [])],
        },
      ];
      return fulfillJson(route, { event_id: "webchat-event-1" });
    }
    if (url.pathname === "/api/chat/sessions") {
      return fulfillJson(route, { status: "ok", data: sessions });
    }
    if (url.pathname === "/api/chat/new_session") {
      sessions.unshift({ session_id: "conversation-new", display_name: "New chat", platform_id: "webchat", updated_at: "unix:3" });
      return fulfillJson(route, { status: "ok", data: sessions[0] });
    }
    if (url.pathname === "/api/chat/post_file") {
      return fulfillJson(route, {
        status: "ok",
        data: {
          attachment_id: "attachment-1",
          filename: "note.txt",
          original_name: "note.txt",
          type: "file",
          url: "/api/chat/get_attachment?attachment_id=attachment-1",
        },
      });
    }
    if (url.pathname === "/api/chat/stop") {
      return fulfillJson(route, { status: "ok", data: { stopped: true } });
    }
    if (url.pathname === "/api/chat/update_session_display_name" || url.pathname === "/api/chat/batch_delete_sessions" || url.pathname === "/api/chat/delete_session") {
      return fulfillJson(route, { status: "ok", data: {} });
    }
    if (url.pathname === "/api/management/conversations") {
      return fulfillJson(route, { conversations: sessions.map((session) => ({ conversation_id: session.session_id, title: session.display_name, platform_id: "webchat" })) });
    }
    if (url.pathname === "/api/management/chat-projects") {
      return fulfillJson(route, projects);
    }
    if (url.pathname === "/api/management/chat-projects/sessions") {
      return fulfillJson(route, { sessions: [] });
    }
    if (url.pathname === "/api/config/provider/template") {
      return fulfillJson(route, { status: "ok", data: { providers: providerList, provider_sources: [], config_schema: {} } });
    }
    if (url.pathname === "/api/config/provider/list") {
      return fulfillJson(route, { status: "ok", data: providerList });
    }
    if (url.pathname === "/api/management/config/abconfs") {
      return fulfillJson(route, { info_list: [{ id: "default", name: "default" }, { id: "ops", name: "Ops" }] });
    }
    if (url.pathname === "/api/config/umo_abconf_routes") {
      return fulfillJson(route, { status: "ok", data: { routing: { "webchat:FriendMessage:webchat!dashboard!conversation-1": "ops" }, routes: [] } });
    }
    if (url.pathname === "/api/config/umo_abconf_route/update") {
      return fulfillJson(route, { status: "ok", data: { routing: { [body.umo]: body.conf_id } } });
    }
    if (url.pathname === "/api/openapi/chat") {
      realtime.subscriptions = [{ request_id: body.request_id || "request-1", conversation_id: body.conversation_id, status: "queued", stop_requested: false }];
      return fulfillJson(route, { accepted: true, request_id: realtime.subscriptions[0].request_id, response_mode: "streaming" });
    }
    if (url.pathname === "/api/openapi/chat/subscriptions") {
      return fulfillJson(route, { subscriptions: realtime.subscriptions });
    }
    if (url.pathname.startsWith("/api/openapi/chat/subscriptions/")) {
      return fulfillJson(route, realtime.subscriptions[0]);
    }
    if (url.pathname === "/api/openapi/chat/stop") {
      return fulfillJson(route, { status: "stop_requested", interrupted_events: 1, matched_subscriptions: 1 });
    }
    if (url.pathname === "/api/openapi/elicitation" && request.method() === "GET") {
      return fulfillJson(route, { elicitations: realtime.elicitations });
    }
    if (url.pathname === "/api/openapi/elicitation" && request.method() === "POST") {
      realtime.elicitations = [{ elicitation_id: body.elicitation_id || "approval-1", status: "pending", request: body.request }];
      return fulfillJson(route, realtime.elicitations[0]);
    }
    if (url.pathname === "/api/openapi/elicitation/respond") {
      realtime.elicitations[0].status = "responded";
      realtime.elicitations[0].result = body.result;
      return fulfillJson(route, realtime.elicitations[0]);
    }

    return fulfillJson(route, {});
  });
}

function richMessage(id, text) {
  return {
    id,
    text,
    reasoning: "I am thinking",
    refs: [{ title: "Source ref", content: "Reference text", url: "https://example.test/ref" }],
    agentStats: { token_usage: { output: 12 } },
    message_parts: [
      { type: "plain", text: `# ${text}\n- item` },
      { type: "tool_call", tool_calls: [{ id: "tc-1", name: "web_search", args: { q: "AstrBot" }, result: "{\"ok\":true}", ts: 1, finished_ts: 1.1 }] },
      { type: "tool_call", tool_calls: [{ id: "py-1", name: "astrbot_execute_python", args: { code: "print(1)" }, result: "1", ts: 2, finished_ts: 2.1 }] },
      { type: "elicitation", payload: { elicitation_id: "approval-1", message: "Approve action?" } },
    ],
  };
}

async function fulfillJson(route, payload, status = 200) {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(payload),
  });
}
