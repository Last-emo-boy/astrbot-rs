import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "subagent-token");
  });
  await installSubagentMocks(page);
});

test("subagent route supports source-style edit save handoff preview and execution workflow", async ({ page }) => {
  await page.goto("/subagent");

  await expect(page.getByRole("heading", { name: "SubAgent 编排" })).toBeVisible();
  await expect(page.locator(".subagent-card")).toContainText("researcher");
  await expect(page.locator(".subagent-handoff-table")).toContainText("transfer_to_researcher");

  await page.locator('[data-action="subagent-add"]').click();
  const cards = page.locator("[data-subagent-card]");
  await expect(cards).toHaveCount(2);

  const newCard = cards.nth(1);
  await newCard.locator(".subagent-agent-name").fill("analyst");
  await newCard.locator(".subagent-agent-provider").selectOption("openai/gpt-4.1-mini");
  await newCard.locator(".subagent-agent-persona").selectOption("incident");
  await newCard.locator(".subagent-agent-prompt").fill("Investigate operational incidents.");
  await newCard.locator(".subagent-agent-description").fill("Analyze incidents and propose next actions.");
  await newCard.locator('input[value="custom"]').check();
  await newCard.locator('input[name="subagent-tool"][value="diagnostics"]').check();

  await page.locator('[data-action="subagent-save"]').click();
  await expect(page.locator("#toast")).toContainText("保存成功");
  await expect(page.locator(".subagent-agent-list")).toContainText("analyst");
  await expect(page.locator(".subagent-handoff-table")).toContainText("transfer_to_analyst");
  await expect(page.locator(".subagent-handoff-table")).toContainText("diagnostics");

  await page.locator("#subagent-execute-name").selectOption("analyst");
  await page.locator("#subagent-execute-input").fill("Summarize the incident timeline.");
  await page.locator('[data-action="subagent-execute"]').click();
  await expect(page.locator("#toast")).toContainText("SubAgent execution bridge 已调用");
  await expect(page.locator(".subagent-execution-table")).toContainText("analyst: Summarize the incident timeline.");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installSubagentMocks(page) {
  const providers = [
    { id: "openai/gpt-4.1-mini", name: "OpenAI", model: "gpt-4.1-mini", provider_type: "chat_completion", enable: true },
    { id: "anthropic/claude", name: "Claude", model: "claude-sonnet", provider_type: "chat_completion", enable: true },
  ];
  const personas = [
    { persona_id: "support", name: "Support", system_prompt: "Help users safely." },
    { persona_id: "incident", name: "Incident Lead", system_prompt: "Lead incident response." },
  ];
  const tools = [
    { name: "diagnostics", description: "Run diagnostics", parameters: { type: "object" }, active: true, handler_module_path: "ops.diagnostics" },
    { name: "knowledge_search", description: "Search knowledge base", parameters: { type: "object" }, active: true, handler_module_path: "kb.search" },
  ];
  const executions = [];
  let config = {
    main_enable: true,
    remove_main_duplicate_tools: false,
    router_system_prompt: "Route work to the most suitable SubAgent.",
    agents: [
      {
        name: "researcher",
        enabled: true,
        persona_id: "support",
        provider_id: "anthropic/claude",
        system_prompt: "Research with citations.",
        public_description: "Research tasks and summarize evidence.",
        tools: null,
      },
    ],
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = ["POST", "PATCH", "PUT"].includes(request.method())
      ? JSON.parse(request.postData() || "{}")
      : {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 2, embedding_provider_count: 0, rerank_provider_count: 0, default_chat_provider_id: "openai/gpt-4.1-mini" },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, {
        services: [
          {
            id: "subagent",
            label: "SubAgent",
            configured: true,
            closure_level: "runtime",
            api_base: "/api/subagent/config",
            notes: ["source-compatible + persisted + bridge"],
          },
        ],
      });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/subagent/config" && request.method() === "GET") {
      return fulfillJson(route, legacyOk(config));
    }
    if (url.pathname === "/api/subagent/config" && request.method() === "POST") {
      config = normalizeConfig(body);
      return fulfillJson(route, legacyOk(config, "保存成功"));
    }
    if (url.pathname === "/api/subagent/available-tools") {
      return fulfillJson(route, legacyOk(tools));
    }
    if (url.pathname === "/api/config/provider/list") {
      return fulfillJson(route, legacyOk(providers));
    }
    if (url.pathname === "/api/persona/list") {
      return fulfillJson(route, legacyOk(personas));
    }
    if (url.pathname === "/api/management/subagents" && request.method() === "GET") {
      return fulfillJson(route, catalog(config, executions));
    }
    if (url.pathname === "/api/management/subagents/execute" && request.method() === "POST") {
      const agent = config.agents.find((item) => item.name === body.agent_name);
      if (!agent) return fulfillJson(route, { error: "missing agent" }, 404);
      const execution = {
        run_id: `subagent-run-${executions.length + 1}`,
        agent_name: agent.name,
        handoff_tool: `transfer_to_${agent.name}`,
        input: body.input,
        output: `${agent.name}: ${body.input}`,
        status: "completed",
        background: Boolean(body.background),
        provider_id: agent.provider_id || null,
        persona_id: agent.persona_id || null,
        tools: agent.tools,
        all_tools: agent.tools === null,
        context: body.context || {},
      };
      executions.push(execution);
      return fulfillJson(route, { execution, catalog: catalog(config, executions) });
    }
    return fulfillJson(route, { error: `unhandled ${request.method()} ${url.pathname}` }, 404);
  });
}

function catalog(config, executions) {
  return {
    main_enable: config.main_enable,
    remove_main_duplicate_tools: config.remove_main_duplicate_tools,
    router_system_prompt: config.router_system_prompt,
    agents: config.agents.map((agent) => ({
      ...agent,
      all_tools: agent.tools === null,
    })),
    handoffs: config.agents
      .filter((agent) => agent.enabled !== false && agent.name)
      .map((agent) => ({
        tool_name: `transfer_to_${agent.name}`,
        agent_name: agent.name,
        description: agent.public_description || `Delegate to ${agent.name}`,
        parameters: { type: "object" },
        provider_id: agent.provider_id || null,
        persona_id: agent.persona_id || null,
        tools: agent.tools,
        all_tools: agent.tools === null,
      })),
    executions,
  };
}

function normalizeConfig(payload) {
  return {
    main_enable: Boolean(payload.main_enable),
    remove_main_duplicate_tools: Boolean(payload.remove_main_duplicate_tools),
    router_system_prompt: payload.router_system_prompt || "",
    agents: (payload.agents || []).map((agent) => ({
      name: String(agent.name || "").trim(),
      enabled: agent.enabled !== false,
      persona_id: String(agent.persona_id || "").trim(),
      provider_id: String(agent.provider_id || "").trim(),
      system_prompt: String(agent.system_prompt || "").trim(),
      public_description: String(agent.public_description || "").trim(),
      tools: Array.isArray(agent.tools) ? agent.tools.map(String).filter(Boolean) : null,
    })),
  };
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
