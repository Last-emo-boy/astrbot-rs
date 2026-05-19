import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "persona-token");
  });
  await installPersonaMocks(page);
});

test("persona route supports folder tree form drag reorder move clone and preview workflow", async ({ page }) => {
  await page.goto("/persona");

  await expect(page.getByRole("button", { name: "创建 Persona" })).toBeVisible();
  await expect(page.locator(".persona-folder-sidebar")).toBeAttached();
  const opsFolderCard = page.locator('.persona-folder-card[data-drop-folder="ops"]');
  await expect(opsFolderCard).toContainText("Ops");
  await expect(page.locator('[data-drag-type="persona"][data-persona="support"]')).toBeVisible();

  await page.locator('[data-drag-type="persona"][data-persona="support"]').dragTo(opsFolderCard);
  await expect(page.locator("#toast")).toContainText("Persona 已移动");
  await expect(page.locator(".persona-card")).toContainText("support");

  await page.locator('[data-action="persona-preview"][data-persona="support"]').first().click();
  await expect(page.locator("#persona-preview-dialog")).toBeVisible();
  await expect(page.locator("#persona-preview-dialog")).toContainText("Help safely");
  await page.locator('#persona-preview-dialog [aria-label="Close"]').click();

  await page.locator('[data-action="persona-create-open"]').click();
  await expect(page.locator("#persona-form-dialog")).toBeVisible();
  await page.locator("#persona-form-id").fill("incident-lead");
  await page.locator("#persona-form-prompt").fill("Lead incident response with concise updates.");
  await page.locator("#persona-form-error").fill("Cannot provide unsafe guidance.");
  await page.locator('input[name="persona-tools-mode"][value="specific"]').check();
  await page.locator('input[name="persona-skills-mode"][value="specific"]').check();
  await page.locator(".persona-dialog-user").first().fill("Status?");
  await page.locator(".persona-dialog-assistant").first().fill("Green.");
  await page.locator('[data-action="persona-form-save"]').click();
  await expect(page.locator("#toast")).toContainText("人格创建成功");
  await expect(page.locator('[data-drag-type="persona"][data-persona="incident-lead"]')).toBeVisible();

  await page.locator('[data-action="persona-folder-rename-open"][data-folder="incident"]').click();
  await expect(page.locator("#persona-folder-rename-dialog")).toBeVisible();
  await page.locator("#persona-folder-rename-name").fill("Incidents");
  await page.locator('[data-action="persona-folder-rename-submit"]').click();
  await expect(page.locator("#toast")).toContainText("文件夹更新成功");
  await expect(page.locator(".persona-folder-card")).toContainText("Incidents");

  await page.locator('[data-action="persona-clone-open"][data-persona="support"]').click();
  await expect(page.locator("#persona-clone-dialog")).toBeVisible();
  await page.locator("#persona-clone-new-id").fill("support_copy");
  await page.locator('[data-action="persona-clone-submit"]').click();
  await expect(page.locator('[data-drag-type="persona"][data-persona="support_copy"]')).toBeVisible();

  await page.locator('[data-action="persona-rank-down"]:not([disabled])').first().click();
  await expect(page.locator("#toast")).toContainText("顺序已更新");

  await page.locator('[data-action="persona-move-open"][data-persona="incident-lead"]').click();
  await expect(page.locator("#persona-move-dialog")).toBeVisible();
  await page.locator("#persona-move-target").selectOption("");
  await page.locator('[data-action="persona-move-submit"]').click();
  await expect(page.locator("#toast")).toContainText("Persona 已移动");

  await page.locator(".persona-main-search .persona-search-field").fill("support");
  await page.locator('.persona-main-search [data-action="persona-search"]').click();
  await expect(page.locator('[data-drag-type="persona"][data-persona="support"]')).toBeVisible();

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installPersonaMocks(page) {
  const folders = [
    { folder_id: "ops", name: "Ops", parent_id: null, description: "Operations personas", sort_order: 0 },
    { folder_id: "incident", name: "Incident", parent_id: "ops", description: "Incident playbooks", sort_order: 0 },
  ];
  const personas = [
    {
      persona_id: "support",
      system_prompt: "Help safely",
      custom_error_message: null,
      begin_dialogs: ["hello", "hi"],
      tools: null,
      skills: ["writer"],
      folder_id: null,
      sort_order: 0,
    },
  ];

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.method() === "POST" ? JSON.parse(request.postData() || "{}") : {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "openai" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "persona", label: "Persona", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/management/tools") {
      return fulfillJson(route, { tools: [{ name: "diagnostics", description: "Run checks", origin: "plugin", origin_name: "ops", active: true }] });
    }
    if (url.pathname === "/api/management/skills") {
      return fulfillJson(route, { skills: [{ name: "writer", description: "Write updates", active: true }], sandbox_cache: { ready: true } });
    }
    if (url.pathname === "/api/management/mcp/servers") {
      return fulfillJson(route, { servers: [{ name: "docs", tools: ["diagnostics"], active: true }], active_count: 1 });
    }
    if (url.pathname === "/api/persona/folder/tree") {
      return fulfillJson(route, legacyOk(buildTree(folders)));
    }
    if (url.pathname === "/api/persona/folder/list") {
      const parentId = normalizeFolderId(url.searchParams.get("parent_id"));
      return fulfillJson(route, legacyOk(folders.filter((folder) => normalizeFolderId(folder.parent_id) === parentId)));
    }
    if (url.pathname === "/api/persona/list") {
      const hasFolderFilter = url.searchParams.has("folder_id");
      const folderId = normalizeFolderId(url.searchParams.get("folder_id"));
      const rows = hasFolderFilter
        ? personas.filter((persona) => normalizeFolderId(persona.folder_id) === folderId)
        : personas;
      return fulfillJson(route, legacyOk(rows.slice().sort(compareBySortAndId)));
    }
    if (url.pathname === "/api/persona/create") {
      const persona = {
        persona_id: body.persona_id,
        system_prompt: body.system_prompt,
        custom_error_message: body.custom_error_message || null,
        begin_dialogs: body.begin_dialogs || [],
        tools: body.tools ?? null,
        skills: body.skills ?? null,
        folder_id: normalizeFolderId(body.folder_id),
        sort_order: Number(body.sort_order || 0),
      };
      personas.push(persona);
      return fulfillJson(route, legacyOk({ persona }, "人格创建成功"));
    }
    if (url.pathname === "/api/persona/update") {
      const persona = personas.find((item) => item.persona_id === body.persona_id);
      if (persona) Object.assign(persona, body, { folder_id: normalizeFolderId(body.folder_id) });
      return fulfillJson(route, legacyOk({ persona }, "人格更新成功"));
    }
    if (url.pathname === "/api/persona/move") {
      const persona = personas.find((item) => item.persona_id === body.persona_id);
      if (persona) persona.folder_id = normalizeFolderId(body.folder_id);
      return fulfillJson(route, legacyOk({ persona }, "人格移动成功"));
    }
    if (url.pathname === "/api/persona/clone") {
      const source = personas.find((item) => item.persona_id === body.source_persona_id);
      const persona = { ...source, persona_id: body.new_persona_id };
      personas.push(persona);
      return fulfillJson(route, legacyOk({ persona }, "人格克隆成功"));
    }
    if (url.pathname === "/api/persona/delete") {
      const index = personas.findIndex((item) => item.persona_id === body.persona_id);
      if (index >= 0) personas.splice(index, 1);
      return fulfillJson(route, legacyOk({ deleted: index >= 0 }, "人格删除成功"));
    }
    if (url.pathname === "/api/persona/reorder") {
      for (const [index, item] of (body.items || []).entries()) {
        if (item.type === "folder") {
          const folder = folders.find((entry) => entry.folder_id === item.id);
          if (folder) folder.sort_order = index;
        } else {
          const persona = personas.find((entry) => entry.persona_id === item.id);
          if (persona) persona.sort_order = index;
        }
      }
      return fulfillJson(route, legacyOk({ message: "排序更新成功" }, "排序更新成功"));
    }
    if (url.pathname === "/api/persona/folder/create") {
      const folder = {
        folder_id: body.folder_id || slug(body.name),
        name: body.name,
        parent_id: normalizeFolderId(body.parent_id),
        description: body.description || null,
        sort_order: folders.length,
      };
      folders.push(folder);
      return fulfillJson(route, legacyOk({ folder }, "文件夹创建成功"));
    }
    if (url.pathname === "/api/persona/folder/update") {
      const folder = folders.find((item) => item.folder_id === body.folder_id);
      if (folder) {
        if (body.name) folder.name = body.name;
        if ("parent_id" in body) folder.parent_id = normalizeFolderId(body.parent_id);
        if ("description" in body) folder.description = body.description || null;
      }
      return fulfillJson(route, legacyOk({ folder }, "文件夹更新成功"));
    }
    if (url.pathname === "/api/persona/folder/delete") {
      const index = folders.findIndex((folder) => folder.folder_id === body.folder_id);
      if (index >= 0) folders.splice(index, 1);
      for (const persona of personas) {
        if (persona.folder_id === body.folder_id) persona.folder_id = null;
      }
      return fulfillJson(route, legacyOk({ deleted: index >= 0 }, "文件夹删除成功"));
    }
    if (url.pathname === "/api/management/personas/resolve") {
      const persona = personas.find((item) => item.persona_id === body.forced_persona_id);
      return fulfillJson(route, {
        persona_id: persona?.persona_id || "default",
        source: persona ? "forced_session" : "default",
        profile: persona || null,
      });
    }
    return fulfillJson(route, { error: `unhandled ${url.pathname}` }, 404);
  });
}

function buildTree(folders) {
  const visit = (parentId) => folders
    .filter((folder) => normalizeFolderId(folder.parent_id) === parentId)
    .sort(compareBySortAndId)
    .map((folder) => ({ ...folder, children: visit(folder.folder_id) }));
  return visit(null);
}

function compareBySortAndId(left, right) {
  return (left.sort_order || 0) - (right.sort_order || 0)
    || String(left.persona_id || left.folder_id).localeCompare(String(right.persona_id || right.folder_id));
}

function normalizeFolderId(value) {
  const id = String(value || "").trim();
  return id ? id : null;
}

function slug(value) {
  return String(value || "folder").trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "folder";
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
