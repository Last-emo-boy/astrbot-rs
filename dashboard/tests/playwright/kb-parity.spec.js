import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("astrbot.managementToken", "kb-token");
  });
  await installKnowledgeMocks(page);
});

test("knowledge base route covers list detail upload retrieval settings document and legacy surfaces", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "mobile", "desktop exercises the full KB table and modal workflow");

  await page.goto("/knowledge-base");

  await expect(page.locator('[data-page="knowledge-list"]')).toBeVisible();
  await expect(page.locator(".kb-card")).toContainText("Docs");
  await expect(page.locator(".kb-card")).toContainText("Project docs");

  await page.locator('[data-action="kb-create-dialog-open"][data-mode="create"]').click();
  await expect(page.locator("#kb-form-dialog")).toBeVisible();
  await page.locator("#kb-form-id").fill("kb-new");
  await page.locator("#kb-form-name").fill("New KB");
  await page.locator("#kb-form-embedding").fill("embedding");
  await page.locator('#kb-form-dialog [data-action="kb-create"]').click();
  await expect(page.locator("#toast")).toContainText("知识库已创建");

  await page.goto("/knowledge-base/kb-1");
  await expect(page.locator('[data-page="knowledge-detail"]')).toBeVisible();
  await expect(page.locator(".knowledge-title-card")).toContainText("Docs");

  await page.locator("#kb-detail-tabs-documents-tab").click();
  await expect(page.locator("#kb-documents-table")).toContainText("Intro");
  await page.locator('[data-action="kb-upload-dialog-open"]').click();
  await expect(page.locator("#kb-upload-dialog")).toBeVisible();
  await page.locator("#kb-ingest-doc-id").fill("doc-upload");
  await page.locator("#kb-ingest-name").fill("Uploaded note");
  await page.locator("#kb-ingest-content").fill("Uploaded knowledge is searchable.");
  await page.locator('#kb-upload-dialog [data-action="kb-ingest"]').click();
  await expect(page.locator("#toast")).toContainText("Ingest 完成");
  await expect(page.locator("#kb-documents-table")).toContainText("Uploaded note");

  await page.locator('[data-action="kb-upload-dialog-open"]').click();
  await page.locator('[data-action="kb-upload-mode"][data-mode="url"]').click();
  await page.locator("#kb-ingest-doc-id").fill("doc-url");
  await page.locator("#kb-ingest-source-url").fill("https://example.invalid/guide");
  await page.locator("#kb-ingest-content").fill("URL import content for retrieval.");
  await page.locator('#kb-upload-dialog [data-action="kb-ingest"]').click();
  await expect(page.locator("#toast")).toContainText("Ingest 完成");
  await expect(page.locator("#kb-documents-table")).toContainText("Imported URL");

  await page.locator("#kb-detail-tabs-retrieval-tab").click();
  await page.locator("#kb-query").fill("retrieval");
  await page.locator('[data-action="kb-retrieve"]').click();
  await expect(page.locator(".retrieval-results-list")).toContainText("chunk-doc-url");
  await expect(page.locator(".retrieval-results-list")).toContainText("URL import content");

  await page.locator("#kb-detail-tabs-settings-tab").click();
  await page.locator("#kb-name").fill("Docs Saved");
  await page.locator('[data-action="kb-settings-save"]').click();
  await expect(page.locator("#toast")).toContainText("知识库设置已保存");
  await expect(page.locator(".knowledge-title-card")).toContainText("Docs Saved");

  await page.goto("/knowledge-base/kb-1/document/doc-url");
  await expect(page.locator('[data-page="knowledge-document-detail"]')).toBeVisible();
  await expect(page.locator("#kb-chunks-table")).toContainText("chunk-doc-url");
  await page.locator('[data-action="kb-chunk-view-open"]').first().click();
  await expect(page.locator("#kb-chunk-view-dialog")).toBeVisible();
  await page.locator('[data-action="kb-dialog-close"]').click();
  await page.locator('[data-action="kb-chunk-delete-dialog-open"]').first().click();
  await expect(page.locator("#kb-chunk-delete-dialog")).toBeVisible();
  await page.locator('#kb-chunk-delete-dialog [data-action="kb-chunk-delete"]').click();
  await expect(page.locator("#toast")).toContainText("Chunk 已删除");

  await page.goto("/alkaid/knowledge-base");
  await expect(page.locator('[data-page="legacy-alkaid-knowledge"]')).toBeVisible();
  await expect(page.locator(".notice-banner")).toContainText("建议迁移");
  await expect(page.locator('a[href="#/knowledge-base"]')).toContainText("使用新版知识库");

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(5_000);
});

test("knowledge base mobile list and legacy replacement remain reachable", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/knowledge-base");

  await expect(page.locator('[data-page="knowledge-list"]')).toBeVisible();
  await expect(page.locator(".kb-card")).toContainText("Docs");
  await page.locator('[data-action="kb-legacy-open"]').click();
  await expect(page.locator('[data-page="legacy-alkaid-knowledge"]')).toBeVisible();

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});

async function installKnowledgeMocks(page) {
  let knowledgeBases = [
    {
      kb_id: "kb-1",
      name: "Docs",
      description: "Project docs",
      emoji: "📚",
      embedding_provider_id: "embedding",
      rerank_provider_id: "rerank",
      chunk_size: 256,
      chunk_overlap: 32,
      top_k_dense: 50,
      top_k_sparse: 50,
      top_m_final: 5,
      stats: { doc_count: 1, chunk_count: 1 },
    },
  ];
  let documents = [
    {
      doc_id: "doc-1",
      kb_id: "kb-1",
      name: "Intro",
      file_type: "text",
      file_size: 128,
      chunk_count: 1,
      media_count: 0,
    },
  ];
  let chunksByDoc = {
    "doc-1": [
      {
        chunk_id: "chunk-doc-1",
        doc_id: "doc-1",
        kb_id: "kb-1",
        chunk_index: 0,
        content: "Project docs intro chunk.",
        char_count: 25,
        metadata: { doc_name: "Intro" },
      },
    ],
  };
  let uploadTask = {
    task_id: "upload-1",
    kb_id: "kb-1",
    kind: "upload",
    status: "completed",
    progress: { status: "completed", file_index: 0, file_total: 1, file_name: "intro.txt", stage: "completed", current: 1, total: 1 },
    result: { document_ids: ["doc-upload"], doc_count: 1, chunk_count: 1 },
    error: null,
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    let body = {};
    try {
      body = request.postDataJSON?.() || {};
    } catch {
      body = {};
    }

    if (url.pathname === "/api/management/status") {
      return fulfillJson(route, {
        providers: { chat_provider_count: 1, embedding_provider_count: 1, rerank_provider_count: 1, default_embedding_provider_id: "embedding", default_rerank_provider_id: "rerank" },
        platforms: { platform_count: 1, platform_ids: ["webchat"] },
        plugins: { handler_count: 0, handlers: [] },
      });
    }
    if (url.pathname === "/api/management/dashboard/capabilities") {
      return fulfillJson(route, { services: [{ id: "knowledge", label: "Knowledge Base", closure_level: "runtime" }] });
    }
    if (url.pathname === "/api/management/stats") {
      return fulfillJson(route, { total_messages: 0, platform_counts: [], provider_usage: [], recent_events: [] });
    }
    if (url.pathname === "/api/management/logs") {
      return fulfillJson(route, { snapshot: { entries: [] } });
    }
    if (url.pathname === "/api/management/kb/catalog") {
      return fulfillJson(route, { knowledge_bases: knowledgeBases });
    }
    if (url.pathname === "/api/management/kb/get") {
      const kb = knowledgeBases.find((item) => item.kb_id === body.kb_id) || null;
      return fulfillJson(route, { knowledge_base: kb });
    }
    if (url.pathname === "/api/management/kb/create") {
      const kb = {
        kb_id: body.kb_id,
        name: body.name,
        description: body.description || "",
        emoji: body.emoji || "📚",
        embedding_provider_id: body.embedding_provider_id || "embedding",
        rerank_provider_id: body.rerank_provider_id || null,
        chunk_size: body.chunk_size || 512,
        chunk_overlap: body.chunk_overlap || 50,
        top_k_dense: 50,
        top_k_sparse: 50,
        top_m_final: 5,
        stats: { doc_count: 0, chunk_count: 0 },
      };
      knowledgeBases = [kb, ...knowledgeBases];
      return fulfillJson(route, { knowledge_base: kb });
    }
    if (url.pathname === "/api/management/kb/update") {
      knowledgeBases = knowledgeBases.map((kb) => (
        kb.kb_id === body.kb_id
          ? { ...kb, name: body.name || kb.name, description: body.description ?? kb.description, emoji: body.emoji || kb.emoji, rerank_provider_id: body.rerank_provider_id ?? kb.rerank_provider_id }
          : kb
      ));
      return fulfillJson(route, { knowledge_base: knowledgeBases.find((kb) => kb.kb_id === body.kb_id) });
    }
    if (url.pathname === "/api/management/kb/document/list") {
      return fulfillJson(route, { documents: documents.filter((doc) => doc.kb_id === body.kb_id) });
    }
    if (url.pathname === "/api/management/kb/document/get") {
      return fulfillJson(route, documents.find((doc) => doc.doc_id === body.doc_id) || null);
    }
    if (url.pathname === "/api/management/kb/chunk/list") {
      return fulfillJson(route, { chunks: chunksByDoc[body.doc_id] || [] });
    }
    if (url.pathname === "/api/management/kb/chunk/delete") {
      for (const docId of Object.keys(chunksByDoc)) {
        chunksByDoc[docId] = chunksByDoc[docId].filter((chunk) => chunk.chunk_id !== body.chunk_id);
      }
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/kb/document/delete") {
      documents = documents.filter((doc) => doc.doc_id !== body.doc_id);
      delete chunksByDoc[body.doc_id];
      return fulfillJson(route, { ok: true });
    }
    if (url.pathname === "/api/management/kb/ingest") {
      const docId = body.doc_id || `doc-${documents.length + 1}`;
      const name = body.name || body.source_url || "Uploaded";
      const doc = {
        doc_id: docId,
        kb_id: body.kb_id,
        name,
        file_type: body.source_kind || "text",
        file_size: String(body.content || "").length,
        chunk_count: 1,
        media_count: 0,
      };
      const chunk = {
        chunk_id: `chunk-${docId}`,
        doc_id: docId,
        kb_id: body.kb_id,
        chunk_index: 0,
        content: String(body.content || "").replace(/<[^>]*>/g, ""),
        char_count: String(body.content || "").length,
        metadata: { doc_name: name },
      };
      documents = [doc, ...documents.filter((item) => item.doc_id !== docId)];
      chunksByDoc[docId] = [chunk];
      updateStats(body.kb_id);
      uploadTask = { ...uploadTask, task_id: `upload-${docId}`, result: { document_ids: [docId], doc_count: 1, chunk_count: 1 } };
      return fulfillJson(route, { document: doc, chunks: [chunk] });
    }
    if (url.pathname === "/api/management/kb/retrieve") {
      const hits = Object.values(chunksByDoc)
        .flat()
        .filter((chunk) => !body.kb_ids?.length || body.kb_ids.includes(chunk.kb_id))
        .map((chunk, index) => ({ ...chunk, score: 1 - index * 0.1, doc_name: documents.find((doc) => doc.doc_id === chunk.doc_id)?.name }));
      return fulfillJson(route, { mode: "hybrid_vector", query: body.query, results: hits.slice(0, body.top_k || 5) });
    }
    if (url.pathname === "/api/management/kb/upload/plan") {
      uploadTask = { ...uploadTask, task_id: body.task_id, kb_id: body.kb_id, kind: body.kind || "upload", status: "pending" };
      return fulfillJson(route, { task: uploadTask });
    }
    if (url.pathname === "/api/management/kb/upload/progress") {
      uploadTask = { ...uploadTask, status: "processing", progress: { status: "processing", file_index: 0, file_total: body.file_total || 1, file_name: body.file_name || "intro.txt", stage: body.stage || "embedding", current: body.current || 0, total: body.total || 1 } };
      return fulfillJson(route, { task: uploadTask });
    }
    if (url.pathname === "/api/management/kb/upload/complete") {
      uploadTask = { ...uploadTask, status: "completed", result: { document_ids: body.document_ids || [], doc_count: body.document_ids?.length || 0, chunk_count: body.chunk_count || 0 } };
      return fulfillJson(route, { task: uploadTask });
    }
    if (url.pathname.startsWith("/api/management/kb/upload/progress/")) {
      return fulfillJson(route, { task: uploadTask });
    }
    if (url.pathname === "/api/management/config/current") {
      return fulfillJson(route, { config: { provider_settings: {} } });
    }
    if (url.pathname === "/api/management/config/apply") {
      return fulfillJson(route, { execution: { action: "applied" } });
    }

    return fulfillJson(route, { ok: true });
  });

  function updateStats(kbId) {
    const docCount = documents.filter((doc) => doc.kb_id === kbId).length;
    const chunkCount = documents
      .filter((doc) => doc.kb_id === kbId)
      .reduce((sum, doc) => sum + (chunksByDoc[doc.doc_id]?.length || 0), 0);
    knowledgeBases = knowledgeBases.map((kb) => kb.kb_id === kbId ? { ...kb, stats: { doc_count: docCount, chunk_count: chunkCount } } : kb);
  }
}

async function fulfillJson(route, body, status = 200) {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}
