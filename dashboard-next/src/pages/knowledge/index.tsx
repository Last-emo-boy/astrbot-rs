import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { useParams, A } from "@solidjs/router";
import { apiGet, apiPost, apiPostMultipart } from "@/api/client";
import { Button, Field, Input } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface KbItem {
  id?: string;
  name?: string;
  documents?: number;
  embedding_model?: string;
}

interface KbListResponse {
  knowledge_bases?: KbItem[];
}

interface KbDocument {
  id?: string;
  name?: string;
  status?: string;
  chunks?: number;
  size?: number;
}

interface KbDetailResponse {
  knowledge_base?: KbItem;
  documents?: KbDocument[];
}

const KbList: Component = () => {
  const [data, { refetch }] = createResource<KbListResponse>(async () =>
    apiGet<KbListResponse>("/api/management/kb").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<{ name: string; embedding_model: string }>({
    name: "",
    embedding_model: "",
  });

  const create = async () => {
    try {
      await apiPost("/api/management/kb/create", draft());
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
      await apiPost("/api/management/kb/delete", { id });
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
                        <A href={`/knowledge-base/${kb.id ?? ""}`}>{kb.name ?? kb.id ?? "-"}</A>
                      </td>
                      <td class="text-mono">{kb.documents ?? 0}</td>
                      <td class="muted">{kb.embedding_model ?? ""}</td>
                      <td>
                        <Button size="sm" variant="danger" onClick={() => kb.id && del(kb.id)}>删除</Button>
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

const KbDetail: Component<{ kbId: string }> = (props) => {
  const [data, { refetch }] = createResource<KbDetailResponse, string>(
    () => props.kbId,
    async (id) => apiGet<KbDetailResponse>(`/api/management/kb/${id}`).catch(() => ({} as KbDetailResponse))
  );
  const [file, setFile] = createSignal<File | null>(null);
  const [query, setQuery] = createSignal("");
  const [hits, setHits] = createSignal<Array<{ text?: string; score?: number }>>([]);

  const upload = async () => {
    const f = file();
    if (!f) return;
    const form = new FormData();
    form.append("file", f);
    form.append("kb_id", props.kbId);
    try {
      await apiPostMultipart("/api/management/kb/upload", form);
      toastSuccess("已开始解析");
      setFile(null);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const search = async () => {
    try {
      const res = await apiPost<{ hits?: Array<{ text?: string; score?: number }> }>(
        "/api/management/kb/search",
        { kb_id: props.kbId, query: query() }
      );
      setHits(res?.hits ?? []);
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
          <A href="/knowledge-base" class="btn">
            返回
          </A>
        }
      />
      <Card title="上传文档">
        <div class="row">
          <Input type="file" onChange={(e) => setFile(e.currentTarget.files?.[0] ?? null)} />
          <Button variant="primary" disabled={!file()} onClick={upload}>
            上传
          </Button>
        </div>
      </Card>
      <Card title="检索">
        <div class="row">
          <Input
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            placeholder="检索关键词…"
          />
          <Button onClick={search}>检索</Button>
        </div>
        <Show when={hits().length}>
          <ul>
            <For each={hits()}>
              {(h) => (
                <li>
                  <span class="badge">{(h.score ?? 0).toFixed(3)}</span> {h.text}
                </li>
              )}
            </For>
          </ul>
        </Show>
      </Card>
      <Card title="文档">
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.documents?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>状态</th><th>分片</th><th>大小</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.documents ?? []}>
                  {(d) => (
                    <tr>
                      <td>{d.name ?? d.id}</td>
                      <td>{d.status ?? "-"}</td>
                      <td class="text-mono">{d.chunks ?? 0}</td>
                      <td class="text-mono">{d.size ?? 0}</td>
                      <td>
                        <Button size="sm" variant="danger" onClick={() => d.id && removeDoc(d.id)}>删除</Button>
                      </td>
                    </tr>
                  )}
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
      <KbDetail kbId={params.kbId!} />
    </Show>
  );
};

export default KnowledgePage;
