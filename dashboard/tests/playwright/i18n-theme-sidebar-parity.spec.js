import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "i18n-token");
    window.localStorage.setItem("astrbot.locale", "en-US");
    window.localStorage.setItem("astrbot-locale", "en-US");
    window.localStorage.removeItem("astrbot.dashboardPreferences");
  });
  await installSettingsMocks(page);
});

test("settings mirrors source locale theme sidebar and browser desktop fallback controls", async ({ page }) => {
  await page.goto("/settings");

  await expect(page.locator('[data-page="settings"]')).toBeVisible();
  await expect(page.locator("#topbar-locale")).toHaveValue("en-US");
  await expect(page.getByRole("heading", { name: "Network" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sidebar" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Style" })).toBeVisible();
  await expect(page.locator("#settings-desktop-bridge")).toContainText("browser");

  await page.locator("#topbar-locale").selectOption("ru-RU");
  await expect(page.locator("#settings-network")).toContainText("Сеть");
  await expect(page.locator("#topbar-locale")).toHaveValue("ru-RU");

  await page.locator("#dashboard-locale").selectOption("en-US");
  await page.locator("#dashboard-theme").selectOption("dark");
  await page.locator("#theme-primary-color").fill("#123456");
  await page.locator("#theme-secondary-color").fill("#0abcde");
  await page.locator("#sidebar-compact").check();
  await page.locator('[data-action="save-dashboard-preferences"]').click();
  await expect(page.locator("#toast")).toContainText("Dashboard preferences saved");
  await expect(page.locator("body")).toHaveAttribute("data-theme", "dark");
  await expect(page.locator("body")).toHaveClass(/sidebar-compact/);
  await expect(page.locator("#topbar-locale")).toHaveValue("en-US");
  await expect(page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--primary").trim())).resolves.toBe("#123456");
  await expect(page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--accent").trim())).resolves.toBe("#0abcde");

  await page.locator('[data-action="settings-sidebar-open"]').click();
  await expect(page.locator("#sidebar-customizer-dialog")).toBeVisible();
  await page.locator('[data-sidebar-list="main"] [data-route-id="chat"] [data-action="settings-sidebar-move-more"]').click();
  await expect(page.locator('[data-sidebar-list="more"] .sidebar-customizer-item[data-route-id="chat"]')).toBeVisible();
  await page.locator("#sidebar-customizer-dialog [data-action=\"settings-sidebar-save\"]").click();
  await expect(page.locator("#toast")).toContainText("Sidebar customization saved");
  await expect(page.locator(".nav-group").filter({ hasText: "More Features" }).locator('[data-route="chat"]')).toHaveCount(1);

  await page.locator("#settings-sidebar [data-action=\"settings-sidebar-reset\"]").click();
  await expect(page.locator("#toast")).toContainText("Sidebar customization reset");
  await expect(page.locator(".nav-group").filter({ hasText: "Runtime" }).locator('[data-route="chat"]')).toHaveCount(1);

  await page.locator('#settings-desktop-bridge [data-action="settings-desktop-probe"]').click();
  await expect(page.locator("#toast")).toContainText("Desktop bridge probe completed");
  await expect(page.locator("#settings-desktop-bridge")).toContainText("desktop bridge unavailable");
});

test("desktop bridge controls call Electron bridge and updater affordances", async ({ page }) => {
  await page.addInitScript(() => {
    window.astrbotDesktop = {
      isDesktop: true,
      isDesktopRuntime: async () => true,
      getBackendState: async () => ({ running: true, pid: 6185 }),
      restartBackend: async (token) => ({ ok: true, tokenPresent: Boolean(token) }),
      onTrayRestartBackend: () => undefined,
    };
    window.astrbotAppUpdater = {
      checkForAppUpdate: async () => ({ ok: true, hasUpdate: true, latestVersion: "v9.9.9" }),
      installAppUpdate: async () => ({ ok: true, installed: true }),
    };
  });

  await page.goto("/settings");
  await expect(page.locator("#settings-desktop-bridge")).toContainText("manageable");
  await page.locator('#settings-desktop-bridge [data-action="settings-desktop-probe"]').click();
  await expect(page.locator("#settings-desktop-bridge")).toContainText('"pid": 6185');
  await page.locator('#settings-desktop-bridge [data-action="settings-desktop-update-check"]').click();
  await expect(page.locator("#settings-desktop-bridge")).toContainText("v9.9.9");
  await page.locator('#settings-desktop-bridge [data-action="settings-desktop-restart"]').click();
  await expect(page.locator("#toast")).toContainText("Desktop backend restart requested");
});

test("mobile drawer opens closes and collapses after navigation", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "mobile", "mobile drawer behavior is project-specific");

  await page.goto("/settings");
  await expect(page.locator("body")).not.toHaveClass(/nav-open/);

  await page.locator("#mobile-sidebar-toggle").click();
  await expect(page.locator("body")).toHaveClass(/nav-open/);
  await expect(page.locator("#drawer-scrim")).toBeVisible();

  const viewport = page.viewportSize();
  await page.mouse.click((viewport?.width || 393) - 8, 24);
  await expect(page.locator("body")).not.toHaveClass(/nav-open/);

  await page.locator("#mobile-sidebar-toggle").click();
  await page.locator('[data-route="settings"]').click();
  await expect(page.locator("body")).not.toHaveClass(/nav-open/);
});

async function installSettingsMocks(page) {
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "mock" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ label: "Dashboard", closure_level: "runtime", api_base: "/api/management" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/management/api-keys") {
      return fulfillJson(route, { api_keys: [{ key_id: "key-1", name: "Dashboard key", key_prefix: "abk_dash", scopes: ["chat"], active: true }] });
    }
    if (url.pathname === "/api/management/update/check") {
      return fulfillJson(route, { check: { current_version: "v4.0.0", latest_version: "v4.0.0", has_new_version: false } });
    }
    if (url.pathname === "/api/management/update/releases") {
      return fulfillJson(route, { releases: [] });
    }
    if (url.pathname === "/api/management/update/changelog") {
      return fulfillJson(route, { releases: [] });
    }
    if (url.pathname === "/api/management/update/migration-check") {
      return fulfillJson(route, { check: { pending_storage_migrations: [], legacy_data_migration_needed: false } });
    }
    if (url.pathname === "/api/management/backup/files") {
      return fulfillJson(route, { files: [] });
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
