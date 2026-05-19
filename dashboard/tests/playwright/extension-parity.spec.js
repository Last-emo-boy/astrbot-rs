import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "extension-token");
  });
  await installExtensionMocks(page);
});

test("extension routes support installed plugins market tools commands mcp and skills dialogs", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "mobile", "mobile coverage is handled by the tool-use alias smoke");
  await page.goto("/extension");

  await expect(page.locator('[data-page="extension-installed"]')).toBeVisible();
  await expect(page.locator(".extension-card", { hasText: "Weather Plugin" })).toContainText("Forecast plugin");
  await expect(page.locator(".warning-panel")).toContainText("broken");

  await page.locator('[data-action="plugin-view-mode"][data-view="list"]').click();
  await expect(page.locator(".table", { hasText: "Weather Plugin" })).toBeVisible();

  await page.locator('[data-action="plugin-doc-open"][data-plugin="weather"][data-doc="readme"]').first().click();
  await expect(page.locator("#plugin-doc-dialog")).toContainText("Weather market docs");
  await page.locator('#plugin-doc-dialog [data-action="plugin-doc-close"]').click();

  await page.locator('[data-action="plugin-config-open"][data-plugin="weather"]').first().click();
  await expect(page.locator("#plugin-config-dialog")).toBeVisible();
  await page.locator('#plugin-config-dialog [data-action="plugin-config-file-read"]').click();
  await expect(page.locator("#plugin-config-dialog #plugin-config-json")).toContainText("Shanghai");
  await page.locator('#plugin-config-dialog [data-action="plugin-config-file-write"]').click();
  await expect(page.locator("#toast")).toContainText("插件配置文件已写入");
  await page.locator('#plugin-config-dialog [data-action="extension-dialog-close"]').click();

  await page.locator('[data-action="plugin-source-open"][data-plugin="weather"]').first().click();
  await expect(page.locator("#plugin-source-dialog")).toContainText("python_compat");
  await page.locator('#plugin-source-dialog [data-action="extension-dialog-close"]').click();

  await page.locator('[data-action="plugin-upload-plan"]').first().click();
  await expect(page.locator("#toast")).toContainText("插件上传计划已生成");
  await page.locator('[data-action="plugin-source-plan"]').click();
  await expect(page.locator("#toast")).toContainText("插件来源计划已生成");
  await page.locator('[data-action="plugin-lifecycle"][data-plugin="weather"][data-lifecycle="disable"]').first().click();
  await expect(page.locator("#toast")).toContainText("插件 lifecycle 已更新");

  await page.goto("/extension-marketplace");
  await expect(page.locator('[data-page="extension-marketplace"]')).toBeVisible();
  await expect(page.locator(".source-warning")).toContainText("插件源");
  await page.locator("#market-search").fill("weather");
  await page.locator('[data-action="extension-filter"]').click();
  await expect(page.locator(".market-plugin-card", { hasText: "Weather" }).first()).toBeVisible();
  await expect(page.locator(".market-plugin-card", { hasText: "News" })).toHaveCount(0);
  await page.locator('[data-action="plugin-doc-open"][data-plugin="weather"]').first().click();
  await expect(page.locator("#plugin-doc-dialog")).toContainText("Weather market docs");
  await page.locator('#plugin-doc-dialog [data-action="plugin-doc-close"]').click();
  await page.locator('[data-action="plugin-execute"][data-plan="install"][data-plugin="weather"]').first().click();
  await expect(page.locator("#toast")).toContainText("插件安装已执行");
  await page.locator('[data-action="plugin-execute"][data-plan="update"][data-plugin="weather"]').first().click();
  await expect(page.locator("#toast")).toContainText("插件更新已执行");
  await page.locator('[data-action="plugin-update-all-plan"]').click();
  await expect(page.locator("#toast")).toContainText("插件 update-all plan 已生成");

  await page.goto("/extension/tools");
  await expect(page.locator('[data-page="extension-tools"]')).toBeVisible();
  await expect(page.locator(".command-table")).toContainText("/weather");
  await expect(page.locator(".ui-state.error")).toContainText("Command conflicts");
  await page.locator('[data-action="command-details-open"][data-command="weather.main"]').dispatchEvent("click");
  await expect(page.locator("#command-details-dialog")).toContainText("Weather command");
  await page.locator('#command-details-dialog [data-action="extension-dialog-close"]').click();
  await page.locator('[data-action="command-rename-open"][data-command="weather.main"]').dispatchEvent("click");
  await page.locator("#command-rename-command").fill("forecast");
  await page.locator('#command-rename-dialog [data-action="command-rename-save"]').click();
  await expect(page.locator(".command-table")).toContainText("forecast");
  await page.locator('[data-action="tool-details-open"][data-tool="weather.lookup"]').dispatchEvent("click");
  await expect(page.locator("#tool-details-dialog")).toContainText("Lookup city");
  await page.locator('#tool-details-dialog [data-action="extension-dialog-close"]').click();
  await page.locator('[data-action="toggle-tool"][data-tool="weather.lookup"]').click();
  await expect(page.locator("#toast")).toContainText("工具状态已更新");
  await expect(page.locator("#mcp-json")).toBeVisible();
  await page.locator('[data-action="mcp-check"][data-mcp="docs"]').click();
  await expect(page.locator("#toast")).toContainText("MCP 配置检查完成");
  await page.locator('[data-action="mcp-sync"][data-mcp="docs"]').click();
  await expect(page.locator("#toast")).toContainText("MCP bridge plan 已生成");
  await page.locator('[data-action="mcp-json-template"][data-template="streamable_http"]').click();
  await expect(page.locator("#mcp-json")).toContainText("streamable_http");
  await page.locator("#mcp-json-name").fill("http-docs");
  await page.locator('[data-action="mcp-json-upsert"]').click();
  await expect(page.locator("#toast")).toContainText("MCP JSON 配置已保存");
  await page.locator('[data-action="mcp-sync-provider"]').click();
  await expect(page.locator("#toast")).toContainText("MCP provider sync plan 已生成");

  await page.goto("/extension/skills");
  await expect(page.locator('[data-page="extension-skills"]')).toBeVisible();
  await expect(page.locator(".skill-card", { hasText: "writer" })).toContainText("Write docs");
  await page.locator('[data-action="skill-download"][data-skill="writer"]').click();
  await expect(page.locator("#toast")).toContainText("Skill 下载当前不可用");
  await page.locator('[data-action="skill-install-plan"]').click();
  await expect(page.locator("#toast")).toContainText("安装计划已生成");
  await page.locator('[data-action="skill-install"]').click();
  await expect(page.locator(".skill-card", { hasText: "new_skill" })).toBeVisible();
  await page.locator('[data-action="skills-mode"][data-mode="neo"]').click();
  await expect(page.locator("text=Neo Skills")).toBeVisible();
  await expect(page.locator("text=cand-1")).toBeVisible();
  await page.locator('[data-action="skill-payload-open"][data-payload-ref="payload-1"]').click();
  await expect(page.locator("#skill-payload-dialog")).toContainText("payload-1");
  await page.locator('#skill-payload-dialog [data-action="extension-dialog-close"]').click();
  await page.locator('[data-action="skill-neo-action"][data-endpoint="evaluate"]').first().click();
  await expect(page.locator("#toast")).toContainText("Neo Skills 操作已提交");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

test("tool-use alias renders the source-style tools surface on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/tool-use");

  await expect(page.locator('[data-page="extension-tools"]')).toBeVisible();
  await expect(page.getByRole("heading", { name: "Commands、Tools 与 MCP" })).toBeVisible();
  await expect(page.locator(".command-table")).toContainText("/weather");
  await expect(page.locator("#mcp-json")).toBeVisible();

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});

async function installExtensionMocks(page) {
  const lifecycle = {
    handlers: {
      handler_count: 1,
      handlers: [{ plugin_name: "weather", handler_name: "main", event_type: "Message", priority: 10, enabled: true }],
    },
    plugins: [
      {
        plugin_id: "weather",
        name: "Weather Plugin",
        version: "1.2.0",
        description: "Forecast plugin",
        state: "active",
        active: true,
        source: { kind: "python_compat", root_dir: "plugins/weather", reserved: false },
        capabilities: ["handler", "tool"],
        permissions: ["network"],
        config: { city: "Shanghai" },
        config_files: [{ filename: "config.json" }],
        readme: { markdown: "# Weather Docs\nSafe weather docs" },
        changelog: { markdown: "# Weather Changelog" },
      },
      {
        plugin_id: "broken",
        name: "Broken Plugin",
        version: "0.0.1",
        description: "Failed import",
        state: "failed",
        active: false,
        source: { kind: "python_compat", root_dir: "plugins/broken", reserved: false },
        capabilities: [],
        permissions: [],
      },
    ],
    operations: [],
  };
  const market = {
    plugins: [
      {
        plugin_id: "weather",
        name: "Weather",
        version: "1.3.0",
        description: "Weather market docs",
        repo_url: "https://example.com/weather",
        installed: false,
        installed_version: "1.2.0",
        compatibility: { compatible: true },
        package: { source: { kind: "repository" } },
        readme: { markdown: "# Weather market docs" },
      },
      {
        plugin_id: "news",
        name: "News",
        version: "0.2.0",
        repo_url: "https://example.com/news",
        installed: false,
        compatibility: { compatible: true },
        package: { source: { kind: "archive" } },
      },
    ],
    installed_plugins: [],
    operations: [],
  };
  const tools = {
    tools: [
      {
        name: "weather.lookup",
        description: "Lookup city",
        active: true,
        origin_name: "Weather Plugin",
        origin: "plugin",
        source: "plugin:weather",
        user_toggle_allowed: true,
        parameters: { type: "object" },
      },
      {
        name: "astr_kb_search",
        description: "Knowledge base search",
        active: true,
        origin_name: "AstrBot",
        origin: "internal",
        source: "internal",
        user_toggle_allowed: false,
      },
    ],
  };
  const commands = {
    commands: [
      {
        handler_full_name: "weather.main",
        plugin_name: "weather",
        handler_name: "main",
        command_type: "command",
        original_command: "weather",
        current_fragment: "/weather",
        effective_command: "/weather",
        aliases: ["forecast"],
        effective_aliases: ["/forecast"],
        permission: "admin",
        enabled: true,
        reserved: false,
        description: "Weather command",
        response: "ok",
        priority: 10,
      },
    ],
    conflicts: [{ command: "/weather", handlers: ["weather.main", "other.main"] }],
  };
  const mcp = {
    active_count: 1,
    servers: [
      {
        name: "docs",
        active: true,
        transport: "stdio",
        command: "npx",
        args: ["-y", "server"],
        valid: true,
        session_read_timeout_seconds: 60,
        client_capabilities: {},
      },
    ],
  };
  const skills = {
    skills: [
      {
        name: "writer",
        description: "Write docs",
        path: "skills/writer/SKILL.md",
        active: true,
        source_type: "local_only",
        source_label: "Local",
        local_exists: true,
        sandbox_exists: false,
      },
      {
        name: "preset",
        description: "Sandbox preset",
        path: "sandbox/preset/SKILL.md",
        active: true,
        source_type: "sandbox_only",
        source_label: "Sandbox",
        local_exists: false,
        sandbox_exists: true,
      },
    ],
    sandbox_cache: { ready: true, count: 1 },
  };
  const neoCandidates = { data: [{ id: "cand-1", skill_key: "writer", status: "pending", payload_ref: "payload-1" }] };
  const neoReleases = { data: [{ id: "rel-1", skill_key: "writer", stage: "stable", is_active: true }] };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.postDataJSON?.() || {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 0, rerank_provider_count: 0 },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: lifecycle.handlers.handler_count, handlers: lifecycle.handlers.handlers },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, {
        services: [
          { id: "plugin_lifecycle", label: "Plugin lifecycle", closure_level: "runtime" },
          { id: "plugin_market", label: "Plugin market", closure_level: "in_memory" },
          { id: "commands", label: "Commands", closure_level: "runtime" },
          { id: "tools", label: "Tools", closure_level: "runtime" },
          { id: "mcp", label: "MCP", closure_level: "in_memory" },
          { id: "skills", label: "Skills", closure_level: "in_memory" },
        ],
      });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [], recent_events: [] });
    }
    if (url.pathname === "/api/management/plugins/lifecycle") {
      return fulfillJson(route, lifecycle);
    }
    if (url.pathname === "/api/management/plugin-market") {
      return fulfillJson(route, market);
    }
    if (url.pathname === "/api/management/tools") {
      return fulfillJson(route, tools);
    }
    if (url.pathname === "/api/management/commands") {
      return fulfillJson(route, commands);
    }
    if (url.pathname === "/api/management/mcp/servers") {
      return fulfillJson(route, mcp);
    }
    if (url.pathname === "/api/management/skills") {
      return fulfillJson(route, skills);
    }
    if (url.pathname === "/api/skills/neo/candidates") {
      return fulfillJson(route, neoCandidates);
    }
    if (url.pathname === "/api/skills/neo/releases") {
      return fulfillJson(route, neoReleases);
    }
    if (url.pathname === "/api/skills/neo/payload") {
      return fulfillJson(route, { payload_ref: url.searchParams.get("payload_ref"), content: { skill_key: "writer" } });
    }
    if (url.pathname.startsWith("/api/skills/neo/")) {
      return fulfillJson(route, { accepted: true, payload: body });
    }
    if (url.pathname === "/api/management/plugins/lifecycle/action") {
      const plugin = lifecycle.plugins.find((item) => item.plugin_id === body.plugin_id);
      if (plugin) {
        if (body.action === "disable") plugin.active = false;
        if (body.action === "activate") plugin.active = true;
        if (body.action === "reload") plugin.state = "reloading";
      }
      lifecycle.operations.unshift({ action: body.action, plugin_id: body.plugin_id });
      return fulfillJson(route, { event: lifecycle.operations[0], catalog: lifecycle });
    }
    if (url.pathname === "/api/management/plugins/upload-plan") {
      return fulfillJson(route, { plugin_id: "weather", requires_unpack: true, entries: body.entries || [] });
    }
    if (url.pathname === "/api/management/plugins/source-plan") {
      return fulfillJson(route, { plugin_id: body.plugin_id, source: body.source || body });
    }
    if (url.pathname === "/api/management/plugins/config") {
      const plugin = lifecycle.plugins.find((item) => item.plugin_id === body.plugin_id);
      if (plugin) plugin.config = body.config || {};
      return fulfillJson(route, { changed: true, catalog: lifecycle });
    }
    if (url.pathname === "/api/management/plugins/config-file/read") {
      return fulfillJson(route, { plugin_id: body.plugin_id, filename: body.filename, config: { city: "Shanghai", enabled: true } });
    }
    if (url.pathname === "/api/management/plugins/config-file/write") {
      return fulfillJson(route, { written: true, plugin_id: body.plugin_id, filename: body.filename, config: body.config });
    }
    if (url.pathname === "/api/management/plugins/config-file/delete") {
      return fulfillJson(route, { deleted: true, plugin_id: body.plugin_id, filename: body.filename });
    }
    if (url.pathname === "/api/management/plugin-market/install-plan") {
      return fulfillJson(route, { plan: { action: "install", plugin_id: body.plugin_id } });
    }
    if (url.pathname === "/api/management/plugin-market/update-all-plan") {
      return fulfillJson(route, { plans: market.plugins.filter((plugin) => plugin.installed).map((plugin) => ({ plugin_id: plugin.plugin_id, action: "update" })) });
    }
    if (url.pathname === "/api/management/plugin-market/install") {
      const plugin = market.plugins.find((item) => item.plugin_id === body.plugin_id);
      if (plugin) {
        plugin.installed = true;
        plugin.pending_loader_reload = true;
        market.installed_plugins = [{ plugin_id: plugin.plugin_id, version: plugin.version }];
      }
      return fulfillJson(route, { operation: { action: "install", status: "completed" }, plugins: market.plugins, installed_plugins: market.installed_plugins });
    }
    if (url.pathname === "/api/management/plugin-market/update") {
      return fulfillJson(route, { operation: { action: "update", status: "completed", plugin_id: body.plugin_id } });
    }
    if (url.pathname === "/api/management/plugin-market/update-all") {
      return fulfillJson(route, { operations: [{ action: "update", status: "completed" }] });
    }
    if (url.pathname === "/api/management/plugin-market/uninstall") {
      const plugin = market.plugins.find((item) => item.plugin_id === body.plugin_id);
      if (plugin) plugin.installed = false;
      market.installed_plugins = market.installed_plugins.filter((item) => item.plugin_id !== body.plugin_id);
      return fulfillJson(route, { operation: { action: "uninstall", status: "completed" }, installed_plugins: market.installed_plugins });
    }
    if (url.pathname === "/api/management/commands/update") {
      const command = commands.commands.find((item) => item.plugin_name === body.plugin_name && item.handler_name === body.handler_name);
      if (command) {
        if (body.command) {
          command.current_fragment = body.command;
          command.effective_command = body.command;
        }
        if (typeof body.enabled === "boolean") command.enabled = body.enabled;
        if (body.permission) command.permission = body.permission;
      }
      return fulfillJson(route, { changed: true, command, catalog: commands });
    }
    if (url.pathname === "/api/management/tools/toggle") {
      const tool = tools.tools.find((item) => item.name === body.name);
      if (tool) tool.active = body.active;
      return fulfillJson(route, { name: body.name, active: body.active, catalog: tools });
    }
    if (url.pathname === "/api/management/mcp/servers/upsert") {
      const server = {
        name: body.name,
        active: body.server?.active !== false,
        transport: body.server?.transport || "stdio",
        command: body.server?.command || "",
        url: body.server?.url || "",
        args: body.server?.args || [],
        valid: true,
      };
      mcp.servers = [server, ...mcp.servers.filter((item) => item.name !== body.name)];
      mcp.active_count = mcp.servers.filter((server) => server.active).length;
      return fulfillJson(route, { changed: true, catalog: mcp });
    }
    if (url.pathname === "/api/management/mcp/servers/check") {
      return fulfillJson(route, { ok: true, message: "not probed in fake server" });
    }
    if (url.pathname === "/api/management/mcp/servers/sync") {
      return fulfillJson(route, { synced_servers: body.names || ["docs"], bridge_tools: ["mcp_docs_read_resource"] });
    }
    if (url.pathname === "/api/management/mcp/servers/delete") {
      mcp.servers = mcp.servers.filter((server) => server.name !== body.name);
      mcp.active_count = mcp.servers.filter((server) => server.active).length;
      return fulfillJson(route, { changed: true, catalog: mcp });
    }
    if (url.pathname === "/api/management/skills/activation") {
      const skill = skills.skills.find((item) => item.name === body.name);
      if (skill) skill.active = body.active;
      return fulfillJson(route, { name: body.name, active: body.active });
    }
    if (url.pathname === "/api/management/skills/install-plan") {
      return fulfillJson(route, { plan: { skill_name: "new_skill", entries: body.entries || [] } });
    }
    if (url.pathname === "/api/management/skills/install") {
      skills.skills.push({
        name: "new_skill",
        description: "Installed from upload",
        path: "skills/new_skill/SKILL.md",
        active: true,
        source_type: "local_only",
        source_label: "Local",
        local_exists: true,
        sandbox_exists: false,
      });
      return fulfillJson(route, { skill: skills.skills.at(-1), plan: { skill_name: "new_skill" } });
    }
    if (url.pathname === "/api/management/skills/delete-plan") {
      return fulfillJson(route, { plan: { skill_name: body.name, remove_local_dir: true } });
    }
    if (url.pathname === "/api/management/skills/delete") {
      skills.skills = skills.skills.filter((skill) => skill.name !== body.name);
      return fulfillJson(route, { deleted: true, plan: { skill_name: body.name } });
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
