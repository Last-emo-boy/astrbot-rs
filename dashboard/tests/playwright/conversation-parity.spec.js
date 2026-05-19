import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "conversation-token");
  });
  await installConversationMocks(page);
});

test("conversation route supports source-style filter detail edit export and delete workflow", async ({ page }) => {
  await page.goto("/conversation");

  await expect(page.getByRole("heading", { name: "Conversation History" })).toBeVisible();
  await expect(page.locator("table")).toContainText("Ops room");
  await expect(page.locator("table")).toContainText("telegram:GroupMessage:room-1");

  await page.locator("#conversation-filter-platforms").fill("telegram");
  await page.locator("#conversation-filter-message-type").selectOption("GroupMessage");
  await page.locator("#conversation-filter-search").fill("ops");
  await page.locator('[data-action="conversation-filter-apply"]').click();
  await expect(page.locator("#toast")).toContainText("筛选");
  await expect(page.locator("table")).toContainText("Ops room");

  await page.locator('[data-action="conversation-view"]').first().click();
  await expect(page.locator("#conversation-history-dialog")).toBeVisible();
  await page.locator('[data-action="conversation-history-edit"]').click();
  await page.locator("#conversation-history-editor").fill('[{"role":"user","content":"edited question"},{"role":"assistant","content":"edited answer"}]');
  await page.locator('[data-action="conversation-history-save"]').click();
  await expect(page.locator("#toast")).toContainText("history");
  await expect(page.locator("#conversation-history-dialog")).toContainText("edited answer");
  await page.locator('#conversation-history-dialog [aria-label="Close"]').click();

  await page.locator('[data-action="conversation-edit-open"]').first().click();
  await expect(page.locator("#conversation-edit-dialog")).toBeVisible();
  await page.locator("#conversation-edit-title").fill("Renamed ops");
  await page.locator("#conversation-edit-persona").fill("ops-persona");
  await page.locator('[data-action="conversation-edit-save"]').click();
  await expect(page.locator("table")).toContainText("Renamed ops");

  await page.locator('[data-action="conversation-select"]').first().check();
  await page.locator('[data-action="conversation-export-selected"]').click();
  await expect(page.locator("#toast")).toContainText("导出 1");

  await page.locator('[data-action="conversation-delete-open"]').first().click();
  await expect(page.locator("#conversation-delete-dialog")).toBeVisible();
  await page.locator('[data-action="conversation-delete-confirm"]').click();
  await expect(page.locator("#toast")).toContainText("已删除");

  await page.locator('[data-action="conversation-select-all"]').check();
  await page.locator('[data-action="conversation-batch-delete-open"]').click();
  await expect(page.locator("#conversation-batch-delete-dialog")).toBeVisible();
  await page.locator('[data-action="conversation-batch-delete-confirm"]').click();
  await expect(page.locator("#toast")).toContainText("批量删除");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installConversationMocks(page) {
  let conversations = [
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
    {
      user_id: "telegram:GroupMessage:room-2",
      cid: "conversation-b",
      platform_id: "telegram",
      title: "Ops archive",
      persona_id: "",
      created_at: 11,
      updated_at: 21,
      history: "[]",
    },
  ];

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.method() === "POST" ? JSON.parse(request.postData() || "{}") : {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0 },
        platforms: { platform_count: 2, platform_ids: ["webchat", "telegram"], webchat_platform_count: 1 },
        plugins: { handler_count: 0, handlers: [] },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "conversation", label: "Conversation", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 2, platform_counts: [], provider_usage: [], recent_events: [] });
    }
    if (url.pathname === "/api/management/logs") {
      return fulfillJson(route, { snapshot: { entries: [] } });
    }
    if (url.pathname === "/api/conversation/list") {
      const search = (url.searchParams.get("search") || "").toLowerCase();
      const platforms = (url.searchParams.get("platforms") || "").split(",").filter(Boolean);
      const messageTypes = (url.searchParams.get("message_types") || "").split(",").filter(Boolean);
      const filtered = conversations.filter((conversation) => {
        if (platforms.length && !platforms.includes(conversation.platform_id)) return false;
        if (messageTypes.length && !messageTypes.some((type) => conversation.user_id.includes(`:${type}:`))) return false;
        if (search && !`${conversation.title} ${conversation.user_id} ${conversation.cid} ${conversation.history}`.toLowerCase().includes(search)) return false;
        return true;
      });
      return fulfillJson(route, {
        status: "ok",
        message: "",
        data: {
          conversations: filtered,
          pagination: { page: 1, page_size: 20, total: filtered.length, total_pages: 1 },
        },
      });
    }
    if (url.pathname === "/api/conversation/detail") {
      return fulfillJson(route, {
        status: "ok",
        message: "",
        data: conversations.find((conversation) => conversation.user_id === body.user_id && conversation.cid === body.cid),
      });
    }
    if (url.pathname === "/api/conversation/update_history") {
      const target = conversations.find((conversation) => conversation.user_id === body.user_id && conversation.cid === body.cid);
      target.history = JSON.stringify(body.history || []);
      return fulfillJson(route, { status: "ok", message: "", data: { message: "history updated" } });
    }
    if (url.pathname === "/api/conversation/update") {
      const target = conversations.find((conversation) => conversation.user_id === body.user_id && conversation.cid === body.cid);
      target.title = body.title;
      target.persona_id = body.persona_id;
      return fulfillJson(route, { status: "ok", message: "", data: { message: "updated" } });
    }
    if (url.pathname === "/api/conversation/delete") {
      const targets = body.conversations || [body];
      const before = conversations.length;
      conversations = conversations.filter((conversation) => !targets.some((target) => target.user_id === conversation.user_id && target.cid === conversation.cid));
      return fulfillJson(route, {
        status: "ok",
        message: "",
        data: { deleted_count: before - conversations.length, failed_count: 0, failed_items: [] },
      });
    }
    if (url.pathname === "/api/conversation/export") {
      return route.fulfill({
        status: 200,
        contentType: "application/jsonl; charset=utf-8",
        headers: { "content-disposition": "attachment; filename=\"conversations.jsonl\"" },
        body: conversations.map((conversation) => JSON.stringify(conversation)).join("\n"),
      });
    }
    return fulfillJson(route, { error: `unhandled ${url.pathname}` }, 404);
  });
}

function fulfillJson(route, data, status = 200) {
  return route.fulfill({
    status,
    contentType: "application/json; charset=utf-8",
    body: JSON.stringify(data),
  });
}
