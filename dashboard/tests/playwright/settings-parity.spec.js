import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "settings-token");
    window.localStorage.setItem("astrbot.locale", "en-US");
    window.localStorage.setItem("astrbot-locale", "en-US");
  });
  await installSettingsMocks(page);
});

test("settings route supports source-style API key backup migration and changelog workflows", async ({ page }) => {
  await page.goto("/settings");

  await expect(page.locator('[data-page="settings"]')).toBeVisible();
  await expect(page.getByRole("heading", { name: "Network" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "API Keys" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Style" })).toBeVisible();
  await expect(page.locator("#settings-api-key-table")).toContainText("Dashboard key");

  await page.locator("#api-preset-name").fill("Staging");
  await page.locator("#api-preset-url").fill("http://127.0.0.1:7000");
  await page.locator('[data-action="settings-preset-add"]').click();
  await expect(page.locator("#toast")).toContainText("preset");

  await page.locator('[data-action="api-key-issue"]').click();
  await expect(page.locator("#settings-api-key-table")).toContainText("Dashboard automation");
  await expect(page.locator(".notice.warning")).toContainText("Secret 只显示一次");

  await page.locator('[data-action="settings-open-backup"]').click();
  await expect(page.locator("#backup-dialog")).toBeVisible();
  await page.locator('[data-action="backup-export"]').click();
  await expect(page.locator("#backup-dialog")).toContainText("export-route");
  await page.getByRole("tab", { name: "List" }).click();
  await expect(page.locator("#backup-dialog-files")).toContainText("backup.zip");
  await page.locator('#backup-dialog [aria-label="Close"]').click();
  await expect(page.locator("#backup-dialog")).toBeHidden();

  await page.locator('#settings-migration [data-action="settings-open-migration"]').click();
  await expect(page.locator("#migration-dialog")).toBeVisible();
  await page.locator('[data-action="migration-plan"]').click();
  await expect(page.locator("#migration-dialog")).toContainText("migration");
  await page.locator('#migration-dialog [aria-label="Close"]').click();

  await page.locator('[data-action="settings-open-changelog"]').click();
  await expect(page.locator("#changelog-dialog")).toBeVisible();
  await expect(page.locator("#changelog-dialog")).toContainText("Dashboard parity");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installSettingsMocks(page) {
  let apiKeys = [
    {
      key_id: "key-1",
      name: "Dashboard key",
      key_prefix: "abk_dash",
      scopes: ["chat", "file"],
      created_by: "admin",
      active: true,
      is_expired: false,
    },
  ];
  const backupFiles = [
    { filename: "backup.zip", size_bytes: 4096, astrbot_version: "v4.0.0", modified_at_unix: 1779105600 },
  ];
  let operation = { operation_id: "project-update-v4.1.0", kind: "project_update", progress: { status: "queued" } };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.postDataJSON?.() || {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "mock" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, {
        services: [
          { label: "Backup", closure_level: "runtime", api_base: "/api/management/backup" },
          { label: "Update", closure_level: "runtime", api_base: "/api/management/update" },
        ],
      });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/management/api-keys") {
      return fulfillJson(route, { api_keys: apiKeys });
    }
    if (url.pathname === "/api/management/api-keys/issue") {
      const issued = {
        key_id: body.key_id || "dashboard-key",
        name: body.name || "Dashboard automation",
        key_prefix: "abk_new",
        scopes: body.scopes || ["chat"],
        created_by: body.created_by || "dashboard",
        active: true,
        is_expired: false,
      };
      apiKeys = [issued, ...apiKeys];
      return fulfillJson(route, { issued, secret: body.secret || "ak_dashboard_secret", api_keys: apiKeys });
    }
    if (url.pathname === "/api/management/update/check") {
      return fulfillJson(route, { check: { current_version: "v4.0.0", latest_version: "v4.1.0", has_new_version: true, dashboard_version: "v4.0.0", dashboard_has_new_version: true } });
    }
    if (url.pathname === "/api/management/update/releases") {
      return fulfillJson(route, { releases: [{ version: "v4.1.0", title: "Dashboard parity" }] });
    }
    if (url.pathname === "/api/management/update/changelog") {
      return fulfillJson(route, { current_version: "v4.0.0", latest_version: "v4.1.0", releases: [{ version: "v4.1.0", title: "Dashboard parity", body: "# Dashboard parity\n- Settings workflow" }] });
    }
    if (url.pathname === "/api/management/update/migration-check") {
      return fulfillJson(route, { check: { pending_storage_migrations: ["001-main"], legacy_data_migration_needed: true } });
    }
    if (url.pathname === "/api/management/update/migration-plan") {
      operation = { operation_id: "migration", kind: "migration", progress: { status: "completed" }, metadata: body };
      return fulfillJson(route, { operation });
    }
    if (url.pathname === "/api/management/update/operations/run") {
      operation = { operation_id: body.operation_id || operation.operation_id, kind: operation.kind, progress: { status: "completed" } };
      return fulfillJson(route, { operation });
    }
    if (url.pathname.startsWith("/api/management/update/operations")) {
      return fulfillJson(route, { operation });
    }
    if (url.pathname === "/api/management/backup/files") {
      return fulfillJson(route, { files: backupFiles });
    }
    if (url.pathname === "/api/management/backup/export") {
      return fulfillJson(route, { task: { task_id: "export-route", status: "completed", result: { filename: "backup.zip" } } });
    }
    if (url.pathname.startsWith("/api/management/backup/progress")) {
      return fulfillJson(route, { task: { task_id: "export-route", status: "completed", result: { filename: "backup.zip" } } });
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
