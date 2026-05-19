import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "cron-token");
  });
  await installCronMocks(page);
});

test("cron route supports source-style create edit toggle run delete workflow", async ({ page }) => {
  await page.goto("/cron");

  await expect(page.getByRole("heading", { name: "Cron Jobs" })).toBeVisible();
  await expect(page.locator(".cron-jobs-table")).toContainText("Daily summary");
  await expect(page.locator(".cron-hero-panel")).toContainText("WebChat(webchat)");

  await page.locator('[data-action="cron-toggle"][data-job="daily-summary"]').click({ force: true });
  await expect(page.locator("#toast")).toContainText("Cron job 已停用");

  await page.locator('[data-action="cron-edit-open"][data-job="daily-summary"]').click();
  await expect(page.locator("#cron-form-dialog")).toBeVisible();
  await page.locator("#cron-form-note").fill("Updated daily summary");
  await page.locator('[data-action="cron-form-save"]').click();
  await expect(page.locator("#toast")).toContainText("Cron job 已更新");
  await expect(page.locator(".cron-jobs-table")).toContainText("Updated daily summary");

  await page.locator('[data-action="cron-create-open"]').click();
  await expect(page.locator("#cron-form-dialog")).toBeVisible();
  await page.locator("#cron-form-run-once").check();
  await page.locator("#cron-form-name").fill("Incident wake");
  await page.locator("#cron-form-session").fill("webchat:incident-room:group");
  await page.locator("#cron-form-note").fill("Check incident status");
  await page.locator("#cron-form-run-at").fill("2026-05-20T08:00");
  await page.locator('[data-action="cron-form-save"]').click();
  await expect(page.locator("#toast")).toContainText("Cron job 已创建");
  await expect(page.locator(".cron-jobs-table")).toContainText("Incident wake");

  await page.locator('[data-action="cron-run"][data-job="incident-wake"]').click();
  await expect(page.locator("#toast")).toContainText("Cron job 已执行");

  await page.locator('[data-action="cron-delete-open"][data-job="incident-wake"]').click();
  await expect(page.locator("#cron-delete-dialog")).toBeVisible();
  await page.locator('[data-action="cron-delete-confirm"]').click();
  await expect(page.locator("#toast")).toContainText("Cron job 已删除");
  await expect(page.locator(".cron-jobs-table")).not.toContainText("Incident wake");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installCronMocks(page) {
  let schedulerState = "running";
  const jobs = [
    {
      job_id: "daily-summary",
      name: "Daily summary",
      job_type: "active_agent",
      cron_expression: "0 8 * * *",
      timezone: "Asia/Shanghai",
      session: "webchat:demo",
      note: "Generate yesterday summary",
      payload: { session: "webchat:demo", note: "Generate yesterday summary", origin: "dashboard" },
      enabled: true,
      persistent: true,
      run_once: false,
      next_run_time: "2026-05-20T08:00:00+08:00",
      last_run_at: "2026-05-19T08:00:00+08:00",
    },
  ];

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = ["POST", "PATCH", "PUT"].includes(request.method())
      ? JSON.parse(request.postData() || "{}")
      : {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "openai" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "cron", label: "Cron", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/platform/stats") {
      return fulfillJson(route, legacyOk({
        platforms: [
          { id: "webchat", type: "webchat", meta: { id: "webchat", name: "webchat", display_name: "WebChat", support_proactive_message: true } },
        ],
      }));
    }
    if (url.pathname === "/api/management/cron/jobs") {
      return fulfillJson(route, {
        state: schedulerState,
        jobs,
        scheduled_jobs: jobs.filter((job) => job.enabled).map((job) => ({
          job_id: job.job_id,
          schedule_key: job.run_once ? job.run_at : job.cron_expression,
          enabled: job.enabled,
        })),
      });
    }
    if (url.pathname === "/api/management/cron/start") {
      schedulerState = "running";
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/cron/shutdown") {
      schedulerState = "stopped";
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/cron/tick") {
      return fulfillJson(route, { report: { ran_count: 1, due_count: 1, failed_count: 0 } });
    }
    if (url.pathname === "/api/cron/jobs" && request.method() === "GET") {
      return fulfillJson(route, legacyOk(jobs));
    }
    if (url.pathname === "/api/cron/jobs" && request.method() === "POST") {
      const job = normalizeJob(body);
      jobs.push(job);
      return fulfillJson(route, legacyOk(job, "created"));
    }
    const jobPath = url.pathname.match(/^\/api\/cron\/jobs\/([^/]+)(?:\/run)?$/);
    if (jobPath) {
      const jobId = decodeURIComponent(jobPath[1]);
      const job = jobs.find((item) => item.job_id === jobId);
      if (!job) return fulfillJson(route, legacyOk({}, "missing"), 404);
      if (url.pathname.endsWith("/run")) {
        job.last_run_at = "2026-05-19T12:00:00+08:00";
        return fulfillJson(route, legacyOk({ job_id: jobId }, "run"));
      }
      if (request.method() === "PATCH") {
        Object.assign(job, normalizeJob({ ...job, ...body }, job.job_id));
        return fulfillJson(route, legacyOk(job, "updated"));
      }
      if (request.method() === "DELETE") {
        jobs.splice(jobs.indexOf(job), 1);
        return fulfillJson(route, legacyOk({}, "Cron job 已删除"));
      }
    }
    return fulfillJson(route, { error: `unhandled ${request.method()} ${url.pathname}` }, 404);
  });
}

function normalizeJob(payload, existingId = "") {
  const jobId = existingId || slug(payload.name || "cron-job");
  const runOnce = Boolean(payload.run_once);
  return {
    job_id: jobId,
    name: payload.name || "active_agent_task",
    job_type: "active_agent",
    cron_expression: runOnce ? "" : payload.cron_expression,
    timezone: payload.timezone || "Asia/Shanghai",
    session: payload.session,
    note: payload.note || payload.description || payload.name,
    payload: {
      session: payload.session,
      note: payload.note || payload.description || payload.name,
      run_at: runOnce ? payload.run_at : undefined,
      origin: "dashboard",
    },
    enabled: payload.enabled !== false,
    persistent: true,
    run_once: runOnce,
    run_at: runOnce ? payload.run_at : "",
    next_run_time: runOnce ? payload.run_at : "2026-05-20T08:00:00+08:00",
    last_run_at: payload.last_run_at || "",
  };
}

function slug(value) {
  return String(value || "cron-job").trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "cron-job";
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
