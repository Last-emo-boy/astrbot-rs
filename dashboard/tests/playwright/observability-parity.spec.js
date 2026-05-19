import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "observability-token");
  });
  await installObservabilityMocks(page);
});

test("console and trace routes support source-style history live controls settings and pip dialog", async ({ page }) => {
  await page.goto("/logs");

  await expect(page.locator('[data-page="console"]')).toBeVisible();
  await expect(page.locator("#console-terminal")).toContainText("runtime ready");
  await expect(page.locator("#console-terminal")).toContainText("Runtime");

  await page.locator('[data-action="console-pip-open"]').click();
  await expect(page.locator("#console-pip-dialog")).toBeVisible();
  await page.locator("#console-pip-package").fill("llmtuner");
  await page.locator("#console-pip-mirror").fill("https://pypi.example/simple");
  await page.locator('[data-action="console-pip-install"]').click();
  await expect(page.locator("#toast")).toContainText("安装成功。");

  await page.locator('[data-action="logs-stream-start"]').click();
  await expect(page.locator("#console-terminal")).toContainText("live streamed log");
  await expect(page.locator("#console-stream-state")).toContainText(/SSE|最新日志/);

  await page.locator("#console-search").fill("live");
  await page.locator('[data-action="console-filter"]').click();
  await expect(page.locator("#console-terminal")).toContainText("live streamed log");
  await expect(page.locator("#console-terminal")).not.toContainText("runtime ready");

  await page.goto("/trace");
  await expect(page.locator('[data-page="trace"]')).toBeVisible();
  await expect(page.locator(".trace-table")).toContainText("span-1");
  await page.locator('[data-action="trace-toggle-event"][data-span="span-1"]').click();
  await expect(page.locator(".trace-records")).toContainText("astr_agent_prepare");
  await expect(page.locator(".trace-records")).toContainText("[REDACTED]");

  await page.locator("#trace-enabled").uncheck();
  await page.locator("#trace-redact-fields").fill("authorization, api_key");
  await page.locator('[data-action="trace-settings-save"]').click();
  await expect(page.locator("#toast")).toContainText("Trace 设置已保存");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installObservabilityMocks(page) {
  const legacyRows = [
    {
      id: 1,
      type: "log",
      time: 1779169818,
      level: "INFO",
      data: "runtime ready",
      message: "runtime ready",
      source: "Runtime",
      target: "event-1",
    },
    {
      type: "trace",
      time: 1779169820,
      span_id: "span-1",
      name: "pipeline.process",
      span_name: "pipeline.process",
      action: "astr_agent_prepare",
      umo: "webchat:FriendMessage:user-1",
      message_origin: "webchat:FriendMessage:user-1",
      sender_name: "Alice",
      message_outline: "hello trace",
      fields: { authorization: "[REDACTED]", provider: "mock" },
    },
  ];
  let traceSettings = {
    enabled: true,
    capture_message_outline: true,
    max_events: 100,
    redact_fields: ["authorization"],
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = ["POST", "PATCH", "PUT"].includes(request.method())
      ? JSON.parse(request.postData() || "{}")
      : {};

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
          { id: "console", label: "Console", configured: true, closure_level: "runtime", api_base: "/api/management/logs" },
          { id: "trace", label: "Trace", configured: true, closure_level: "runtime", api_base: "/api/management/trace" },
        ],
      });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, log_count: 2, trace_count: 1, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/management/logs") {
      return fulfillJson(route, {
        snapshot: {
          entries: [
            { id: 1, level: "Info", source: "Runtime", target: "event-1", message: "runtime ready", occurred_at_unix: 1779169818 },
          ],
          next_cursor: 1,
        },
      });
    }
    if (url.pathname === "/api/log-history") {
      return fulfillJson(route, legacyOk({ logs: legacyRows }));
    }
    if (url.pathname === "/api/live-log") {
      return route.fulfill({
        status: 200,
        contentType: "text/event-stream; charset=utf-8",
        body: 'id: 3\ndata: {"id":3,"type":"log","time":1779169822,"level":"ERROR","data":"live streamed log","message":"live streamed log","source":"Runtime","target":"event-3"}\n\n',
      });
    }
    if (url.pathname === "/api/update/pip-install") {
      return fulfillJson(route, legacyOk(null, "安装成功。"));
    }
    if (url.pathname === "/api/management/trace") {
      return fulfillJson(route, {
        settings: traceSettings,
        events: [
          {
            span_id: "span-1",
            span_name: "pipeline.process",
            action: "astr_agent_prepare",
            message_origin: "webchat:FriendMessage:user-1",
            sender_name: "Alice",
            message_outline: "hello trace",
            fields: [["authorization", "[REDACTED]"], ["provider", "mock"]],
            occurred_at_unix: 1779169820,
          },
        ],
      });
    }
    if (url.pathname === "/api/management/trace/settings" && request.method() === "GET") {
      return fulfillJson(route, traceSettings);
    }
    if (url.pathname === "/api/management/trace/settings" && request.method() === "POST") {
      traceSettings = {
        enabled: body.enabled !== false,
        capture_message_outline: body.capture_message_outline !== false,
        max_events: Number(body.max_events || 100),
        redact_fields: body.redact_fields || [],
      };
      return fulfillJson(route, traceSettings);
    }
    if (url.pathname === "/api/trace/settings") {
      return fulfillJson(route, legacyOk({ trace_enable: traceSettings.enabled }));
    }
    return fulfillJson(route, { error: `unhandled ${request.method()} ${url.pathname}` }, 404);
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
