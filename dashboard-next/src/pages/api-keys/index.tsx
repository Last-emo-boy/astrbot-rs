import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Input } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface ApiKey {
  id?: string;
  label?: string;
  scope?: string;
  created_at?: string;
  last_used?: string;
  preview?: string;
}

interface ApiKeysResponse {
  keys?: ApiKey[];
}

const ApiKeysPage: Component = () => {
  const [data, { refetch }] = createResource<ApiKeysResponse>(async () =>
    apiGet<ApiKeysResponse>("/api/management/api-keys").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [label, setLabel] = createSignal("");
  const [scope, setScope] = createSignal("chat");
  const [created, setCreated] = createSignal<string | null>(null);

  const create = async () => {
    try {
      const res = await apiPost<{ token?: string }>("/api/management/api-keys/create", {
        label: label(),
        scope: scope(),
      });
      setCreated(res?.token ?? null);
      setLabel("");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const revoke = async (id: string) => {
    if (!confirm("撤销该 API Key？")) return;
    try {
      await apiPost("/api/management/api-keys/revoke", { id });
      toastSuccess("已撤销");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="API Keys"
        subtitle="OpenAPI 调用凭证"
        actions={<Button variant="primary" onClick={() => setOpen(true)}>生成 Key</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.keys?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr><th>标签</th><th>Scope</th><th>预览</th><th>创建时间</th><th>最近使用</th><th>操作</th></tr>
              </thead>
              <tbody>
                <For each={data()?.keys ?? []}>
                  {(k) => (
                    <tr>
                      <td>{k.label ?? "-"}</td>
                      <td><span class="badge">{k.scope ?? "-"}</span></td>
                      <td class="text-mono">{k.preview ?? "-"}</td>
                      <td class="text-mono">{k.created_at ?? ""}</td>
                      <td class="text-mono">{k.last_used ?? "-"}</td>
                      <td>
                        <Button size="sm" variant="danger" onClick={() => k.id && revoke(k.id)}>撤销</Button>
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
        title="生成 API Key"
        onClose={() => {
          setOpen(false);
          setCreated(null);
        }}
        actions={
          <>
            <Button onClick={() => { setOpen(false); setCreated(null); }}>关闭</Button>
            <Show when={!created()}>
              <Button variant="primary" onClick={create}>生成</Button>
            </Show>
          </>
        }
      >
        <Show when={!created()} fallback={
          <Field label="新生成的 Key（仅显示一次）">
            <Input value={created() ?? ""} readOnly />
          </Field>
        }>
          <Field label="标签">
            <Input value={label()} onInput={(e) => setLabel(e.currentTarget.value)} />
          </Field>
          <Field label="Scope">
            <Input value={scope()} onInput={(e) => setScope(e.currentTarget.value)} />
          </Field>
        </Show>
      </Modal>
    </>
  );
};

export default ApiKeysPage;
