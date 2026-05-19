import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "session-token");
  });
  await installSessionMocks(page);
});

test("session management route supports filter edit batch group and delete workflow", async ({ page }) => {
  await page.goto("/session-management");

  await expect(page.getByRole("heading", { name: "Session Rules" })).toBeVisible();
  const rulesTable = page.locator(".session-rules-table");
  const groupsPanel = page.locator("section.panel", {
    has: page.getByRole("heading", { name: "分组管理" }),
  });
  await expect(rulesTable).toContainText("Ops Room");

  await page.locator("#session-filter").fill("ops");
  await page.locator('[data-action="session-filter"]').click();
  await expect(page.locator("#toast")).toContainText("筛选");
  await expect(rulesTable).toContainText("webchat:GroupMessage:room-1");

  await page.locator('[data-action="session-rule-edit-open"]').first().click();
  await expect(page.locator("#session-rule-editor-dialog")).toBeVisible();
  await page.locator("#session-editor-custom-name").fill("Renamed Ops");
  await page.locator("#session-editor-llm-enabled").check();
  await page.locator('[data-action="session-rule-save-service"]').click();
  await expect(page.locator("#toast")).toContainText("Service rule");
  await expect(rulesTable).toContainText("Renamed Ops");

  await page.locator("#session-editor-chat-provider").selectOption("openai");
  await page.locator("#session-editor-tts-provider").selectOption("edge");
  await page.locator('[data-action="session-rule-save-provider"]').click();
  await expect(page.locator("#toast")).toContainText("Provider rule");

  await page.locator("#session-editor-enabled-plugins").fill("weather");
  await page.locator('[data-action="session-rule-save-plugin"]').click();
  await expect(page.locator("#toast")).toContainText("Plugin rule");

  await page.locator("#session-editor-kb-ids").fill("kb-1");
  await page.locator("#session-editor-kb-ids").blur();
  await page.locator('[data-action="session-rule-save-kb"]').click();
  await expect(page.locator("#toast")).toContainText("KB rule");
  await page.locator('#session-rule-editor-dialog [aria-label="Close"]').click();

  await page.locator('[data-action="session-select"]').first().check();
  await expect(page.locator('[data-action="session-batch-delete-open"]')).toBeVisible();
  await page.locator("#session-batch-llm").selectOption("false");
  await page.locator("#session-batch-tts").selectOption("false");
  await page.locator("#session-batch-chat-provider").selectOption("openai");
  await page.locator('[data-action="session-batch-apply"]').click();
  await expect(page.locator("#toast")).toContainText("批量更新成功");

  await page.locator('[data-action="session-group-create-open"]').click();
  await expect(page.locator("#session-group-dialog")).toBeVisible();
  await page.locator("#session-group-name").fill("Support");
  await page.locator('[data-action="session-group-add-umo"]').first().click();
  await page.locator('[data-action="session-group-save"]').click();
  await expect(groupsPanel).toContainText("Support");

  await page.locator('[data-action="session-quick-name-open"]').first().click();
  await expect(page.locator("#session-quick-name-dialog")).toBeVisible();
  await page.locator("#session-quick-name").fill("Quick Ops");
  await page.locator("#session-quick-name").blur();
  await clickActionAtCenter(page, page.locator('[data-action="session-quick-name-save"]'), "session-quick-name-save");
  await expect(rulesTable).toContainText("Quick Ops");

  await page.locator('[data-action="session-rule-delete-open"]').first().click();
  await expect(page.locator("#session-delete-dialog")).toBeVisible();
  await clickActionAtCenter(page, page.locator('[data-action="session-rule-delete-confirm"]'), "session-rule-delete-confirm");
  await expect(page.locator("#toast")).toContainText("规则集已删除");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installSessionMocks(page) {
  let rules = [
    {
      umo: "webchat:GroupMessage:room-1",
      platform: "webchat",
      message_type: "GroupMessage",
      session_id: "room-1",
      rules: {
        session_service_config: {
          session_enabled: true,
          llm_enabled: false,
          tts_enabled: true,
          custom_name: "Ops Room",
          persona_id: "support",
        },
        provider_perf_chat_completion: "openai",
      },
    },
  ];
  let groups = [
    { id: "ops", name: "Ops", umos: ["webchat:GroupMessage:room-1"], umo_count: 1 },
  ];
  const activeUmos = ["webchat:GroupMessage:room-1", "webchat:FriendMessage:user-1"];

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.method() === "POST" ? JSON.parse(request.postData() || "{}") : {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "openai" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1, handlers: [{ plugin_name: "weather" }] },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "session", label: "Session", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats" || url.pathname === "/api/management/logs") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [], snapshot: { entries: [] } });
    }
    if (url.pathname === "/api/session/list-rule") {
      const search = (url.searchParams.get("search") || "").toLowerCase();
      const filtered = rules.filter((rule) => `${rule.umo} ${JSON.stringify(rule.rules)}`.toLowerCase().includes(search));
      return fulfillJson(route, legacyOk({
        rules: filtered,
        total: filtered.length,
        page: 1,
        page_size: 10,
        available_rule_keys: ["session_service_config", "session_plugin_config", "kb_config"],
        available_personas: [{ name: "support", prompt: "Help" }],
        available_chat_providers: [{ id: "openai", name: "OpenAI", model: "gpt-4.1-mini" }],
        available_stt_providers: [{ id: "whisper", name: "Whisper", model: "whisper-1" }],
        available_tts_providers: [{ id: "edge", name: "Edge", model: "edge-tts" }],
        available_plugins: [{ name: "weather", display_name: "Weather" }],
        available_kbs: [{ kb_id: "kb-1", kb_name: "Docs", emoji: "D" }],
      }));
    }
    if (url.pathname === "/api/session/active-umos") {
      return fulfillJson(route, legacyOk({ umos: activeUmos }));
    }
    if (url.pathname === "/api/session/groups") {
      return fulfillJson(route, legacyOk({ groups }));
    }
    if (url.pathname === "/api/session/update-rule") {
      let target = rules.find((rule) => rule.umo === body.umo);
      if (!target) {
        target = { umo: body.umo, platform: "webchat", message_type: "GroupMessage", session_id: body.umo.split(":").pop(), rules: {} };
        rules.push(target);
      }
      target.rules[body.rule_key] = body.rule_value;
      return fulfillJson(route, legacyOk({ message: "updated", umo: body.umo }));
    }
    if (url.pathname === "/api/session/delete-rule") {
      if (body.rule_key) {
        const target = rules.find((rule) => rule.umo === body.umo);
        if (target) delete target.rules[body.rule_key];
      } else {
        rules = rules.filter((rule) => rule.umo !== body.umo);
      }
      return fulfillJson(route, legacyOk({ message: "deleted", umo: body.umo }));
    }
    if (url.pathname === "/api/session/batch-delete-rule") {
      rules = rules.filter((rule) => !(body.umos || []).includes(rule.umo));
      return fulfillJson(route, legacyOk({ deleted_count: body.umos?.length || 0, failed_umos: [] }));
    }
    if (url.pathname === "/api/session/batch-update-service") {
      const targets = body.umos?.length ? body.umos : activeUmos;
      for (const umo of targets) {
        let target = rules.find((rule) => rule.umo === umo);
        if (!target) {
          target = { umo, platform: "webchat", message_type: "GroupMessage", session_id: umo.split(":").pop(), rules: {} };
          rules.push(target);
        }
        target.rules.session_service_config = {
          ...(target.rules.session_service_config || {}),
          ...(body.llm_enabled === undefined ? {} : { llm_enabled: body.llm_enabled }),
          ...(body.tts_enabled === undefined ? {} : { tts_enabled: body.tts_enabled }),
        };
      }
      return fulfillJson(route, legacyOk({ success_count: targets.length, failed_count: 0, failed_umos: [] }));
    }
    if (url.pathname === "/api/session/batch-update-provider") {
      const targets = body.umos?.length ? body.umos : activeUmos;
      const key = body.provider_type === "text_to_speech" ? "provider_perf_text_to_speech" : "provider_perf_chat_completion";
      for (const umo of targets) {
        const target = rules.find((rule) => rule.umo === umo) || rules[0];
        target.rules[key] = body.provider_id;
      }
      return fulfillJson(route, legacyOk({ success_count: targets.length, failed_count: 0, failed_umos: [] }));
    }
    if (url.pathname === "/api/session/group/create") {
      const group = { id: body.name.toLowerCase(), name: body.name, umos: body.umos || [], umo_count: (body.umos || []).length };
      groups.push(group);
      return fulfillJson(route, legacyOk({ group, message: "created" }));
    }
    if (url.pathname === "/api/session/group/update") {
      const group = groups.find((item) => item.id === body.id);
      if (group) {
        if (body.name) group.name = body.name;
        if (body.umos) group.umos = body.umos;
        if (body.add_umos) group.umos = Array.from(new Set([...group.umos, ...body.add_umos]));
        group.umo_count = group.umos.length;
      }
      return fulfillJson(route, legacyOk({ group, message: "updated" }));
    }
    if (url.pathname === "/api/session/group/delete") {
      groups = groups.filter((group) => group.id !== body.id);
      return fulfillJson(route, legacyOk({ message: "deleted" }));
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

async function clickActionAtCenter(page, locator, expectedAction) {
  const box = await locator.boundingBox();
  expect(box).not.toBeNull();
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  const hitAction = await page.evaluate(([hitX, hitY]) => {
    return document.elementFromPoint(hitX, hitY)?.closest("[data-action]")?.dataset.action || "";
  }, [x, y]);
  expect(hitAction).toBe(expectedAction);
  await page.mouse.click(x, y);
}
