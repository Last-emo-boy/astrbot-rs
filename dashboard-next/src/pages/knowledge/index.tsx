import { A, useParams } from "@solidjs/router";
import { createEffect, createMemo, createResource, createSignal, For, onCleanup, Show, type Component } from "solid-js";
import { apiGet, apiPost, apiPostMultipart, buildEventSource } from "@/api/client";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Button, Field, Input } from "@/components/Form";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";
import {
  createChunkPreviews,
  DEFAULT_CHUNK_SIZE,
  uploadFileInChunks,
  type ChunkPreview,
  type ChunkedUploadProgress,
} from "@/features/uploads/chunkedUpload";

interface KbItem {
  kb_id?: string;
  id?: string;
  name?: string;
  documents?: number;
  stats?: { document_count?: number; chunk_count?: number };
  embedding_provider_id?: string;
  embedding_model?: string;
}

interface KbListResponse {
  knowledge_bases?: KbItem[];
}

interface KbDocument {
  doc_id?: string;
  id?: string;
  name?: string;
  status?: string;
  chunk_count?: number;
  chunks?: number;
  file_size?: number;
  size?: number;
  file_type?: string;
  file_path?: string | null;
}

interface KbChunk {
  chunk_id?: string;
  id?: string;
  chunk_index?: number;
  index?: number;
  content?: string;
  text?: string;
  score?: number;
}

interface KbDetailResponse {
  knowledge_base?: KbItem;
  documents?: KbDocument[];
}

interface DocumentDetailResponse {
  document?: KbDocument;
  chunks?: KbChunk[];
}

interface IngestionEvent {
  doc_id?: string;
  task_id?: string;
  status?: string;
  progress?: number;
  message?: string;
}

interface KnowledgeUploadTask {
  task_id?: string;
  status?: string;
  progress?: {
    status?: string;
    file_index?: number;
    file_total?: number;
    file_name?: string;
    stage?: string;
    current?: number;
    total?: number;
  };
  result?: {
    document_ids?: string[];
    chunk_count?: number;
  };
  error?: string;
}

interface KnowledgeUploadTaskResponse {
  task: KnowledgeUploadTask;
}

interface RetrievalHit {
  content?: string;
  highlight?: string;
  score?: number;
}

interface RetrievalResponse {
  results?: RetrievalHit[];
}

interface LegacyResponse<T> {
  data?: T;
}

const formatBytes = (value: number): string => {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
};

const stableId = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "") || `kb-${Date.now()}`;

const kbIdOf = (kb: KbItem): string => kb.kb_id ?? kb.id ?? "";
const docIdOf = (doc: KbDocument): string => doc.doc_id ?? doc.id ?? "";
const chunkTextOf = (chunk: KbChunk): string => chunk.content ?? chunk.text ?? "";
const legacyData = <T,>(response: LegacyResponse<T> | T): T =>
  response && typeof response === "object" && "data" in response ? (response as LegacyResponse<T>).data as T : response as T;

const uploadTaskId = (kbId: string, fileName: string): string =>
  stableId(`upload-${kbId}-${fileName}-${Date.now()}`);

const documentIdFromFileName = (fileName: string): string => `doc-${stableId(fileName)}`;

const uploadEventFromTask = (task: KnowledgeUploadTask): IngestionEvent => {
  const current = task.progress?.current ?? 0;
  const total = Math.max(1, task.progress?.total ?? task.progress?.file_total ?? 1);
  const event: IngestionEvent = {
    status: task.status ?? task.progress?.status ?? "pending",
    progress: current / total,
    message: task.error ?? `${task.progress?.file_name ?? "document"} · ${task.progress?.stage ?? "queued"}`,
  };
  if (task.task_id) event.task_id = task.task_id;
  const docId = task.result?.document_ids?.[0] ?? task.task_id;
  if (docId) event.doc_id = docId;
  return event;
};

const highlight = (text: string, query: string) => {
  const keyword = query.trim();
  if (!keyword) return text;
  const lower = text.toLowerCase();
  const start = lower.indexOf(keyword.toLowerCase());
  if (start < 0) return text;
  const end = start + keyword.length;
  return (
    <>
      {text.slice(0, start)}
      <mark>{text.slice(start, end)}</mark>
      {text.slice(end)}
    </>
  );
};

async function reindexDocument(kbId: string, docId: string): Promise<DocumentDetailResponse> {
  const [document, chunkCatalog] = await Promise.all([
    apiPost<KbDocument>("/api/management/kb/document/get", { doc_id: docId }),
    apiPost<{ chunks?: KbChunk[] }>("/api/management/kb/chunk/list", { doc_id: docId }),
  ]);
  const chunks = chunkCatalog.chunks ?? [];
  const content = chunks.map(chunkTextOf).filter(Boolean).join("\n\n");
  await apiPost("/api/management/kb/ingest", {
    kb_id: kbId,
    doc_id: docId,
    name: document.name ?? docId,
    source_kind: document.file_type ?? "document",
    source_url: document.file_path ?? undefined,
    content,
    clean_html: false,
  });
  const refreshedChunks = await apiPost<{ chunks?: KbChunk[] }>("/api/management/kb/chunk/list", { doc_id: docId });
  return { document, chunks: refreshedChunks.chunks ?? [] };
}

const KbList: Component = () => {
  const [data, { refetch }] = createResource<KbListResponse>(async () =>
    apiGet<KbListResponse>("/api/management/kb/catalog").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<{ name: string; embedding_model: string }>({
    name: "",
    embedding_model: "",
  });

  const create = async () => {
    try {
      const name = draft().name.trim();
      await apiPost("/api/management/kb/create", {
        kb_id: stableId(name),
        name,
        embedding_provider_id: draft().embedding_model.trim(),
      });
      toastSuccess("已创建");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该知识库及全部文档？")) return;
    try {
      await apiPost("/api/management/kb/delete", { kb_id: id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="知识库"
        subtitle="向量化文档检索"
        actions={<Button variant="primary" onClick={() => setOpen(true)}>新建知识库</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.knowledge_bases?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>文档数</th><th>Embedding</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.knowledge_bases ?? []}>
                  {(kb) => (
                    <tr>
                      <td>
                        <A href={`/knowledge-base/${kbIdOf(kb)}`}>{kb.name ?? (kbIdOf(kb) || "-")}</A>
                      </td>
                      <td class="text-mono">{kb.stats?.document_count ?? kb.documents ?? 0}</td>
                      <td class="muted">{kb.embedding_provider_id ?? kb.embedding_model ?? ""}</td>
                      <td>
                        <Button size="sm" variant="danger" onClick={() => kbIdOf(kb) && del(kbIdOf(kb))}>删除</Button>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </Show>
        </Show>
      </Card>
      <Modal
        open={open()}
        title="新建知识库"
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={create}>创建</Button>
          </>
        }
      >
        <Field label="名称">
          <Input value={draft().name} onInput={(e) => setDraft({ ...draft(), name: e.currentTarget.value })} />
        </Field>
        <Field label="Embedding Provider ID">
          <Input
            value={draft().embedding_model}
            onInput={(e) => setDraft({ ...draft(), embedding_model: e.currentTarget.value })}
          />
        </Field>
      </Modal>
    </>
  );
};

const DocumentDetail: Component<{ kbId: string; docId: string }> = (props) => {
  const [data, { refetch }] = createResource(
    () => props.docId,
    async (docId) => {
      const [document, chunkCatalog] = await Promise.all([
        apiPost<KbDocument>("/api/management/kb/document/get", { doc_id: docId }),
        apiPost<{ chunks?: KbChunk[] }>("/api/management/kb/chunk/list", { doc_id: docId }),
      ]);
      return { document, chunks: chunkCatalog.chunks ?? [] };
    }
  );

  const reindex = async () => {
    try {
      await reindexDocument(props.kbId, props.docId);
      toastSuccess("已提交重建索引");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title={data()?.document?.name ?? "文档分片"}
        subtitle={`分片 ${data()?.chunks?.length ?? data()?.document?.chunks ?? 0}`}
        actions={
          <>
            <A href={`/knowledge-base/${props.kbId}`} class="btn">返回</A>
            <Button variant="primary" onClick={reindex}>重建索引</Button>
          </>
        }
      />
      <Card title="Chunk 预览">
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.chunks?.length} fallback={<EmptyState message="暂无分片详情，文档表仍会显示分片数量。" />}>
            <div class="stack">
              <For each={data()?.chunks ?? []}>
                {(chunk, index) => (
                  <div class="chunk-preview">
                    <div class="toolbar">
                      <span class="badge">#{chunk.chunk_index ?? chunk.index ?? index()}</span>
                      <div class="toolbar__spacer" />
                      <Show when={chunk.score !== undefined}>
                        <span class="text-mono muted">{chunk.score?.toFixed(3)}</span>
                      </Show>
                    </div>
                    <div>{chunkTextOf(chunk)}</div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Card>
    </>
  );
};

const KbDetail: Component<{ kbId: string }> = (props) => {
  const [data, { refetch }] = createResource(
    () => props.kbId,
    async (id): Promise<KbDetailResponse> => {
      const [kb, docs] = await Promise.all([
        apiPost<{ knowledge_base?: KbItem }>("/api/management/kb/get", { kb_id: id }),
        apiPost<{ documents?: KbDocument[] }>("/api/management/kb/document/list", { kb_id: id }),
      ]);
      const response: KbDetailResponse = { documents: docs.documents ?? [] };
      if (kb.knowledge_base) response.knowledge_base = kb.knowledge_base;
      return response;
    }
  );
  const [file, setFile] = createSignal<File | null>(null);
  const [chunkPreviews, setChunkPreviews] = createSignal<ChunkPreview[]>([]);
  const [uploadProgress, setUploadProgress] = createSignal<ChunkedUploadProgress | null>(null);
  const [query, setQuery] = createSignal("");
  const [hits, setHits] = createSignal<Array<{ text?: string; score?: number }>>([]);
  const [ingestionEvents, setIngestionEvents] = createSignal<IngestionEvent[]>([]);
  const [activeUploadTaskId, setActiveUploadTaskId] = createSignal<string | null>(null);

  const ingestionByDoc = createMemo(() => {
    const map = new Map<string, IngestionEvent>();
    for (const event of ingestionEvents()) {
      if (event.doc_id) map.set(event.doc_id, event);
    }
    return map;
  });

  createEffect(() => {
    const current = file();
    setUploadProgress(null);
    setChunkPreviews(current ? createChunkPreviews(current, DEFAULT_CHUNK_SIZE) : []);
  });

  createEffect(() => {
    const timer = window.setInterval(() => void refetch(), 5000);
    onCleanup(() => window.clearInterval(timer));
  });

  createEffect(() => {
    const taskId = activeUploadTaskId();
    if (!taskId) return;
    const stream = buildEventSource(`/api/management/kb/upload/progress/${encodeURIComponent(taskId)}/stream`);

    stream.addEventListener("upload", (event) => {
      const payload = JSON.parse((event as MessageEvent).data) as KnowledgeUploadTask;
      const next = uploadEventFromTask(payload);
      setIngestionEvents((events) => [next, ...events.filter((item) => item.task_id !== next.task_id)].slice(0, 12));
      if (payload.status === "completed" || payload.status === "failed" || payload.status === "cancelled") {
        stream.close();
        setActiveUploadTaskId(null);
        void refetch();
      }
    });
    stream.onerror = () => {
      stream.close();
      setActiveUploadTaskId(null);
    };

    onCleanup(() => stream.close());
  });

  const upload = async () => {
    const f = file();
    if (!f) return;
    const taskId = uploadTaskId(props.kbId, f.name);
    try {
      await apiPost<KnowledgeUploadTaskResponse>("/api/management/kb/upload/plan", {
        task_id: taskId,
        kb_id: props.kbId,
        kind: "upload",
        file_total: 1,
      });
      setActiveUploadTaskId(taskId);
      await uploadFileInChunks(f, async (chunk) => {
        await apiPost<KnowledgeUploadTaskResponse>("/api/management/kb/upload/progress", {
          task_id: taskId,
          file_index: 0,
          file_total: 1,
          file_name: f.name,
          stage: chunk.index + 1 === chunk.total ? "embedding" : "chunking",
          current: chunk.index + 1,
          total: chunk.total,
        });
      }, { onProgress: setUploadProgress });
      const form = new FormData();
      form.append("kb_id", props.kbId);
      form.append("file", f, f.name);
      legacyData<{ task_id?: string; file_count?: number }>(
        await apiPostMultipart<LegacyResponse<{ task_id?: string; file_count?: number }>>("/api/kb/document/upload", form)
      );
      await apiPost<KnowledgeUploadTaskResponse>("/api/management/kb/upload/complete", {
        task_id: taskId,
        document_ids: [documentIdFromFileName(f.name)],
        chunk_count: chunkPreviews().length,
      });
      setIngestionEvents((events) => [
        {
          task_id: taskId,
          doc_id: documentIdFromFileName(f.name),
          status: "completed",
          progress: 1,
          message: `${f.name} 上传完成，文档解析状态会继续刷新。`,
        },
        ...events.filter((event) => event.task_id !== taskId),
      ].slice(0, 12));
      toastSuccess("已开始解析");
      setFile(null);
      await refetch();
    } catch (err) {
      await apiPost("/api/management/kb/upload/fail", {
        task_id: taskId,
        error: err instanceof Error ? err.message : String(err),
      }).catch(() => undefined);
      setActiveUploadTaskId(null);
      toastError(err);
    }
  };

  const search = async () => {
    try {
      const res = await apiPost<RetrievalResponse>(
        "/api/management/kb/retrieve",
        { kb_ids: [props.kbId], query: query() }
      );
      setHits((res.results ?? []).map((hit) => ({
        text: hit.highlight ?? hit.content ?? "",
        ...(hit.score !== undefined ? { score: hit.score } : {}),
      })));
    } catch (err) {
      toastError(err);
    }
  };

  const reindexAll = async () => {
    try {
      for (const doc of data()?.documents ?? []) {
        const docId = docIdOf(doc);
        if (docId) await reindexDocument(props.kbId, docId);
      }
      toastSuccess("已提交知识库重建索引");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const removeDoc = async (docId: string) => {
    if (!confirm("删除该文档？")) return;
    try {
      await apiPost("/api/management/kb/document/delete", { kb_id: props.kbId, doc_id: docId });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title={data()?.knowledge_base?.name ?? "知识库"}
        subtitle={`文档 ${data()?.documents?.length ?? 0}`}
        actions={
          <>
            <A href="/knowledge-base" class="btn">返回</A>
            <Button onClick={reindexAll}>重建索引</Button>
          </>
        }
      />
      <Card title="上传文档">
        <div class="stack">
          <div class="row">
            <Input type="file" onChange={(e) => setFile(e.currentTarget.files?.[0] ?? null)} />
            <Button variant="primary" disabled={!file()} onClick={upload}>分片上传</Button>
          </div>
          <Show when={file()}>
            <div class="chunk-strip">
              <For each={chunkPreviews().slice(0, 8)}>
                {(chunk) => (
                  <span class="chunk-strip__item">
                    #{chunk.index + 1} {formatBytes(chunk.size)}
                  </span>
                )}
              </For>
              <Show when={chunkPreviews().length > 8}>
                <span class="muted">+{chunkPreviews().length - 8}</span>
              </Show>
            </div>
          </Show>
          <Show when={uploadProgress()}>
            {(progress) => (
              <div class="progress-line">
                <div
                  class="progress-line__bar"
                  style={{ width: `${Math.round((progress().uploadedBytes / Math.max(1, progress().totalBytes)) * 100)}%` }}
                />
                <span>
                  {progress().uploadedChunks}/{progress().totalChunks} chunks · {formatBytes(progress().uploadedBytes)}
                </span>
              </div>
            )}
          </Show>
        </div>
      </Card>
      <Card title="检索">
        <div class="row">
          <Input value={query()} onInput={(e) => setQuery(e.currentTarget.value)} placeholder="检索关键词…" />
          <Button onClick={search}>检索</Button>
        </div>
        <Show when={hits().length}>
          <ul class="search-hit-list">
            <For each={hits()}>
              {(h) => (
                <li>
                  <span class="badge">{(h.score ?? 0).toFixed(3)}</span> {highlight(h.text ?? "", query())}
                </li>
              )}
            </For>
          </ul>
        </Show>
      </Card>
      <Card title="Ingestion 状态">
        <Show when={ingestionEvents().length} fallback={<div class="muted">等待解析事件；页面会每 5 秒刷新文档状态。</div>}>
          <div class="stack">
            <For each={ingestionEvents()}>
              {(event) => (
                <div class="status-row">
                  <span class="badge badge--accent">{event.status ?? "pending"}</span>
                  <span class="text-mono">{event.doc_id ?? "-"}</span>
                  <span class="muted">{event.message ?? ""}</span>
                  <Show when={event.progress !== undefined}>
                    <span class="text-mono">{Math.round((event.progress ?? 0) * 100)}%</span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Card>
      <Card title="文档">
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.documents?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>状态</th><th>分片</th><th>大小</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.documents ?? []}>
                  {(d) => {
                    const documentId = () => docIdOf(d);
                    const live = () => (documentId() ? ingestionByDoc().get(documentId()) : undefined);
                    return (
                      <tr>
                        <td>{documentId() ? <A href={`/knowledge-base/${props.kbId}/document/${documentId()}`}>{d.name ?? documentId()}</A> : d.name ?? "-"}</td>
                        <td>
                          <span class="badge">{live()?.status ?? d.status ?? "-"}</span>
                        </td>
                        <td class="text-mono">{d.chunk_count ?? d.chunks ?? 0}</td>
                        <td class="text-mono">{formatBytes(d.file_size ?? d.size ?? 0)}</td>
                        <td class="row">
                          <Button
                            size="sm"
                            onClick={async () => {
                              if (!documentId()) return;
                              await reindexDocument(props.kbId, documentId());
                              toastSuccess("已提交重建索引");
                              await refetch();
                            }}
                          >
                            重建
                          </Button>
                          <Button size="sm" variant="danger" onClick={() => documentId() && removeDoc(documentId())}>删除</Button>
                        </td>
                      </tr>
                    );
                  }}
                </For>
              </tbody>
            </table>
          </Show>
        </Show>
      </Card>
    </>
  );
};

const KnowledgePage: Component = () => {
  const params = useParams();
  return (
    <Show when={params.kbId} fallback={<KbList />}>
      <Show when={params.docId} fallback={<KbDetail kbId={params.kbId!} />}>
        <DocumentDetail kbId={params.kbId!} docId={params.docId!} />
      </Show>
    </Show>
  );
};

export default KnowledgePage;
