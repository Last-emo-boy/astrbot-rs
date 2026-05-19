import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface ProviderItem {
  id?: string;
  name?: string;
  kind?: string;
  enabled?: boolean;
  config?: Record<string, unknown>;
}

interface ProvidersResponse {
  providers?: ProviderItem[];
  catalog?: unknown;
}

const ProvidersPage: Component = () => {
  const [data, { refetch }] = createResource<ProvidersResponse>(async () =>
    apiGet<ProvidersResponse>("/api/management/providers")
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<string>("{}");
  const [editing, setEditing] = createSignal<ProviderItem | null>(null);

  const openEdit = (item: ProviderItem | null) => {
    setEditing(item);
    setDraft(JSON.stringify(item ?? { name: "", kind: "openai", config: {} }, null, 2));
    setOpen(true);
  };

  const save = async () => {
    try {
      const payload = JSON.parse(draft());
      await apiPost("/api/management/providers/upsert", payload);
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("确认删除该 Provider？")) return;
    try {
      await apiPost("/api/management/providers/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const check = async (id: string) => {
    try {
      await apiPost("/api/management/providers/check", { id });
      toastSuccess("健康检查通过");
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="模型 Provider"
        subtitle="LLM / Embedding / Rerank 提供方"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.providers?.length} fallback={<EmptyState message="尚未配置 Provider" />}>
            <table class="table">
              <thead>
                <tr>
                  <th>名称</th>
                  <th>类型</th>
                  <th>状态</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                <For each={data()?.providers ?? []}>
                  {(p) => (
                    <tr>
                      <td>{p.name ?? p.id ?? "-"}</td>
                      <td><span class="badge">{p.kind ?? "?"}</span></td>
                      <td>{p.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => openEdit(p)}>编辑</Button>
                        <Button size="sm" variant="ghost" onClick={() => p.id && check(p.id)}>测试</Button>
                        <Button size="sm" variant="danger" onClick={() => p.id && del(p.id)}>删除</Button>
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
        title={editing() ? "编辑 Provider" : "新增 Provider"}
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={save}>保存</Button>
          </>
        }
      >
        <Field label="JSON 配置" hint="包含 name, kind, enabled, config 字段">
          <Textarea rows={20} value={draft()} onInput={(e) => setDraft(e.currentTarget.value)} />
        </Field>
      </Modal>
    </>
  );
};

export default ProvidersPage;
