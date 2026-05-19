import { expect, test } from "@playwright/test";

const status = {
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
  plugins: { handler_count: 2 },
};

const capabilities = { services: [] };
const stats = { total_messages: 0, uptime_seconds: 60, platform_counts: [], provider_usage: [] };

const schema = {
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

const currentConfig = {
  webchat_server: { enabled: true, host: "127.0.0.1", port: 6185 },
  chat_providers: [{ id: "openai" }],
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "test-token");
  });
  await page.route("**/api/management/status", (route) => route.fulfill({ json: status }));
  await page.route("**/api/management/dashboard/capabilities", (route) => route.fulfill({ json: capabilities }));
  await page.route("**/api/management/stats", (route) => route.fulfill({ json: stats }));
  await page.route("**/api/management/config/schema", (route) => route.fulfill({ json: schema }));
  await page.route("**/api/management/config/current", (route) => route.fulfill({ json: { config: currentConfig } }));
  await page.route("**/api/management/config/abconfs", (route) => route.fulfill({
    json: { info_list: [{ id: "default", name: "default" }, { id: "ops", name: "Ops" }] },
  }));
  await page.route("**/api/management/config/abconfs/get", (route) => route.fulfill({
    json: { abconf: { id: "ops", name: "Ops", config: { ...currentConfig, webchat_server: { ...currentConfig.webchat_server, port: 7001 } } } },
  }));
  await page.route("**/api/management/config/routes", (route) => route.fulfill({
    json: { routes: [{ pattern: "webchat:group:ops-*", config_id: "ops" }] },
  }));
  await page.route("**/api/management/config/routes/replace", (route) => route.fulfill({
    json: { changed: true, routes: [{ pattern: "webchat:group:ops-*", config_id: "ops" }] },
  }));
  await page.route("**/api/management/config/preview", (route) => route.fulfill({
    json: {
      config: currentConfig,
      plan: { changed_fields: ["webchat_server"], reload_action: "restart_runtime" },
    },
  }));
  await page.route("**/api/management/config/apply", (route) => route.fulfill({
    json: {
      config: currentConfig,
      plan: { changed_fields: ["webchat_server"], reload_action: "restart_runtime" },
      execution: { accepted: true },
    },
  }));
  await page.route("**/api/t2i/templates/active", (route) => route.fulfill({
    json: { status: "ok", data: { active_template: "base" } },
  }));
  await page.route("**/api/t2i/templates/create", (route) => route.fulfill({
    status: 201,
    json: { status: "ok", data: { name: "custom_template" }, message: "created" },
  }));
  await page.route("**/api/t2i/templates/set_active", (route) => route.fulfill({
    json: { status: "ok", data: { active_template: "base" }, message: "active" },
  }));
  await page.route("**/api/t2i/templates/reset_default", (route) => route.fulfill({
    json: { status: "ok", data: { active_template: "base" }, message: "reset" },
  }));
  await page.route("**/api/t2i/templates/base", (route) => route.fulfill({
    json: { status: "ok", data: { name: "base", content: "<main>{{ text | safe }} {{ version }}</main>" } },
  }));
  await page.route("**/api/t2i/templates", (route) => route.fulfill({
    json: { status: "ok", data: [{ name: "base", is_default: true, active: true }] },
  }));
});

test("config route supports source-style ABConf, unsaved switch, UMO and T2I workflow", async ({ page }) => {
  await page.goto("/config");

  await expect(page.locator('[data-page="config"]')).toBeVisible();
  await expect(page.locator("text=ABConf")).toBeVisible();
  await expect(page.locator("text=UMOP Routes")).toBeVisible();
  await expect(page.locator("#t2i-template-editor").getByRole("heading", { name: "T2I Template Editor" })).toBeVisible();

  await page.fill("#config-editor", JSON.stringify({ ...currentConfig, webchat_server: { ...currentConfig.webchat_server, port: 7002 } }, null, 2));
  await page.click('[data-action="config-mode-system"]');
  await expect(page.locator("#config-unsaved-dialog")).toBeVisible();
  await page.click('[data-action="config-unsaved-discard"]', { force: true });
  await expect(page.locator('[data-config-mode="system"]')).toBeVisible();

  await page.click('[data-action="t2i-template-new"]');
  await expect(page.locator("#t2i-template-content")).toHaveValue(/{{ text \| safe }}/);
  await page.fill("#t2i-template-content", "<main>{{ version }}</main>");
  await page.click('[data-action="t2i-template-save"]');
  await expect(page.locator("#toast")).toContainText("T2I 模板已保存");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});
