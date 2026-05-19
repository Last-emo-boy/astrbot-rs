import { api } from "../api.js";
import { $, showToast } from "../dom.js";
import {
  loadKnowledge,
  loadKnowledgeChunks,
  loadKnowledgeDetail,
  loadKnowledgeDocument,
  loadKnowledgeDocuments,
  loadKnowledgeUploadTask,
} from "../loaders.js";
import { state } from "../state.js";
import { optionalText, splitCsv } from "./forms.js";

export async function handleKnowledgeActions({ action, target }) {
  if (action === "kb-create-dialog-open") {
    state.kbDialog = "kb-form";
    state.kbDialogReturn = "";
    state.kbDialogTarget = target.dataset.kb || state.routeParams?.kbId || "";
    state.kbFormMode = target.dataset.mode === "edit" || state.kbDialogTarget ? "edit" : "create";
  }
  if (action === "kb-delete-dialog-open") {
    state.kbDialog = "kb-delete";
    state.kbDialogTarget = target.dataset.kb || state.routeParams?.kbId || "";
  }
  if (action === "kb-document-delete-dialog-open") {
    state.kbDialog = "document-delete";
    state.kbDialogTarget = target.dataset.doc || "";
  }
  if (action === "kb-chunk-delete-dialog-open") {
    state.kbDialog = "chunk-delete";
    state.kbDialogTarget = target.dataset.chunk || "";
  }
  if (action === "kb-upload-dialog-open") {
    state.kbDialog = "upload";
    state.kbDialogReturn = "";
    state.kbUploadMode = state.kbUploadMode || "file";
  }
  if (action === "kb-upload-mode") {
    state.kbUploadMode = target.dataset.mode || "file";
    state.kbDialog = "upload";
  }
  if (action === "kb-tavily-dialog-open") {
    state.kbDialogReturn = "upload";
    state.kbDialog = "tavily";
  }
  if (action === "kb-dialog-close") {
    state.kbDialog = "";
    state.kbDialogTarget = "";
    state.kbDialogReturn = "";
  }
  if (action === "kb-form-dialog-return") {
    state.kbDialog = "kb-form";
  }
  if (action === "kb-upload-dialog-return") {
    state.kbDialog = state.kbDialogReturn || "upload";
    state.kbDialogReturn = "";
  }
  if (action === "kb-emoji-dialog-open") {
    state.kbDialogReturn = "kb-form";
    state.kbDialog = "emoji";
  }
  if (action === "kb-emoji-select") {
    if (kbField("emoji")) kbField("emoji").value = target.dataset.emoji || "📚";
    state.kbDialog = state.kbDialogReturn || "kb-form";
    state.kbDialogReturn = "";
  }
  if (action === "kb-tab") {
    state.kbActiveTab = target.dataset.tab || "overview";
    state.kbDialog = "";
  }
  if (action === "kb-open") {
    const kbId = target.dataset.kb;
    if (kbId) {
      state.kbActiveTab = "overview";
      state.kbDialog = "";
      state.routeParams = { kbId };
      state.routePath = `/knowledge-base/${kbId}`;
      state.routeSourcePath = state.routePath;
      await Promise.all([loadKnowledgeDetail(kbId), loadKnowledgeDocuments(kbId)]);
    }
  }
  if (action === "kb-document-open") {
    const kbId = target.dataset.kb || state.routeParams?.kbId;
    const docId = target.dataset.doc;
    if (kbId && docId) {
      state.routeParams = { kbId, docId };
      state.routePath = `/knowledge-base/${kbId}/document/${docId}`;
      state.routeSourcePath = state.routePath;
      await Promise.all([loadKnowledgeDocument(docId), loadKnowledgeChunks(docId)]);
    }
  }
  if (action === "kb-detail-refresh") {
    const kbId = target.dataset.kb || state.routeParams?.kbId;
    await Promise.all([loadKnowledge(), loadKnowledgeDetail(kbId), loadKnowledgeDocuments(kbId)]);
    showToast("知识库详情已刷新");
  }
  if (action === "kb-chunk-view-open") {
    state.kbSelectedChunkId = target.dataset.chunk || "";
    state.kbDialog = "chunk-view";
  }
  if (action === "kb-document-search") {
    state.kbDocumentSearch = $("#kb-document-search")?.value.trim() || "";
  }
  if (action === "kb-legacy-open") {
    if (globalThis.window?.location) {
      globalThis.window.location.hash = "#/alkaid/knowledge-base";
    }
    state.routeSourcePath = "/alkaid/knowledge-base";
  }

  if (action === "kb-preflight") {
    const report = await api("/api/management/kb/preflight", {
      method: "POST",
      body: JSON.stringify({
        embedding_provider_id: kbField("embedding").value.trim(),
        expected_embedding_dimension: Number($("#kb-expected-dimension")?.value || 0) || null,
        rerank_provider_id: optionalKbText("rerank"),
      }),
    });
    state.operation = report;
    const embeddingOk = report.report.embedding.available && report.report.embedding.dimension_matches;
    const rerankOk = !report.report.rerank || report.report.rerank.smoke_test_passed;
    showToast(`预检完成：${embeddingOk && rerankOk ? "可用" : "不可用"}`);
  }
  if (action === "kb-create") {
    const kbId = kbField("id").value.trim();
    const result = await api("/api/management/kb/create", {
      method: "POST",
      body: JSON.stringify({
        kb_id: kbId,
        name: kbField("name").value.trim(),
        description: optionalKbText("description"),
        emoji: optionalKbText("emoji"),
        embedding_provider_id: kbField("embedding").value.trim(),
        rerank_provider_id: optionalKbText("rerank"),
        chunk_size: Number(kbField("chunk-size").value) || null,
        chunk_overlap: Number(kbField("chunk-overlap").value) || null,
      }),
    });
    state.operation = result;
    state.kbDetail = result;
    state.kbDialog = "";
    await loadKnowledge();
    showToast("知识库已创建");
  }
  if (action === "kb-update" || action === "kb-settings-save") {
    const kbId = target.dataset.kb || kbField("id")?.value.trim() || state.routeParams?.kbId;
    const result = await api("/api/management/kb/update", {
      method: "POST",
      body: JSON.stringify({
        kb_id: kbId,
        name: optionalKbText("name"),
        description: optionalKbText("description"),
        emoji: optionalKbText("emoji"),
        rerank_provider_id: optionalKbText("rerank"),
        chunk_size: Number(kbField("chunk-size")?.value || 0) || null,
        chunk_overlap: Number(kbField("chunk-overlap")?.value || 0) || null,
      }),
    });
    state.operation = result;
    state.kbDetail = result;
    state.kbDialog = "";
    await Promise.all([loadKnowledge(), loadKnowledgeDetail(kbId)]);
    showToast(action === "kb-settings-save" ? "知识库设置已保存" : "知识库已更新");
  }
  if (action === "kb-get") {
    const kbId = target.dataset.kb || kbField("id")?.value.trim() || state.routeParams?.kbId;
    await loadKnowledgeDetail(kbId);
    state.operation = state.kbDetail;
    showToast("知识库详情已读取");
  }
  if (action === "kb-delete") {
    const kbId = target.dataset.kb || state.kbDialogTarget || kbField("id")?.value.trim();
    state.operation = await api("/api/management/kb/delete", {
      method: "POST",
      body: JSON.stringify({ kb_id: kbId }),
    });
    state.kbDetail = null;
    state.kbDocuments = null;
    state.kbChunks = null;
    state.kbDialog = "";
    state.kbDialogTarget = "";
    await loadKnowledge();
    showToast("知识库已删除");
  }
  if (action === "kb-documents") {
    const kbId = target.dataset.kb || state.routeParams?.kbId || kbField("id")?.value.trim();
    await loadKnowledgeDocuments(kbId);
    state.kbActiveTab = "documents";
    showToast("知识库文档已读取");
  }
  if (action === "kb-document-delete") {
    const docId = target.dataset.doc || state.kbDialogTarget;
    state.operation = await api("/api/management/kb/document/delete", {
      method: "POST",
      body: JSON.stringify({ doc_id: docId }),
    });
    state.kbDialog = "";
    state.kbDialogTarget = "";
    if (state.routeParams?.kbId) await loadKnowledgeDocuments(state.routeParams.kbId);
    showToast("文档已删除");
  }
  if (action === "kb-chunks") {
    await loadKnowledgeChunks(target.dataset.doc || state.routeParams?.docId);
    showToast("Chunks 已读取");
  }
  if (action === "kb-chunk-delete") {
    const chunkId = target.dataset.chunk || state.kbDialogTarget;
    state.operation = await api("/api/management/kb/chunk/delete", {
      method: "POST",
      body: JSON.stringify({ chunk_id: chunkId }),
    });
    state.kbDialog = "";
    state.kbDialogTarget = "";
    if (state.routeParams?.docId) await loadKnowledgeChunks(state.routeParams.docId);
    showToast("Chunk 已删除");
  }
  if (action === "kb-retrieve") {
    const kbIds = target.dataset.kb ? [target.dataset.kb] : splitCsv($("#kb-retrieve-kb-ids")?.value || "");
    state.kbQuery = $("#kb-query")?.value.trim() || "";
    state.kbTopK = Number($("#kb-retrieve-top-k")?.value || 5);
    state.kbDebugMode = Boolean($("#kb-retrieve-debug")?.checked);
    state.kbRetrieval = await api("/api/management/kb/retrieve", {
      method: "POST",
      body: JSON.stringify({
        query: state.kbQuery,
        kb_ids: kbIds,
        top_k: state.kbTopK,
      }),
    });
    state.operation = state.kbRetrieval;
    state.kbActiveTab = "retrieval";
    showToast(`检索完成：${state.kbRetrieval.results.length} 条`);
  }
  if (action === "kb-ingest") {
    const kbId = $("#kb-id")?.value.trim() || state.routeParams?.kbId;
    if (!kbId) throw new Error("缺少 KB ID");
    const taskId = $("#kb-task-id")?.value.trim();
    if (taskId && !state.kbUploadTask?.task) {
      await planUploadTask(kbId, taskId);
    }
    const ingestResult = await api("/api/management/kb/ingest", {
      method: "POST",
      body: JSON.stringify({
        kb_id: kbId,
        doc_id: optionalText("#kb-ingest-doc-id"),
        name: $("#kb-ingest-name").value.trim(),
        source_kind: $("#kb-ingest-source-kind").value,
        source_url: optionalText("#kb-ingest-source-url"),
        content: $("#kb-ingest-content")?.value || "",
        clean_html: Boolean($("#kb-ingest-clean-html")?.checked),
      }),
    });
    state.operation = ingestResult;
    if (taskId) {
      await completeUploadTask(taskId, ingestResult.document?.doc_id, ingestResult.chunks?.length || 0);
    }
    state.kbDialog = "";
    await Promise.all([
      loadKnowledge(),
      loadKnowledgeDetail(kbId),
      loadKnowledgeDocuments(kbId),
      ingestResult.document?.doc_id ? loadKnowledgeChunks(ingestResult.document.doc_id) : Promise.resolve(),
    ]);
    state.kbActiveTab = "documents";
    showToast(`Ingest 完成：${ingestResult.chunks.length} 个 chunks`);
  }
  if (action === "kb-tavily-save") {
    const apiKey = $("#kb-tavily-key")?.value.trim();
    if (!apiKey) throw new Error("Tavily API Key 不能为空");
    state.operation = {
      kind: "kb_tavily_key_capture",
      status: "not_applied",
      message: "RS RuntimeConfig 当前没有 provider_settings.websearch_tavily_key 字段；请在兼容配置落地后保存。",
      key_prefix: `${apiKey.slice(0, 6)}...`,
    };
    state.kbDialog = state.kbDialogReturn || "upload";
    state.kbDialogReturn = "";
    showToast("Tavily Key 已捕获，但当前 runtime config 暂不支持写入", "warn");
  }
  if (action === "kb-upload-plan") {
    await planUploadTask($("#kb-id")?.value.trim() || state.routeParams?.kbId, $("#kb-task-id").value.trim());
    showToast("上传任务已创建");
  }
  if (action === "kb-upload-progress") {
    state.kbUploadTask = await api("/api/management/kb/upload/progress", {
      method: "POST",
      body: JSON.stringify({
        task_id: $("#kb-task-id").value.trim(),
        file_index: 0,
        file_total: Number($("#kb-file-total").value) || 1,
        file_name: optionalText("#kb-file-name"),
        stage: $("#kb-upload-stage").value,
        current: Number($("#kb-progress-current").value) || 0,
        total: Number($("#kb-progress-total").value) || 1,
      }),
    });
    state.operation = state.kbUploadTask;
    showToast("上传进度已更新");
  }
  if (action === "kb-upload-complete") {
    await completeUploadTask($("#kb-task-id").value.trim(), $("#kb-ingest-doc-id")?.value.trim() || "doc-dashboard", Number($("#kb-progress-total")?.value || 1) || 1);
    await loadKnowledge();
    showToast("上传任务已完成");
  }
  if (action === "kb-upload-fail") {
    state.kbUploadTask = await api("/api/management/kb/upload/fail", {
      method: "POST",
      body: JSON.stringify({
        task_id: $("#kb-task-id").value.trim(),
        error: "dashboard marked failed",
      }),
    });
    state.operation = state.kbUploadTask;
    showToast("上传任务已标记失败");
  }
  if (action === "kb-upload-poll") {
    await loadKnowledgeUploadTask($("#kb-task-id")?.value.trim() || state.kbUploadTask?.task?.task_id || "");
    state.operation = state.kbUploadTask;
    showToast("上传任务已刷新");
  }
  if (action === "load-kb") {
    await loadKnowledge();
    showToast("知识库已刷新");
  }
}

async function planUploadTask(kbId, taskId) {
  state.kbUploadTask = await api("/api/management/kb/upload/plan", {
    method: "POST",
    body: JSON.stringify({
      task_id: taskId,
      kb_id: kbId,
      kind: state.kbUploadMode === "url" ? "url" : "upload",
      file_total: Number($("#kb-file-total")?.value || 1) || 1,
    }),
  });
  state.operation = state.kbUploadTask;
}

async function completeUploadTask(taskId, docId, chunkCount) {
  state.kbUploadTask = await api("/api/management/kb/upload/complete", {
    method: "POST",
    body: JSON.stringify({
      task_id: taskId,
      document_ids: docId ? [docId] : [],
      chunk_count: chunkCount,
    }),
  });
  state.operation = state.kbUploadTask;
}

function kbField(name) {
  if (state.kbDialog === "kb-form" || state.kbDialogReturn === "kb-form") {
    return $(`#kb-form-${name}`) || $(`#kb-${name}`);
  }
  return $(`#kb-${name}`) || $(`#kb-form-${name}`);
}

function optionalKbText(name) {
  const value = kbField(name)?.value.trim();
  return value ? value : null;
}
