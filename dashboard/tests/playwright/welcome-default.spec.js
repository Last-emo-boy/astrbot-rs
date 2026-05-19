import { expect, test } from "@playwright/test";

const status = {
  providers: {
    chat_provider_count: 1,
    embedding_provider_count: 1,
    rerank_provider_count: 0,
    default_chat_provider_id: "openai",
  },
  platforms: {
    platform_count: 2,
    platform_ids: ["webchat", "telegram"],
    webchat_platform_count: 1,
  },
  plugins: { handler_count: 4 },
};

const capabilities = {
  services: [
    {
      id: "status",
      label: "Status",
      configured: true,
      api_base: "/api/management/status",
      closure_level: "runtime",
      notes: ["runtime ready"],
    },
  ],
};

const stats = {
  generated_at_unix: 1779105600,
  uptime_seconds: 3661,
  log_count: 2,
  trace_count: 1,
  total_messages: 8,
  total_llm_calls: 2,
  total_tokens: 128,
  total_tts_events: 0,
  platform_counts: [
    { platform_id: "webchat", platform_type: "webchat", count: 5 },
    { platform_id: "telegram", platform_type: "telegram", count: 3 },
  ],
  provider_usage: [{ provider_id: "openai", calls: 2, total_tokens: 128 }],
  recent_events: [
    { kind: "platform_message", timestamp: "2026-05-18T10:00:00Z", count: 2 },
    { kind: "platform_message", timestamp: "2026-05-18T11:00:00Z", count: 6 },
  ],
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "test-token");
  });
  await page.route("**/api/management/status", (route) => route.fulfill({ json: status }));
  await page.route("**/api/management/dashboard/capabilities", (route) => route.fulfill({ json: capabilities }));
  await page.route("**/api/management/stats", (route) => route.fulfill({ json: stats }));
});

test("welcome route renders onboarding resources and announcement screenshot", async ({ page }) => {
  await page.goto("/welcome");

  await expect(page.locator('[data-page="welcome"]')).toBeVisible();
  await expect(page.locator("text=Getting Started")).toBeVisible();
  await expect(page.locator("text=Resources")).toBeVisible();
  await expect(page.locator("text=Announcement")).toBeVisible();
  await expect(page.locator("text=业务闭环看板")).toBeVisible();

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});

test("default dashboard route renders source dashboard cards and charts screenshot", async ({ page }) => {
  await page.goto("/dashboard/default");

  await expect(page.locator('[data-page="default-dashboard"]')).toBeVisible();
  await expect(page.locator("text=Total Messages")).toBeVisible();
  await expect(page.locator("text=Online Platforms")).toBeVisible();
  await expect(page.locator("text=Running Time")).toBeVisible();
  await expect(page.locator("text=Memory Usage")).toBeVisible();
  await expect(page.locator("text=Message Trend")).toBeVisible();
  await expect(page.locator("text=Platform Stat")).toBeVisible();

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});
