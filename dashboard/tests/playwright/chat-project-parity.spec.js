import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "chat-project-token");
    window.localStorage.setItem("projectsExpanded", "true");
  });
  await installChatProjectMocks(page);
});

test("chat project sidebar supports create edit membership remove and delete workflow", async ({ page }) => {
  await page.goto("/chat");

  await expect(page.locator('[data-page="chat"]')).toBeVisible();
  await expect(page.locator(".chat-project-list")).toContainText("Research");

  await page.locator(".chat-project-create").click();
  await expect(page.locator("#chat-project-dialog")).toBeVisible();
  await page.locator("#project-emoji").fill("P");
  await page.locator("#project-title").fill("Playwright Project");
  await page.locator("#project-description").fill("Project workflow");
  await page.locator('[data-action="project-dialog-save"]').click();
  await expect(page.locator("#toast")).toContainText("Chat 项目已创建");
  await expect(page.locator(".chat-project-list")).toContainText("Playwright Project");

  await page.locator('[data-action="chat-project-dialog-open"][data-mode="edit"][data-project="proj-2"]').click();
  await expect(page.locator("#chat-project-dialog")).toBeVisible();
  await page.locator("#project-title").fill("Edited Project");
  await page.locator("#project-description").fill("Edited workflow");
  await page.locator('[data-action="project-dialog-save"]').click();
  await expect(page.locator("#toast")).toContainText("Chat 项目已更新");
  await expect(page.locator(".chat-project-list")).toContainText("Edited Project");

  await page.locator('[data-action="project-select"][data-project="proj-2"]').click();
  await expect(page.locator('[data-project-view="proj-2"]')).toBeVisible();
  await expect(page.locator(".chat-project-view")).toContainText("Edited workflow");
  await page.locator("#chat-text").fill("first project prompt");
  await page.locator('[data-action="send-chat"]').click();
  await expect(page.locator(".chat-message-list")).toContainText("first project prompt");
  await expect(page.locator(".chat-breadcrumb")).toContainText("Edited Project");

  await page.locator('[data-action="project-select"][data-project="proj-2"]').click();
  await expect(page.locator('[data-project-view="proj-2"]')).toBeVisible();
  await expect(page.locator(".chat-project-session-list")).toContainText("first project prompt");
  await page.locator('[data-action="project-session-select"][data-session="conversation-new"]').click();
  await expect(page.locator(".chat-message-list")).toContainText("first project prompt");
  await expect(page.locator(".chat-breadcrumb")).toContainText("Edited Project");

  await page.locator('[data-action="project-select"][data-project="proj-2"]').click();
  await page.locator('[data-action="project-session-remove"][data-session="conversation-new"]').click();
  await expect(page.locator("#toast")).toContainText("会话已移出项目");
  await expect(page.locator(".chat-project-session-list")).toContainText("暂无项目会话");

  await page.locator('.chat-project-list [data-action="project-delete"][data-project="proj-2"]').click();
  await expect(page.locator("#toast")).toContainText("Chat 项目已删除");
  await expect(page.locator(".chat-project-list")).not.toContainText("Edited Project");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installChatProjectMocks(page) {
  let nextProject = 2;
  const projects = [
    {
      project_id: "proj-1",
      title: "Research",
      emoji: "R",
      description: "Docs",
      created_at: "2026-05-19T00:00:00Z",
      updated_at: "2026-05-19T00:00:00Z",
    },
  ];
  const sessions = [
    {
      session_id: "conversation-1",
      display_name: "General chat",
      platform_id: "webchat",
      creator: "user",
      is_group: false,
      created_at: "2026-05-19T00:00:00Z",
      updated_at: "2026-05-19T00:00:00Z",
    },
  ];
  const projectMembers = new Map();
  const messagesByConversation = {
    "conversation-1": [plainMessage("m-1", "General history")],
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    let body = {};
    try {
      body = request.method() === "POST" ? JSON.parse(request.postData() || "{}") : {};
    } catch {
      body = {};
    }

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "mock" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 0, handlers: [] },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "chat-projects", label: "Chat Projects", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [], recent_events: [] });
    }
    if (url.pathname === "/api/management/logs" || url.pathname === "/api/log-history") {
      return fulfillJson(route, url.pathname === "/api/log-history" ? legacyOk({ logs: [] }) : { snapshot: { entries: [] } });
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
        plainMessage(`sent-${Date.now()}`, body.text || ""),
      ];
      touchSession(sessions, conversationId, body.text || conversationId);
      return fulfillJson(route, { event_id: "webchat-event-1" });
    }
    if (url.pathname === "/api/chat/sessions") {
      const memberIds = new Set([...projectMembers.values()].flat());
      return fulfillJson(route, legacyOk(sessions.filter((session) => !memberIds.has(session.session_id))));
    }
    if (url.pathname === "/api/chat/new_session") {
      const session = {
        session_id: "conversation-new",
        display_name: "New chat",
        platform_id: "webchat",
        creator: "user",
        is_group: false,
        created_at: "2026-05-19T00:01:00Z",
        updated_at: "2026-05-19T00:01:00Z",
      };
      if (!sessions.some((item) => item.session_id === session.session_id)) {
        sessions.unshift(session);
      }
      return fulfillJson(route, legacyOk(session));
    }
    if (url.pathname === "/api/chat/get_session") {
      const sessionId = url.searchParams.get("session_id") || "conversation-1";
      const project = projectForSession(projects, projectMembers, sessionId);
      return fulfillJson(route, legacyOk({
        session_id: sessionId,
        history: messagesByConversation[sessionId] || [],
        project,
        is_running: false,
      }));
    }
    if (url.pathname === "/api/chat/update_session_display_name" || url.pathname === "/api/chat/batch_delete_sessions" || url.pathname === "/api/chat/delete_session" || url.pathname === "/api/chat/stop") {
      return fulfillJson(route, legacyOk({}));
    }
    if (url.pathname === "/api/management/conversations") {
      return fulfillJson(route, {
        conversations: sessions.map((session) => ({
          conversation_id: session.session_id,
          title: session.display_name,
          platform_id: "webchat",
        })),
      });
    }
    if (url.pathname === "/api/management/chat-projects") {
      return fulfillJson(route, { projects });
    }
    if (url.pathname === "/api/management/chat-projects/create") {
      const now = body.now || "2026-05-19T00:02:00Z";
      const project = {
        project_id: `proj-${nextProject++}`,
        creator: body.creator || "user",
        title: body.title,
        emoji: body.emoji || "📁",
        description: body.description || "",
        created_at: now,
        updated_at: now,
      };
      projects.push(project);
      return fulfillJson(route, { project });
    }
    if (url.pathname === "/api/management/chat-projects/update") {
      const project = projects.find((item) => item.project_id === body.project_id);
      if (project) {
        project.title = body.title ?? project.title;
        project.emoji = body.emoji ?? project.emoji;
        project.description = body.description ?? project.description;
        project.updated_at = body.now || project.updated_at;
      }
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/chat-projects/delete") {
      const index = projects.findIndex((item) => item.project_id === body.project_id);
      if (index >= 0) projects.splice(index, 1);
      projectMembers.delete(body.project_id);
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/chat-projects/sessions/upsert") {
      const session = touchSession(sessions, body.session_id, body.display_name || body.session_id);
      session.creator = body.creator || session.creator || "user";
      session.platform_id = body.platform_id || session.platform_id || "webchat";
      session.is_group = Boolean(body.is_group);
      session.updated_at = body.now || session.updated_at;
      return fulfillJson(route, { session });
    }
    if (url.pathname === "/api/management/chat-projects/add-session") {
      const ids = new Set(projectMembers.get(body.project_id) || []);
      ids.add(body.session_id);
      projectMembers.set(body.project_id, [...ids]);
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/chat-projects/remove-session") {
      for (const [projectId, ids] of projectMembers.entries()) {
        projectMembers.set(projectId, ids.filter((id) => id !== body.session_id));
      }
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/chat-projects/sessions") {
      const ids = new Set(projectMembers.get(body.project_id) || []);
      return fulfillJson(route, { sessions: sessions.filter((session) => ids.has(session.session_id)) });
    }
    if (url.pathname === "/api/config/provider/template") {
      return fulfillJson(route, { status: "ok", data: { providers: [], provider_sources: [], config_schema: {} } });
    }
    if (url.pathname === "/api/config/provider/list") {
      return fulfillJson(route, legacyOk([]));
    }
    if (url.pathname === "/api/management/config/abconfs") {
      return fulfillJson(route, { info_list: [{ id: "default", name: "default" }] });
    }
    if (url.pathname === "/api/config/umo_abconf_routes") {
      return fulfillJson(route, legacyOk({ routing: {}, routes: [] }));
    }

    return fulfillJson(route, { error: `unhandled ${request.method()} ${url.pathname}` }, 404);
  });
}

function touchSession(sessions, sessionId, displayName) {
  let session = sessions.find((item) => item.session_id === sessionId);
  if (!session) {
    session = {
      session_id: sessionId,
      display_name: displayName,
      platform_id: "webchat",
      creator: "user",
      is_group: false,
      created_at: "2026-05-19T00:03:00Z",
      updated_at: "2026-05-19T00:03:00Z",
    };
    sessions.unshift(session);
  }
  session.display_name = displayName || session.display_name || sessionId;
  return session;
}

function projectForSession(projects, projectMembers, sessionId) {
  for (const [projectId, ids] of projectMembers.entries()) {
    if (ids.includes(sessionId)) {
      return projects.find((project) => project.project_id === projectId) || null;
    }
  }
  return null;
}

function plainMessage(id, text) {
  return {
    id,
    text,
    created_at: "2026-05-19T00:04:00Z",
    message_parts: [{ type: "plain", text }],
  };
}

function fulfillJson(route, data, status = 200) {
  return route.fulfill({
    status,
    contentType: "application/json; charset=utf-8",
    body: JSON.stringify(data),
  });
}

function legacyOk(data, message = "") {
  return { status: "ok", message, data };
}
