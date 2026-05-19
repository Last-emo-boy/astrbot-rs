import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "provider-token");
  });
  await installProviderMocks(page);
});

test("provider route supports source-style sources models manual add and typed tabs", async ({ page }) => {
  await page.goto("/providers");

  await expect(page.getByRole("heading", { name: "模型提供商" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "提供商源" })).toBeVisible();
  await expect(page.locator("#provider-source-id")).toHaveValue("openai");
  await expect(page.locator(".provider-model-list")).toContainText("openai/gpt-4.1-mini");

  await page.locator('[data-action="provider-source-models"]').click();
  await expect(page.locator(".provider-model-list")).toContainText("o3-mini");

  await page.locator("#provider-model-search").fill("o3");
  await expect(page.locator(".provider-model-row.configured:visible")).toHaveCount(0);
  await expect(page.locator(".provider-model-list")).toContainText("o3-mini");
  await page.locator("#provider-model-search").fill("");

  await page.locator('[data-action="provider-model-add"][data-model="o3-mini"]').click();
  await expect(page.locator(".provider-model-list")).toContainText("openai/o3-mini");

  await page.locator('[data-action="provider-manual-open"]').click();
  await expect(page.locator("#provider-manual-model-dialog")).toBeVisible();
  await page.locator("#provider-manual-model").fill("gpt-4.1-mini");
  await page.locator('[data-action="provider-manual-add"]').click();
  await expect(page.locator("#toast")).toContainText("模型已存在");
  await page.locator("#provider-manual-model").fill("custom-model");
  await page.locator('[data-action="provider-manual-add"]').click();
  await expect(page.locator(".provider-model-list")).toContainText("openai/custom-model");
  const customModelRow = page.locator(".provider-model-row", { hasText: "openai/custom-model" });
  await expect(customModelRow).toContainText("vision");
  await expect(customModelRow).toContainText("tool");

  await page.locator("#provider-source-api-base").fill("https://api.changed.example/v1");
  await page.locator('[data-action="provider-source-models"]').click();
  await expect(page.locator("#provider-source-api-base")).toHaveValue("https://api.changed.example/v1");

  await page.locator('[data-action="provider-toggle"][data-provider="openai/custom-model"]').click();
  await page.locator('[data-action="provider-check"][data-provider="openai/custom-model"]').click();
  await expect(page.locator("#toast")).toContainText("测试通过");
  await expect(customModelRow).toContainText("可用");

  await page.locator('[data-action="provider-tab"][data-tab="embedding"]').click();
  await expect(page.getByRole("heading", { name: "嵌入" })).toBeVisible();
  await expect(page.locator(".provider-card")).toContainText("embedding");
  await expect(page.locator(".provider-card", { hasText: "embedding" })).toContainText("默认");
  await expect(page.locator(".provider-card", { hasText: "embedding" })).toContainText("dim 1024");
  await page.locator('[data-action="provider-edit-open"][data-provider="embedding"]').click();
  await expect(page.locator("#provider-edit-dialog")).toBeVisible();
  await page.locator('[data-action="provider-embedding-dim"]').click();
  await expect(page.locator("#provider-edit-dimensions")).toHaveValue("1536");
  await page.locator('#provider-edit-dialog [data-action="provider-dialog-close"]').click();
  await page.locator('[data-action="provider-copy"][data-provider="embedding"]').click();
  await expect(page.locator(".provider-card", { hasText: "embedding_copy" })).toBeVisible();

  await page.locator('[data-action="provider-dialog-open"]').first().click();
  await expect(page.locator("#provider-add-dialog")).toBeVisible();
  await expect(page.locator("#provider-add-dialog")).toContainText("OpenAI Embedding");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

async function installProviderMocks(page) {
  const catalog = {
    config_schema: {
      provider: {
        config_template: {
          OpenAI: {
            id: "openai",
            type: "openai_chat_completion",
            provider_type: "chat_completion",
            provider: "openai",
            enable: true,
            api_base: "https://api.openai.com/v1",
          },
          "OpenAI Embedding": {
            id: "embedding",
            type: "openai_embedding",
            provider_type: "embedding",
            provider: "openai",
            enable: true,
            model: "text-embedding-3-small",
            dimensions: 1024,
          },
        },
      },
    },
    provider_sources: [
      {
        id: "openai",
        type: "openai_chat_completion",
        provider_type: "chat_completion",
        provider: "openai",
        enable: true,
        api_base: "https://api.openai.com/v1",
      },
    ],
    providers: [
      {
        id: "openai/gpt-4.1-mini",
        type: "openai_chat_completion",
        provider_type: "chat_completion",
        provider_source_id: "openai",
        provider: "openai",
        model: "gpt-4.1-mini",
        enable: true,
      },
      {
        id: "embedding",
        type: "openai_embedding",
        provider_type: "embedding",
        provider: "openai",
        model: "text-embedding-3-small",
        dimensions: 1024,
        enable: true,
      },
    ],
  };

  const modelMetadata = {
    "gpt-4.1-mini": {
      modalities: { input: ["text", "image"] },
      tool_call: true,
      limit: { context: 1048576 },
    },
    "o3-mini": {
      modalities: { input: ["text"] },
      tool_call: true,
      reasoning: true,
      limit: { context: 200000 },
    },
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.postDataJSON?.() || {};

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: {
          chat_provider_count: catalog.providers.filter((provider) => provider.provider_type === "chat_completion").length,
          speech_to_text_provider_count: 0,
          text_to_speech_provider_count: 0,
          embedding_provider_count: 1,
          rerank_provider_count: 0,
          default_chat_provider_id: "openai/gpt-4.1-mini",
          default_embedding_provider_id: "embedding",
        },
        platforms: { platform_count: 1, platform_ids: ["webchat"], webchat_platform_count: 1 },
        plugins: { handler_count: 1 },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "providers", label: "Providers", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [] });
    }
    if (url.pathname === "/api/config/provider/template") {
      return fulfillJson(route, { status: "ok", data: catalog });
    }
    if (url.pathname === "/api/config/provider_sources/models") {
      return fulfillJson(route, {
        status: "ok",
        data: {
          models: ["gpt-4.1-mini", "o3-mini"],
          model_metadata: modelMetadata,
        },
      });
    }
    if (url.pathname === "/api/config/provider_sources/update") {
      const source = body.config || {};
      const originalId = body.original_id || source.id;
      catalog.provider_sources = [source, ...catalog.provider_sources.filter((item) => item.id !== originalId && item.id !== source.id)];
      catalog.providers = catalog.providers.map((provider) => {
        if (provider.provider_source_id !== originalId) return provider;
        return {
          ...provider,
          provider_source_id: source.id,
          api_base: source.api_base,
          key: source.key,
        };
      });
      return fulfillJson(route, { status: "ok", data: { changed: true } });
    }
    if (url.pathname === "/api/config/provider/new") {
      const provider = { ...body, enable: body.enable ?? body.enabled ?? false };
      catalog.providers = [provider, ...catalog.providers.filter((item) => item.id !== provider.id)];
      return fulfillJson(route, { status: "ok", data: { changed: true, provider } });
    }
    if (url.pathname === "/api/config/provider/update") {
      const provider = body.config || {};
      catalog.providers = catalog.providers.map((item) => (item.id === body.id ? provider : item));
      return fulfillJson(route, { status: "ok", data: { changed: true, provider } });
    }
    if (url.pathname === "/api/config/provider/check_one") {
      return fulfillJson(route, { status: "ok", data: { id: url.searchParams.get("id"), status: "available", error: null } });
    }
    if (url.pathname === "/api/config/provider/get_embedding_dim") {
      return fulfillJson(route, { status: "ok", data: { embedding_dimensions: 1536, dimension: 1536 } });
    }
    if (url.pathname === "/api/config/provider/delete") {
      catalog.providers = catalog.providers.filter((provider) => provider.id !== body.id);
      return fulfillJson(route, { status: "ok", data: { changed: true } });
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
