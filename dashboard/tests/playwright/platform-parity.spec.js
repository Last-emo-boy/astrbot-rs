import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "platform-token");
  });
  await installPlatformMocks(page);
});

test("platform route supports source-style templates config binding routes stats and console", async ({ page }) => {
  await page.goto("/platforms");

  await expect(page.getByRole("heading", { name: "平台适配器" })).toBeVisible();
  await expect(page.locator(".platform-card", { hasText: "onebot-main" })).toContainText("2 条路由");
  await expect(page.locator(".platform-card", { hasText: "onebot-main" })).toContainText("Webhook");

  await page.locator('[data-action="platform-console-toggle"]').click();
  await expect(page.locator(".platform-console")).toContainText("platform ready");
  await expect.poll(() => page.evaluate(() => window.localStorage.getItem("platformPage_showConsole"))).toBe("true");

  await page.locator('[data-action="platform-error-open"][data-platform="onebot-main"]').click();
  await expect(page.locator("#platform-error-dialog")).toContainText("boom");
  await page.locator('#platform-error-dialog [data-action="platform-error-close"]').click();

  await page.locator('[data-action="platform-webhook-open"]').click();
  await expect(page.locator("#platform-webhook-url")).toHaveValue("https://bot.example/api/platform/webhook/uuid-1");
  await page.locator('#platform-webhook-dialog [data-action="platform-webhook-close"]').click();

  await page.locator('[data-action="platform-dialog-open"]').click();
  await expect(page.locator("#platform-add-dialog")).toBeVisible();
  await expect(page.locator(".platform-template-card.active")).toContainText("Console");
  await page.locator('[data-action="platform-template-select"][data-template="OneBot"]').click();
  await expect(page.locator("#platform-new-type")).toHaveValue("onebot");
  await page.locator('[data-action="platform-template-select"][data-template="Console"]').click();
  await page.locator("#platform-new-id").fill("console-ops");
  await page.locator('[data-action="platform-config-section-toggle"]').click();
  await page.locator("#platform-config-select").selectOption("ops");
  await page.locator('#platform-add-dialog [data-action="platform-save-new"]').click();
  await expect(page.locator(".platform-card", { hasText: "console-ops" })).toBeVisible();
  await expect(page.locator(".platform-card", { hasText: "console-ops" })).toContainText("1 条路由");

  await page.locator('[data-action="platform-edit-open"][data-platform="onebot-main"]').click();
  await expect(page.locator("#platform-edit-dialog")).toBeVisible();
  await page.locator('[data-action="platform-route-edit-toggle"]').click();
  await expect(page.locator("#platform-route-0-message-type")).toBeVisible();
  await page.locator("#platform-route-0-session-id").fill("room-*");
  await page.locator('[data-action="platform-route-add"]').click();
  await expect(page.locator("#platform-route-2-session-id")).toBeVisible();
  await page.locator("#platform-route-2-session-id").fill("new-room");
  await page.locator('[data-action="platform-route-up"][data-index="2"]').click();
  await expect(page.locator("#platform-route-1-session-id")).toHaveValue("new-room");
  await page.locator('[data-action="platform-route-delete"][data-index="2"]').click();
  await expect(page.locator("#platform-route-2-session-id")).toHaveCount(0);
  await page.locator('#platform-edit-dialog [data-action="platform-save-edit"]').click();
  await expect(page.locator("#platform-edit-dialog")).toBeHidden();

  await page.locator('[data-action="platform-check"][data-platform="onebot-main"]').click();
  await expect(page.locator("#toast")).toContainText("检查完成");
  await expect(page.locator(".platform-card", { hasText: "onebot-main" })).toContainText("配置可构建");

  await page.locator('[data-action="platform-toggle"][data-platform="console-ops"]').click();
  await expect(page.locator(".platform-card", { hasText: "console-ops" })).toContainText("已停用");
  await page.locator('[data-action="platform-delete"][data-platform="console-ops"]').click();
  await expect(page.locator(".platform-card", { hasText: "console-ops" })).toHaveCount(0);

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installPlatformMocks(page) {
  const catalog = {
    summary: {
      platform_count: 1,
      platform_ids: ["onebot-main"],
      mock_platform_count: 0,
      webchat_platform_count: 0,
      onebot_platform_count: 1,
      recording_sink_count: 1,
    },
    templates: [
      { platform_type: "console", label: "Console", runtime_supported: true },
      { platform_type: "onebot", label: "OneBot", runtime_supported: true },
    ],
  };
  const config = {
    callback_api_base: "https://bot.example",
    platforms: [
      {
        id: "onebot-main",
        type: "onebot",
        name: "OneBot Main",
        enabled: true,
        options: { webhook_uuid: "uuid-1", ws_reverse_port: 6199 },
        secrets: { ws_reverse_token: "secret" },
      },
    ],
  };
  const abconfs = [
    { id: "default", name: "Default" },
    { id: "ops", name: "Ops" },
  ];
  let routing = {
    "onebot-main:*:*": "default",
    "onebot-main:GroupMessage:ops-*": "ops",
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.postDataJSON?.() || {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0 },
        platforms: catalog.summary,
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "platforms", label: "Platforms", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, {
        total_messages: 12,
        platform_counts: [{ platform_id: "onebot-main", platform_type: "onebot", count: 12 }],
        provider_usage: [],
        recent_events: [],
      });
    }
    if (url.pathname === "/api/management/platforms/catalog") {
      catalog.platforms = config.platforms.map(({ id, type, enabled, name }) => ({ id, type, enabled, name }));
      catalog.summary = {
        ...catalog.summary,
        platform_count: config.platforms.length,
        platform_ids: config.platforms.map((platform) => platform.id),
        onebot_platform_count: config.platforms.filter((platform) => platform.type === "onebot").length,
        webchat_platform_count: config.platforms.filter((platform) => platform.type === "webchat").length,
      };
      return fulfillJson(route, catalog);
    }
    if (url.pathname === "/api/config/get") {
      return fulfillJson(route, { status: "ok", data: { config, metadata: {} } });
    }
    if (url.pathname === "/api/config/abconfs") {
      return fulfillJson(route, { status: "ok", data: { info_list: abconfs } });
    }
    if (url.pathname === "/api/config/umo_abconf_routes") {
      return fulfillJson(route, { status: "ok", data: { routing, routes: Object.entries(routing).map(([pattern, config_id]) => ({ pattern, config_id })) } });
    }
    if (url.pathname === "/api/platform/stats") {
      return fulfillJson(route, {
        status: "ok",
        data: {
          platforms: [
            {
              id: "onebot-main",
              status: "error",
              error_count: 1,
              unified_webhook: true,
              last_error: { message: "boom", timestamp: "2026-05-18T10:00:00Z", traceback: "stack" },
            },
          ],
        },
      });
    }
    if (url.pathname === "/api/management/logs") {
      return fulfillJson(route, { snapshot: { entries: [{ timestamp: "2026-05-18T10:00:00Z", level: "INFO", message: "platform ready" }] } });
    }
    if (url.pathname === "/api/config/default") {
      return fulfillJson(route, { status: "ok", data: { config: { platforms: [] }, metadata: {} } });
    }
    if (url.pathname === "/api/config/abconf/new") {
      const confId = "ops-new";
      abconfs.push({ id: confId, name: body.name || confId });
      return fulfillJson(route, { status: "ok", data: { conf_id: confId, abconf: { id: confId, name: body.name || confId, config: body.config || {} } } });
    }
    if (url.pathname === "/api/config/umo_abconf_route/update") {
      routing[body.umo] = body.conf_id;
      return fulfillJson(route, { status: "ok", data: { routing } });
    }
    if (url.pathname === "/api/config/umo_abconf_route/update_all") {
      routing = body.routing || {};
      return fulfillJson(route, { status: "ok", data: { routing } });
    }
    if (url.pathname === "/api/config/platform/new") {
      const platform = { ...body, enabled: body.enabled ?? body.enable ?? true };
      config.platforms = [platform, ...config.platforms.filter((item) => item.id !== platform.id)];
      return fulfillJson(route, { status: "ok", data: { changed: true, platform } });
    }
    if (url.pathname === "/api/config/platform/update") {
      const platform = { ...(body.config || {}), enabled: body.config?.enabled ?? body.config?.enable ?? true };
      config.platforms = [platform, ...config.platforms.filter((item) => item.id !== (body.id || platform.id) && item.id !== platform.id)];
      return fulfillJson(route, { status: "ok", data: { changed: true, platform } });
    }
    if (url.pathname === "/api/config/platform/delete") {
      config.platforms = config.platforms.filter((platform) => platform.id !== body.id);
      return fulfillJson(route, { status: "ok", data: { changed: true } });
    }
    if (url.pathname === "/api/management/platforms/upsert") {
      const platform = { ...body.platform };
      config.platforms = [platform, ...config.platforms.filter((item) => item.id !== platform.id)];
      return fulfillJson(route, { changed: true, catalog });
    }
    if (url.pathname === "/api/management/platforms/check") {
      return fulfillJson(route, { ok: true, platform_id: body.id || body.platform?.id, message: "platform configuration can be built" });
    }
    if (url.pathname === "/api/management/platforms/delete") {
      config.platforms = config.platforms.filter((platform) => platform.id !== body.id);
      return fulfillJson(route, { changed: true, catalog });
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
