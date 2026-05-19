import { expect, test } from "@playwright/test";

test("protected source route uses blank login layout with returnUrl", async ({ page }) => {
  await page.goto("/providers");

  await expect(page.locator("body")).toHaveAttribute("data-layout", "blank");
  await expect(page.locator(".login-panel")).toBeVisible();
  await expect(page.locator(".sidebar")).toBeHidden();
  await expect(page.locator("text=Return URL: /providers")).toBeVisible();

  const screenshot = await page.screenshot();
  expect(screenshot.length).toBeGreaterThan(2_000);
});

test("chatbox detail route remains a public blank layout on desktop and mobile", async ({ page }) => {
  await page.goto("/chatbox/conversation-1");

  await expect(page.locator("body")).toHaveAttribute("data-layout", "blank");
  await expect(page.locator(".sidebar")).toBeHidden();
  await expect(page.locator("#conversation-id")).toHaveValue("conversation-1");
});

test("legacy static aliases route to guarded replacement pages instead of dead links", async ({ page }) => {
  await page.goto("/logs");
  await expect(page.locator("text=Return URL: /logs")).toBeVisible();

  await page.goto("/tool-use");
  await expect(page.locator("text=Return URL: /tool-use")).toBeVisible();

  await page.goto("/extension");
  await expect(page.locator("text=Return URL: /extension")).toBeVisible();
});

test("legacy Alkaid disabled routes show explicit replacement panel", async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "playwright-token");
  });
  await installManagementApiMocks(page);

  for (const legacyPath of ["/alkaid/long-term-memory", "/alkaid/other"]) {
    await page.goto(legacyPath);
    await expect(page.locator('[data-page="legacy-alkaid-replacement"]')).toBeVisible();
    await expect(page.locator("text=legacy Alkaid plugin UI 不在 RS Dashboard runtime parity 范围")).toBeVisible();
    await expect(page.locator('a[href="#/knowledge-base"]')).toBeVisible();
    await expect(page.locator('a[href="#/alkaid/knowledge-base"]')).toBeVisible();
  }
});

test("about route exposes brand links changelog and static logo asset", async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "playwright-token");
  });
  await installManagementApiMocks(page);

  await page.goto("/about");

  await expect(page.locator('[data-page="about"]')).toBeVisible();
  await expect(page.locator(".about-logo")).toBeVisible();
  await expect(page.locator('a[href="https://github.com/AstrBotDevs/AstrBot"]')).toBeVisible();
  await expect(page.locator('a[href="https://github.com/AstrBotDevs/AstrBot/issues"]')).toBeVisible();
  await expect(page.locator("text=AGPL v3")).toBeVisible();
  await expect(page.locator("text=v4.0.0")).toBeVisible();
  await expect.poll(async () => page.locator(".about-logo").evaluate((image) => image.naturalWidth)).toBeGreaterThan(0);

  await page.locator('[data-action="settings-open-changelog"]').click();
  await expect(page.locator("#changelog-dialog")).toBeVisible();
  await expect(page.locator("#changelog-dialog")).toContainText("Dashboard parity");
});

test("config system route supports unsaved edits, UMO batch routes, and T2I template workflow", async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "playwright-token");
  });
  await installManagementApiMocks(page);

  await page.goto("/system");

  await expect(page.locator(".config-page")).toHaveAttribute("data-config-mode", "system");
  await expect(page.locator('[data-action="config-mode-system"]')).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#t2i-template-editor")).toBeVisible();

  await page.locator("#config-editor").fill(JSON.stringify({
    webchat_server: { enabled: true, host: "127.0.0.1", port: 7002 },
    chat_providers: [],
  }, null, 2));
  await page.locator('[data-action="config-mode-normal"]').click();
  await expect(page.locator("#config-unsaved-dialog")).toBeVisible();
  await page.locator('[data-action="config-unsaved-save"]').click({ force: true });
  await expect(page.locator(".config-page")).toHaveAttribute("data-config-mode", "normal");

  await page.locator("#config-routes-json").fill(JSON.stringify({
    "webchat:group:ops-*": "ops",
  }, null, 2));
  await page.locator('[data-action="config-route-replace"]').click();
  await expect(page.locator("#config-routes-table")).toContainText("webchat:group:ops-*");

  await page.locator("#t2i-template-new-name").fill("custom_template");
  await page.locator('[data-action="t2i-template-new"]').click();
  await page.locator("#t2i-template-content").fill("<main>{{ text | safe }} {{ version }}</main>");
  await page.locator('[data-action="t2i-template-save"]').click();
  await expect(page.locator("#t2i-template-select")).toContainText("custom_template");
  await page.locator('[data-action="t2i-template-apply"]').click();
  await expect(page.locator("#t2i-template-editor")).toContainText("active: custom_template");
});

async function installManagementApiMocks(page) {
  const currentConfig = {
    webchat_server: { enabled: true, host: "127.0.0.1", port: 6185 },
    chat_providers: [],
  };
  const abconfs = [
    { id: "default", name: "default" },
    { id: "ops", name: "Ops" },
  ];
  let routes = [{ pattern: "webchat:group:room-*", config_id: "ops" }];
  let activeTemplate = "base";
  const templates = new Map([
    ["base", "<main>{{ text | safe }} {{ version }}</main>"],
  ]);

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.postDataJSON?.() || {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "mock" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 0 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/management/update/check") {
      return fulfillJson(route, {
        check: {
          current_version: "v4.0.0",
          latest_version: "v4.0.0",
          has_new_version: false,
          dashboard_version: "v4.0.1",
          dashboard_has_new_version: true,
        },
      });
    }
    if (url.pathname === "/api/management/update/releases") {
      return fulfillJson(route, { releases: [{ version: "v4.0.1", title: "Dashboard parity" }] });
    }
    if (url.pathname === "/api/management/update/changelog") {
      return fulfillJson(route, {
        current_version: "v4.0.0",
        releases: [{ version: "v4.0.1", title: "Dashboard parity", body: "# Dashboard parity\n- About page" }],
      });
    }
    if (url.pathname === "/api/management/update/migration-check") {
      return fulfillJson(route, { check: { pending_storage_migrations: [], legacy_data_migration_needed: false } });
    }
    if (url.pathname === "/api/management/config/schema") {
      return fulfillJson(route, configSchemaFixture());
    }
    if (url.pathname === "/api/management/config/abconfs") {
      return fulfillJson(route, { info_list: abconfs });
    }
    if (url.pathname === "/api/management/config/current") {
      return fulfillJson(route, { config: currentConfig });
    }
    if (url.pathname === "/api/management/config/abconfs/get") {
      return fulfillJson(route, { abconf: { id: body.id || "ops", name: "Ops", config: currentConfig } });
    }
    if (url.pathname === "/api/management/config/apply" || url.pathname === "/api/management/config/preview") {
      return fulfillJson(route, {
        config: body.config || currentConfig,
        plan: { changed_fields: ["webchat_server"], reload_action: "restart_runtime" },
        execution: { accepted: true, requested: true, action: "restart_runtime", message: "ok" },
      });
    }
    if (url.pathname === "/api/management/config/routes") {
      return fulfillJson(route, { routes });
    }
    if (url.pathname === "/api/management/config/routes/replace") {
      routes = body.routes || [];
      return fulfillJson(route, { changed: true, routes });
    }
    if (url.pathname === "/api/t2i/templates") {
      return fulfillJson(route, legacyOk(Array.from(templates.keys()).map((name) => ({
        name,
        source: name === "base" ? "builtin" : "user",
        is_default: name === "base",
        active: name === activeTemplate,
      }))));
    }
    if (url.pathname === "/api/t2i/templates/active") {
      return fulfillJson(route, legacyOk({ active_template: activeTemplate }));
    }
    if (url.pathname === "/api/t2i/templates/create") {
      templates.set(body.name, body.content);
      return fulfillJson(route, legacyOk({ name: body.name }, "created"), 201);
    }
    if (url.pathname === "/api/t2i/templates/set_active") {
      activeTemplate = body.name;
      return fulfillJson(route, legacyOk({ active_template: activeTemplate }));
    }
    const templateMatch = url.pathname.match(/^\/api\/t2i\/templates\/([^/]+)$/);
    if (templateMatch && request.method() === "GET") {
      const name = decodeURIComponent(templateMatch[1]);
      return fulfillJson(route, legacyOk({ name, content: templates.get(name) || "" }));
    }
    if (templateMatch && request.method() === "PUT") {
      const name = decodeURIComponent(templateMatch[1]);
      templates.set(name, body.content || "");
      return fulfillJson(route, legacyOk({ name }));
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

function legacyOk(data, message = "") {
  return { status: "ok", message, data };
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
