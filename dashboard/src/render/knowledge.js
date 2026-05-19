import { escapeHtml } from "../dom.js";
import { state } from "../state.js";
import {
  button,
  chip,
  confirmDialog,
  dataTable,
  dialog,
  formField,
  metric,
  pill,
  tabs,
  uiState,
} from "./shared.js";

const EMOJI_GROUPS = [
  ["books", ["📚", "📖", "📕", "📗", "📘", "📙", "📓", "📔", "📒", "📑", "🗂️", "📂", "📁", "🗃️", "🗄️"]],
  ["objects", ["💡", "🔬", "🔭", "🏆", "🎯", "🎓", "🔑", "🔒", "🔔", "🔨", "🛠️", "⚙️", "🧭", "🧪", "🧬"]],
  ["symbols", ["⭐", "🌟", "✨", "💫", "⚡", "🔥", "✅", "📌", "📎", "🔖", "💎", "🧩", "🪄", "🌐", "🚀"]],
];

const DETAIL_TABS = [
  ["overview", "概览"],
  ["documents", "文档"],
  ["retrieval", "检索"],
  ["settings", "设置"],
];

export function renderKnowledge() {
  if (state.routeSourcePath === "/alkaid/knowledge-base") {
    return renderLegacyKnowledge();
  }
  if (state.routeParams?.docId) {
    return renderDocumentDetail();
  }
  if (state.routeParams?.kbId) {
    return renderKnowledgeDetail();
  }
  return renderKnowledgeList();
}

function renderKnowledgeList() {
  const catalog = state.kb || { knowledge_bases: [] };
  const knowledgeBases = catalog.knowledge_bases || [];
  const totalDocs = knowledgeBases.reduce((sum, kb) => sum + Number(kb.stats?.doc_count || kb.doc_count || 0), 0);
  const totalChunks = knowledgeBases.reduce((sum, kb) => sum + Number(kb.stats?.chunk_count || kb.chunk_count || 0), 0);

  return `
    <div class="knowledge-page kb-list-page" data-page="knowledge-list">
      <header class="knowledge-header">
        <div>
          <div class="eyebrow">Native Knowledge Base</div>
          <h2>知识库</h2>
          <p>创建、配置并检索 AstrBot RS 的原生 KB 索引。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "创建知识库", action: "kb-create-dialog-open", icon: "+", attrs: { "data-mode": "create" } })}
          ${button({ label: "刷新", action: "load-kb", variant: "secondary", icon: "↻" })}
          <a class="button ghost" href="https://astrbot.app/use/knowledge-base.html" target="_blank" rel="noopener noreferrer">文档</a>
        </div>
      </header>

      <div class="grid cols-4">
        ${metric("Knowledge Bases", knowledgeBases.length, "catalog")}
        ${metric("Documents", totalDocs, "indexed docs")}
        ${metric("Chunks", totalChunks, "vector chunks")}
        ${metric("Upload", state.kbUploadTask?.task?.status || "-", state.kbUploadTask?.task?.task_id || "no task")}
      </div>

      ${catalog.unavailable ? uiState({ state: "error", title: "KB management unavailable", message: catalog.unavailable }) : ""}
      ${knowledgeBases.length ? renderKnowledgeCards(knowledgeBases) : renderEmptyKnowledgeList()}

      <div class="knowledge-footer-link">
        <button type="button" class="button ghost" data-action="kb-legacy-open">切换到旧版知识库</button>
      </div>

      ${renderKbFormDialog()}
      ${renderEmojiDialog()}
      ${renderKbDeleteDialog()}
    </div>
  `;
}

function renderKnowledgeCards(knowledgeBases) {
  return `
    <section class="kb-grid" aria-label="Knowledge base list">
      ${knowledgeBases.map((kb) => {
        const name = kbName(kb);
        const description = kb.description || "暂无描述";
        return `
          <article class="kb-card" data-kb="${escapeHtml(kb.kb_id)}">
            <a class="kb-card-main" href="#/knowledge-base/${encodeURIComponent(kb.kb_id)}" data-action="kb-open" data-kb="${escapeHtml(kb.kb_id)}">
              <span class="kb-emoji" aria-hidden="true">${escapeHtml(kb.emoji || "📚")}</span>
              <h3>${escapeHtml(name)}</h3>
              <p>${escapeHtml(description)}</p>
              <div class="kb-stats">
                <span>▤ ${escapeHtml(kb.stats?.doc_count ?? kb.doc_count ?? 0)} 文档</span>
                <span>☰ ${escapeHtml(kb.stats?.chunk_count ?? kb.chunk_count ?? 0)} Chunks</span>
              </div>
              <div class="ui-chip-row">
                ${chip(kb.embedding_provider_id || "embedding unset", "label")}
                ${kb.rerank_provider_id ? chip(`rerank ${kb.rerank_provider_id}`, "label") : chip("no rerank", "warn")}
              </div>
            </a>
            <footer class="kb-card-actions">
              <button class="button secondary icon" type="button" title="编辑" data-action="kb-create-dialog-open" data-mode="edit" data-kb="${escapeHtml(kb.kb_id)}">✎</button>
              <button class="button ghost icon" type="button" title="删除" data-action="kb-delete-dialog-open" data-kb="${escapeHtml(kb.kb_id)}">×</button>
            </footer>
          </article>
        `;
      }).join("")}
    </section>
  `;
}

function renderEmptyKnowledgeList() {
  return `
    <section class="panel kb-empty-panel">
      ${uiState({
        state: "empty",
        title: "暂无知识库",
        message: "创建知识库后，可以上传文档、写入 URL 内容并运行检索。",
        action: { label: "创建知识库", action: "kb-create-dialog-open" },
      })}
    </section>
  `;
}

function renderKnowledgeDetail() {
  const kb = currentKb();
  if (!kb) {
    return `
      <div class="knowledge-page" data-page="knowledge-detail">
        ${renderKnowledgeBreadcrumb(null)}
        ${uiState({ state: "empty", title: "未找到知识库", message: `当前路由参数：${state.routeParams?.kbId || "-"}` })}
      </div>
    `;
  }

  const activeTab = state.kbActiveTab || "overview";
  return `
    <div class="knowledge-page kb-detail-page" data-page="knowledge-detail" data-kb="${escapeHtml(kb.kb_id)}">
      ${renderKnowledgeBreadcrumb(kb)}
      <header class="knowledge-title-card">
        <div class="kb-title-left">
          <span class="kb-emoji large" aria-hidden="true">${escapeHtml(kb.emoji || "📚")}</span>
          <div>
            <h2>${escapeHtml(kbName(kb))}</h2>
            <p>${escapeHtml(kb.description || "暂无描述")}</p>
            <div class="ui-chip-row">
              ${chip(kb.kb_id, "label")}
              ${chip(`embedding ${kb.embedding_provider_id || "-"}`, "label")}
              ${kb.rerank_provider_id ? chip(`rerank ${kb.rerank_provider_id}`, "label") : chip("no rerank", "warn")}
            </div>
          </div>
        </div>
        <div class="banner-actions">
          ${button({ label: "编辑", action: "kb-create-dialog-open", variant: "secondary", icon: "✎", attrs: { "data-mode": "edit", "data-kb": kb.kb_id } })}
          ${button({ label: "刷新", action: "kb-detail-refresh", variant: "ghost", icon: "↻", attrs: { "data-kb": kb.kb_id } })}
        </div>
      </header>

      <div class="grid cols-4">
        ${metric("Documents", kb.stats?.doc_count ?? 0, "current KB")}
        ${metric("Chunks", kb.stats?.chunk_count ?? 0, "current KB")}
        ${metric("Chunk Size", kb.chunk_size ?? "-", `${kb.chunk_overlap ?? 0} overlap`)}
        ${metric("Retrieval", `${kb.top_m_final ?? 5} final`, `${kb.top_k_dense ?? 50}/${kb.top_k_sparse ?? 50}`)}
      </div>

      ${knowledgeTabs({
        id: "kb-detail-tabs",
        activeId: activeTab,
        items: DETAIL_TABS.map(([id, label]) => ({
          id,
          label: id === "documents" ? `${label} (${kb.stats?.doc_count ?? 0})` : label,
          body: tabBody(id, kb),
        })),
      })}

      ${renderKbFormDialog(kb)}
      ${renderEmojiDialog()}
      ${renderDocumentDeleteDialog()}
      ${renderChunkDeleteDialog()}
      ${renderTavilyDialog()}
    </div>
  `;
}

function knowledgeTabs(config) {
  return tabs(config).replaceAll(
    /data-tab-target="#kb-detail-tabs-([a-z-]+)-panel"/g,
    'data-action="kb-tab" data-tab="$1" data-tab-target="#kb-detail-tabs-$1-panel"',
  );
}

function tabBody(id, kb) {
  if (id === "documents") return renderDocumentsTab(kb);
  if (id === "retrieval") return renderRetrievalTab(kb);
  if (id === "settings") return renderSettingsTab(kb);
  return renderOverviewTab(kb);
}

function renderKnowledgeBreadcrumb(kb, doc = null) {
  return `
    <nav class="kb-breadcrumb" aria-label="Knowledge breadcrumb">
      <a href="#/knowledge-base">知识库</a>
      ${kb ? `<span>/</span><a href="#/knowledge-base/${encodeURIComponent(kb.kb_id)}">${escapeHtml(kbName(kb))}</a>` : ""}
      ${doc ? `<span>/</span><span aria-current="page">${escapeHtml(docName(doc))}</span>` : ""}
    </nav>
  `;
}

function renderOverviewTab(kb) {
  return `
    <div class="grid cols-2">
      <section class="panel">
        <h2>基本信息</h2>
        <div class="status-list">
          ${statusRow("名称", kbName(kb))}
          ${statusRow("描述", kb.description || "暂无描述")}
          ${statusRow("Emoji", kb.emoji || "📚")}
          ${statusRow("KB ID", kb.kb_id)}
        </div>
      </section>
      <section class="panel">
        <h2>模型与统计</h2>
        <div class="status-list">
          ${statusRow("Embedding Provider", kb.embedding_provider_id || "-")}
          ${statusRow("Rerank Provider", kb.rerank_provider_id || "未设置")}
          ${statusRow("文档数", kb.stats?.doc_count ?? 0)}
          ${statusRow("Chunk 数", kb.stats?.chunk_count ?? 0)}
        </div>
      </section>
    </div>
  `;
}

function renderDocumentsTab(kb) {
  const docs = state.kbDocuments?.documents || [];
  return `
    <section class="panel documents-tab">
      <div class="panel-title-row">
        <div>
          <h2>文档管理</h2>
          <p class="empty">支持文本、URL 内容写入；真实 multipart 文件上传仍通过 upload task facade 暴露进度。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "读取文档", action: "kb-documents", variant: "ghost", icon: "↻", attrs: { "data-kb": kb.kb_id } })}
          ${button({ label: "上传", action: "kb-upload-dialog-open", icon: "↑" })}
        </div>
      </div>
      <div class="extension-toolbar">
        <input id="kb-document-search" placeholder="搜索文档..." value="${escapeHtml(state.kbDocumentSearch || "")}" data-action="kb-document-search" />
      </div>
      ${renderDocumentTable(kb, docs)}
      ${renderUploadDialog(kb)}
    </section>
  `;
}

function renderDocumentTable(kb, docs) {
  const filtered = filterBySearch(docs, state.kbDocumentSearch, (doc) => `${docName(doc)} ${doc.doc_id} ${doc.file_type}`);
  return dataTable({
    id: "kb-documents-table",
    columns: [
      {
        key: "name",
        label: "名称",
        html: true,
        render: (doc) => `
          <div class="kb-doc-name">
            <span>${fileIcon(doc.file_type)}</span>
            <div><strong>${escapeHtml(docName(doc))}</strong><small>${escapeHtml(doc.doc_id)}</small></div>
          </div>
        `,
      },
      { key: "file_type", label: "类型" },
      { key: "file_size", label: "大小", render: (doc) => formatFileSize(doc.file_size) },
      { key: "chunk_count", label: "Chunks" },
      {
        key: "actions",
        label: "操作",
        html: true,
        render: (doc) => `
          <div class="button-cell">
            <a class="button secondary" href="#/knowledge-base/${encodeURIComponent(kb.kb_id)}/document/${encodeURIComponent(doc.doc_id)}" data-action="kb-document-open" data-kb="${escapeHtml(kb.kb_id)}" data-doc="${escapeHtml(doc.doc_id)}">查看</a>
            <button class="button ghost" type="button" data-action="kb-document-delete-dialog-open" data-doc="${escapeHtml(doc.doc_id)}" data-name="${escapeHtml(docName(doc))}">删除</button>
          </div>
        `,
      },
    ],
    rows: filtered,
    emptyMessage: "暂无文档。点击上传写入文本或 URL 内容。",
    rowKey: "doc_id",
    pagination: { page: 1, pageSize: 10, total: filtered.length },
  });
}

function hiddenKbId(kb) {
  return `<input id="kb-id" type="hidden" value="${escapeHtml(kb.kb_id)}" />`;
}

function renderUploadDialog(kb) {
  const open = state.kbDialog === "upload";
  const uploadMode = state.kbUploadMode || "file";
  const task = state.kbUploadTask?.task || null;
  return dialog({
    id: "kb-upload-dialog",
    title: "上传文档",
    open,
    maxWidth: "760px",
    body: `
      <div class="ui-tab-list upload-mode-switch" role="tablist">
        <button type="button" class="${uploadMode === "file" ? "active" : ""}" data-action="kb-upload-mode" data-mode="file">文件上传</button>
        <button type="button" class="${uploadMode === "url" ? "active" : ""}" data-action="kb-upload-mode" data-mode="url">从 URL</button>
      </div>
      ${hiddenKbId(kb)}
      ${uploadMode === "url" ? renderUrlUploadForm(kb) : renderFileUploadForm(kb)}
      ${renderUploadTaskPanel(task)}
    `,
    actions: [
      { label: "取消", variant: "ghost", action: "kb-dialog-close" },
      { label: uploadMode === "url" ? "写入 URL 内容" : "写入文本内容", action: "kb-ingest", variant: "primary" },
    ],
  });
}

function renderFileUploadForm(kb) {
  return `
    <div class="upload-dropzone">
      <div class="upload-icon">↑</div>
      <strong>选择或拖入文件</strong>
      <p>.txt, .md, .pdf, .docx, .xls, .xlsx，最多 10 个文件。当前 RS facade 使用下方文本内容执行 ingest。</p>
      ${formField({ id: "kb-upload-file", label: "文件选择", type: "file", accept: ".txt,.md,.pdf,.docx,.xls,.xlsx", attrs: { multiple: true } })}
    </div>
    <div class="form-grid cols-2 mt-16">
      ${formField({ id: "kb-ingest-doc-id", label: "Doc ID", value: state.kbUploadDraft?.docId || `doc-${safeId(kbName(kb))}` })}
      ${formField({ id: "kb-ingest-name", label: "Name", value: state.kbUploadDraft?.name || "Dashboard notes" })}
      ${formField({ id: "kb-upload-chunk-size", label: "Chunk size", type: "number", value: kb.chunk_size || 512 })}
      ${formField({ id: "kb-upload-chunk-overlap", label: "Chunk overlap", type: "number", value: kb.chunk_overlap || 50 })}
      ${formField({ id: "kb-upload-batch-size", label: "Batch size", type: "number", value: 32 })}
      ${formField({ id: "kb-upload-tasks-limit", label: "Tasks limit", type: "number", value: 3 })}
    </div>
    <input id="kb-ingest-source-kind" type="hidden" value="file" />
    <div class="form-row">
      <label for="kb-ingest-content">Content</label>
      <textarea id="kb-ingest-content">AstrBot dashboard knowledge ingest stores documents and chunks in the management KB store.</textarea>
    </div>
    <label class="check-row"><input id="kb-ingest-clean-html" type="checkbox" /> Clean HTML tags</label>
  `;
}

function renderUrlUploadForm(kb) {
  return `
    <div class="notice-banner">
      <span>${pill("Beta", "warn")}</span>
      <span>URL 抽取依赖 Tavily/Web search 配置；当前可将网页内容粘贴到 Content 后直接写入。</span>
      ${button({ label: "配置 Tavily Key", action: "kb-tavily-dialog-open", variant: "secondary" })}
    </div>
    <div class="form-grid cols-2 mt-16">
      ${formField({ id: "kb-ingest-doc-id", label: "Doc ID", value: state.kbUploadDraft?.docId || `url-${safeId(kbName(kb))}` })}
      ${formField({ id: "kb-ingest-name", label: "Name", value: state.kbUploadDraft?.name || "Imported URL" })}
      ${formField({ id: "kb-upload-chunk-size", label: "Chunk size", type: "number", value: kb.chunk_size || 512 })}
      ${formField({ id: "kb-upload-chunk-overlap", label: "Chunk overlap", type: "number", value: kb.chunk_overlap || 50 })}
      ${formField({ id: "kb-url-cleaning-provider", label: "Cleaning provider", value: "", placeholder: "optional chat provider id" })}
      ${formField({ id: "kb-url-enable-cleaning", label: "Enable cleaning", type: "checkbox", value: false })}
    </div>
    <input id="kb-ingest-source-kind" type="hidden" value="url" />
    ${formField({ id: "kb-ingest-source-url", label: "Source URL", value: "", placeholder: "https://example.com/docs" })}
    <div class="form-row">
      <label for="kb-ingest-content">Content</label>
      <textarea id="kb-ingest-content" placeholder="粘贴网页正文或抽取后的文本"></textarea>
    </div>
    <label class="check-row"><input id="kb-ingest-clean-html" type="checkbox" checked /> Clean HTML tags</label>
  `;
}

function renderUploadTaskPanel(task) {
  const progress = task?.progress;
  const current = progress?.current ?? 0;
  const total = progress?.total ?? 100;
  const percent = total ? Math.min(100, Math.round((current / total) * 100)) : 0;
  return `
    <section class="upload-task-panel">
      <div class="panel-title-row">
        <h3>上传任务进度</h3>
        ${button({ label: "轮询", action: "kb-upload-poll", variant: "ghost", icon: "↻" })}
      </div>
      <div class="form-grid cols-4">
        ${formField({ id: "kb-task-id", label: "Task ID", value: task?.task_id || `upload-${Date.now()}` })}
        ${formField({ id: "kb-file-name", label: "File name", value: progress?.file_name || "intro.txt" })}
        ${formField({ id: "kb-file-total", label: "File total", type: "number", value: task?.file_total || 1 })}
        ${formField({ id: "kb-upload-stage", label: "Stage", type: "select", value: progress?.stage || "embedding", options: ["queued", "extracting", "cleaning", "parsing", "chunking", "embedding", "metadata"] })}
        ${formField({ id: "kb-progress-current", label: "Current", type: "number", value: current || 1 })}
        ${formField({ id: "kb-progress-total", label: "Total", type: "number", value: total || 2 })}
      </div>
      <div class="progress-track" aria-label="Upload progress"><span style="width:${escapeHtml(percent)}%"></span></div>
      <div class="actions">
        ${button({ label: "Plan", action: "kb-upload-plan", variant: "secondary" })}
        ${button({ label: "Progress", action: "kb-upload-progress", variant: "secondary" })}
        ${button({ label: "Complete", action: "kb-upload-complete" })}
        ${button({ label: "Fail", action: "kb-upload-fail", variant: "ghost" })}
      </div>
    </section>
  `;
}

function renderRetrievalTab(kb) {
  const results = state.kbRetrieval?.results || [];
  const hasSearched = Boolean(state.kbRetrieval);
  return `
    <section class="panel retrieval-tab">
      <div class="panel-title-row">
        <div>
          <h2>知识库检索</h2>
          <p class="empty">对当前 KB 运行 hybrid vector 检索，可按源端 topK/debug 控制表面操作。</p>
        </div>
        ${button({ label: "搜索", action: "kb-retrieve", icon: "⌕", attrs: { "data-kb": kb.kb_id } })}
      </div>
      <div class="form-grid cols-3">
        <div class="form-row wide-field">
          <label for="kb-query">Query</label>
          <textarea id="kb-query" rows="3" placeholder="输入检索问题">${escapeHtml(state.kbQuery || "project docs")}</textarea>
        </div>
        ${formField({ id: "kb-retrieve-top-k", label: "Top K", type: "number", value: state.kbTopK || 5, hint: "最终返回数量" })}
        ${formField({ id: "kb-retrieve-debug", label: "Debug (t-SNE)", type: "checkbox", value: Boolean(state.kbDebugMode) })}
      </div>
      <input id="kb-retrieve-kb-ids" type="hidden" value="${escapeHtml(kb.kb_id)}" />
      ${hasSearched ? renderRetrievalResults(results) : uiState({ state: "empty", title: "尚未检索", message: "输入查询后点击搜索。" })}
    </section>
  `;
}

function renderRetrievalResults(results) {
  if (!results.length) {
    return uiState({ state: "empty", title: "没有检索结果", message: "尝试更换 query 或先写入文档。" });
  }
  return `
    <div class="results-section">
      <div class="panel-title-row">
        <h3>检索结果</h3>
        ${chip(`${results.length} results`, "label")}
      </div>
      <div class="retrieval-results-list">
        ${results.map((hit, index) => `
          <article class="retrieval-result-card">
            <header>
              <div>
                ${chip(`#${index + 1}`, "label")}
                <strong>Chunk ${escapeHtml(hit.chunk_index ?? index)}</strong>
                <span>${escapeHtml(hit.doc_name || hit.doc_id || "-")}</span>
                <small>${escapeHtml(hit.chunk_id || "")}</small>
              </div>
              ${scoreChip(hit.score)}
            </header>
            <pre>${escapeHtml(hit.content || "")}</pre>
          </article>
        `).join("")}
      </div>
    </div>
  `;
}

function renderSettingsTab(kb) {
  return `
    <section class="panel settings-tab">
      <div class="panel-title-row">
        <div>
          <h2>知识库设置</h2>
          <p class="empty">Embedding Provider 创建后不建议修改；变更会使现有向量失效。</p>
        </div>
        ${button({ label: "保存设置", action: "kb-settings-save", icon: "✓", attrs: { "data-kb": kb.kb_id } })}
      </div>
      <div class="form-grid cols-2">
        ${formField({ id: "kb-id", label: "KB ID", value: kb.kb_id, readonly: true })}
        ${formField({ id: "kb-name", label: "名称", value: kbName(kb) })}
        ${formField({ id: "kb-description", label: "描述", type: "textarea", rows: 3, value: kb.description || "" })}
        ${formField({ id: "kb-emoji", label: "Emoji", value: kb.emoji || "📚" })}
        ${formField({ id: "kb-chunk-size", label: "Chunk size", type: "number", value: kb.chunk_size || 512 })}
        ${formField({ id: "kb-chunk-overlap", label: "Chunk overlap", type: "number", value: kb.chunk_overlap || 50 })}
        ${formField({ id: "kb-top-k-dense", label: "Top K Dense", type: "number", value: kb.top_k_dense || 50, disabled: true, hint: "后端 update facade 暂未开放写入" })}
        ${formField({ id: "kb-top-k-sparse", label: "Top K Sparse", type: "number", value: kb.top_k_sparse || 50, disabled: true, hint: "后端 update facade 暂未开放写入" })}
        ${formField({ id: "kb-embedding", label: "Embedding Provider", value: kb.embedding_provider_id || "", readonly: true, hint: "创建后锁定，需更换时建议新建 KB 并重新上传文档" })}
        ${formField({ id: "kb-rerank", label: "Rerank Provider", value: kb.rerank_provider_id || "" })}
      </div>
      <div class="notice-banner warn">
        <span>${pill("注意", "warn")}</span>
        <span>修改 chunk 设置只影响后续写入；已有 chunks 不会自动重建。</span>
      </div>
    </section>
  `;
}

function renderDocumentDetail() {
  const kb = currentKb();
  const doc = currentDocument();
  const chunks = state.kbChunks?.chunks || [];
  const selectedChunk = chunks.find((chunk) => chunk.chunk_id === state.kbSelectedChunkId) || chunks[0] || null;
  return `
    <div class="knowledge-page document-detail-page" data-page="knowledge-document-detail">
      ${renderKnowledgeBreadcrumb(kb, doc)}
      <header class="knowledge-title-card">
        <div class="kb-title-left">
          <span class="kb-doc-icon" aria-hidden="true">${fileIcon(doc?.file_type)}</span>
          <div>
            <h2>${escapeHtml(doc ? docName(doc) : state.routeParams?.docId || "Document")}</h2>
            <p>${escapeHtml(doc?.doc_id || state.routeParams?.docId || "-")}</p>
          </div>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新 Chunks", action: "kb-chunks", variant: "secondary", icon: "↻", attrs: { "data-doc": state.routeParams?.docId || "" } })}
          ${kb ? `<a class="button ghost" href="#/knowledge-base/${encodeURIComponent(kb.kb_id)}">返回 KB</a>` : `<a class="button ghost" href="#/knowledge-base">返回列表</a>`}
        </div>
      </header>

      ${doc ? renderDocumentInfo(doc) : uiState({ state: "empty", title: "文档详情未加载", message: "后端返回文档详情为空，可刷新或从 KB 文档列表进入。" })}

      <section class="panel">
        <div class="panel-title-row">
          <div>
            <h2>Chunks</h2>
            <p class="empty">显示文档分块、字符数、内容预览和删除操作。</p>
          </div>
          ${chip(`${chunks.length} chunks`, "label")}
        </div>
        ${renderChunkTable(chunks)}
      </section>

      ${selectedChunk ? renderChunkViewDialog(selectedChunk) : ""}
      ${renderChunkDeleteDialog()}
    </div>
  `;
}

function renderDocumentInfo(doc) {
  return `
    <section class="panel">
      <h2>文档信息</h2>
      <div class="grid cols-4 document-info-grid">
        ${statusBox("名称", docName(doc))}
        ${statusBox("类型", doc.file_type || "-")}
        ${statusBox("大小", formatFileSize(doc.file_size))}
        ${statusBox("Chunks", doc.chunk_count || 0)}
      </div>
    </section>
  `;
}

function renderChunkTable(chunks) {
  return dataTable({
    id: "kb-chunks-table",
    columns: [
      { key: "chunk_index", label: "序号", render: (chunk) => `#${Number(chunk.chunk_index ?? 0) + 1}` },
      {
        key: "content",
        label: "内容",
        html: true,
        render: (chunk) => `<div class="chunk-content-preview"><small>${escapeHtml(chunk.chunk_id || "")}</small>${escapeHtml(chunk.content || "")}</div>`,
      },
      { key: "char_count", label: "字符数", render: (chunk) => chunk.char_count ?? String(chunk.content || "").length },
      {
        key: "actions",
        label: "操作",
        html: true,
        render: (chunk) => `
          <div class="button-cell">
            <button class="button secondary" type="button" data-action="kb-chunk-view-open" data-chunk="${escapeHtml(chunk.chunk_id)}">查看</button>
            <button class="button ghost" type="button" data-action="kb-chunk-delete-dialog-open" data-chunk="${escapeHtml(chunk.chunk_id)}">删除</button>
          </div>
        `,
      },
    ],
    rows: chunks,
    emptyMessage: "暂无 chunks。",
    rowKey: "chunk_id",
    pagination: { page: 1, pageSize: 10, total: chunks.length },
  });
}

function renderChunkViewDialog(chunk) {
  return dialog({
    id: "kb-chunk-view-dialog",
    title: "查看 Chunk",
    open: state.kbDialog === "chunk-view",
    maxWidth: "800px",
    body: `
      <div class="status-list">
        ${statusRow("Index", `#${Number(chunk.chunk_index ?? 0) + 1}`)}
        ${statusRow("Chunk ID", chunk.chunk_id)}
        ${statusRow("Doc ID", chunk.doc_id)}
        ${statusRow("字符数", chunk.char_count ?? String(chunk.content || "").length)}
      </div>
      <div class="chunk-content-view">${escapeHtml(chunk.content || "")}</div>
    `,
    actions: [{ label: "关闭", variant: "ghost", action: "kb-dialog-close" }],
  });
}

function renderKbFormDialog(existing = null) {
  const target = state.kbDialogTarget ? findKb(state.kbDialogTarget) : existing;
  const editing = state.kbDialog === "kb-form" && state.kbFormMode === "edit";
  const kb = editing ? target : null;
  return dialog({
    id: "kb-form-dialog",
    title: editing ? "编辑知识库" : "创建知识库",
    open: state.kbDialog === "kb-form",
    maxWidth: "680px",
    body: `
      <div class="emoji-display" data-action="kb-emoji-dialog-open">${escapeHtml(kb?.emoji || "📚")}</div>
      <div class="form-grid cols-2">
        ${formField({ id: "kb-form-id", label: "KB ID", value: kb?.kb_id || `kb-${Date.now()}`, readonly: editing, required: true })}
        ${formField({ id: "kb-form-name", label: "名称", value: kbName(kb) || "", placeholder: "Project docs", required: true })}
        ${formField({ id: "kb-form-description", label: "描述", type: "textarea", rows: 3, value: kb?.description || "" })}
        ${formField({ id: "kb-form-emoji", label: "Emoji", value: kb?.emoji || "📚" })}
        ${formField({ id: "kb-form-embedding", label: "Embedding Provider ID", value: kb?.embedding_provider_id || state.status?.providers?.default_embedding_provider_id || "embedding", readonly: editing, hint: editing ? "嵌入模型创建后不可在源端 UI 中修改" : "" })}
        ${formField({ id: "kb-form-rerank", label: "Rerank Provider ID", value: kb?.rerank_provider_id || state.status?.providers?.default_rerank_provider_id || "" })}
        ${formField({ id: "kb-form-chunk-size", label: "Chunk size", type: "number", value: kb?.chunk_size || 512 })}
        ${formField({ id: "kb-form-chunk-overlap", label: "Chunk overlap", type: "number", value: kb?.chunk_overlap || 50 })}
      </div>
      <div class="actions">
        ${button({ label: "预检 Provider", action: "kb-preflight", variant: "secondary" })}
      </div>
    `,
    actions: [
      { label: "取消", variant: "ghost", action: "kb-dialog-close" },
      { label: editing ? "保存" : "创建", action: editing ? "kb-update" : "kb-create" },
    ],
  });
}

function renderEmojiDialog() {
  return dialog({
    id: "kb-emoji-dialog",
    title: "选择 Emoji",
    open: state.kbDialog === "emoji",
    maxWidth: "520px",
    body: EMOJI_GROUPS.map(([name, emojis]) => `
      <section class="emoji-category">
        <h3>${escapeHtml(name)}</h3>
        <div class="emoji-grid">
          ${emojis.map((emoji) => `<button type="button" class="emoji-item" data-action="kb-emoji-select" data-emoji="${escapeHtml(emoji)}">${escapeHtml(emoji)}</button>`).join("")}
        </div>
      </section>
    `).join(""),
    actions: [{ label: "关闭", variant: "ghost", action: "kb-form-dialog-return" }],
  });
}

function renderKbDeleteDialog() {
  const kb = findKb(state.kbDialogTarget);
  if (state.kbDialog !== "kb-delete") return "";
  return confirmDialog({
    id: "kb-delete-dialog",
    title: "删除知识库",
    message: `确定删除 ${kbName(kb) || state.kbDialogTarget || "该知识库"}？此操作会删除文档与 chunks。`,
    confirmLabel: "删除",
    cancelLabel: "取消",
    open: true,
  }).replace('data-dialog-value="confirm"', `data-action="kb-delete" data-kb="${escapeHtml(state.kbDialogTarget || "")}" data-dialog-value="confirm"`)
    .replace('data-dialog-value="cancel"', 'data-action="kb-dialog-close" data-dialog-value="cancel"');
}

function renderDocumentDeleteDialog() {
  if (state.kbDialog !== "document-delete") return "";
  const doc = currentDocument() || { doc_id: state.kbDialogTarget, name: state.kbDialogTarget };
  return confirmDialog({
    id: "kb-document-delete-dialog",
    title: "删除文档",
    message: `确定删除 ${docName(doc)}？关联 chunks 会一起删除。`,
    confirmLabel: "删除",
    cancelLabel: "取消",
    open: true,
  }).replace('data-dialog-value="confirm"', `data-action="kb-document-delete" data-doc="${escapeHtml(state.kbDialogTarget || "")}" data-dialog-value="confirm"`)
    .replace('data-dialog-value="cancel"', 'data-action="kb-dialog-close" data-dialog-value="cancel"');
}

function renderChunkDeleteDialog() {
  if (state.kbDialog !== "chunk-delete") return "";
  return confirmDialog({
    id: "kb-chunk-delete-dialog",
    title: "删除 Chunk",
    message: `确定删除 ${state.kbDialogTarget || "该 chunk"}？`,
    confirmLabel: "删除",
    cancelLabel: "取消",
    open: true,
  }).replace('data-dialog-value="confirm"', `data-action="kb-chunk-delete" data-chunk="${escapeHtml(state.kbDialogTarget || "")}" data-dialog-value="confirm"`)
    .replace('data-dialog-value="cancel"', 'data-action="kb-dialog-close" data-dialog-value="cancel"');
}

function renderTavilyDialog() {
  return dialog({
    id: "kb-tavily-dialog",
    title: "配置 Tavily API Key",
    open: state.kbDialog === "tavily",
    maxWidth: "520px",
    body: `
      <p class="ui-dialog-message">为了使用基于网页的知识库功能，需要提供 Tavily API Key。Key 会写入 default config 的 provider_settings。</p>
      ${formField({ id: "kb-tavily-key", label: "Tavily API Key", value: "", placeholder: "tvly-..." })}
    `,
    actions: [
      { label: "取消", variant: "ghost", action: "kb-upload-dialog-return" },
      { label: "保存", action: "kb-tavily-save" },
    ],
  });
}

function renderLegacyKnowledge() {
  const knowledgeBases = state.kb?.knowledge_bases || [];
  return `
    <div class="knowledge-page legacy-kb-page" data-page="legacy-alkaid-knowledge">
      <header class="knowledge-header">
        <div>
          <div class="eyebrow">Legacy Alkaid</div>
          <h2>旧版知识库</h2>
          <p>源端 /alkaid/knowledge-base 依赖插件 API；RS Dashboard 保留入口并提示迁移到新版原生 KB。</p>
        </div>
        <div class="banner-actions">
          <a class="button" href="#/knowledge-base">使用新版知识库</a>
          ${button({ label: "刷新", action: "load-kb", variant: "secondary", icon: "↻" })}
        </div>
      </header>
      <section class="panel">
        <div class="notice-banner">
          <span>${pill("建议迁移", "warn")}</span>
          <span>旧版插件接口 /api/plug/alkaid/kb/* 不在当前 RS runtime 中执行；下方以原生 KB catalog 只读展示。</span>
        </div>
        ${knowledgeBases.length ? renderKnowledgeCards(knowledgeBases) : uiState({ state: "empty", title: "没有可展示的原生 KB", message: "可回到新版页面创建。" })}
      </section>
    </div>
  `;
}

function currentKb() {
  const id = state.routeParams?.kbId || state.kbDetail?.knowledge_base?.kb_id;
  return findKb(id) || state.kbDetail?.knowledge_base || null;
}

function findKb(id) {
  if (!id) return null;
  const current = state.kbDetail?.knowledge_base;
  if (current?.kb_id === id) return current;
  return (state.kb?.knowledge_bases || []).find((kb) => kb.kb_id === id) || null;
}

function currentDocument() {
  const docId = state.routeParams?.docId || state.kbDialogTarget;
  if (!docId) return null;
  return state.kbDocumentDetail || (state.kbDocuments?.documents || []).find((doc) => doc.doc_id === docId) || null;
}

function kbName(kb) {
  return kb?.name || kb?.kb_name || "";
}

function docName(doc) {
  return doc?.name || doc?.doc_name || doc?.doc_id || "";
}

function filterBySearch(items, query, textFor) {
  const normalized = String(query || "").trim().toLowerCase();
  if (!normalized) return items;
  return items.filter((item) => textFor(item).toLowerCase().includes(normalized));
}

function statusRow(label, value) {
  return `<div class="status-item"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value ?? "-")}</strong></div>`;
}

function statusBox(label, value) {
  return `<div class="stat-box"><div class="stat-label">${escapeHtml(label)}</div><div class="stat-number">${escapeHtml(value ?? "-")}</div></div>`;
}

function scoreChip(score) {
  const value = Number(score || 0);
  const kind = value >= 0.8 ? "ok" : value >= 0.4 ? "warn" : "error";
  return chip(`Score ${value.toFixed(4)}`, kind);
}

function fileIcon(type) {
  const lower = String(type || "").toLowerCase();
  if (lower.includes("pdf")) return "▣";
  if (lower.includes("md") || lower.includes("markdown")) return "M";
  if (lower.includes("url")) return "↗";
  if (lower.includes("txt") || lower.includes("text")) return "T";
  return "□";
}

function formatFileSize(bytes) {
  const value = Number(bytes || 0);
  if (!value) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(index === 0 ? 0 : 2)} ${units[index]}`;
}

function safeId(value) {
  const id = String(value || "document")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return id || "document";
}
