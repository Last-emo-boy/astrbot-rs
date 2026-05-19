import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface ToolItem {
  id?: string;
  name?: string;
  source?: string;
  description?: string;
  enabled?: boolean;
}

interface ToolsResponse {
  tools?: ToolItem[];
}

const ToolsPage: Component = () => {
  const [data, { refetch }] = createResource<ToolsResponse>(async () =>
    apiGet<ToolsResponse>("/api/management/tools").catch(() => ({}))
  );
  const [selected, setSelected] = createSignal<ToolItem | null>(null);

  const toggle = async (id: string, enabled: boolean) => {
    try {
      await apiPost("/api/management/tools/toggle", { id, enabled });
      toastSuccess(enabled ? "已启用" : "已禁用");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="工具"
        subtitle="函数调用工具注册表"
        actions={<Button onClick={() => refetch()}>刷新</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.tools?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr><th>名称</th><th>来源</th><th>状态</th><th>操作</th></tr>
              </thead>
              <tbody>
                <For each={data()?.tools ?? []}>
                  {(t) => (
                    <tr>
                      <td>{t.name ?? t.id ?? "-"}</td>
                      <td><span class="badge">{t.source ?? "?"}</span></td>
                      <td>{t.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" variant="ghost" onClick={() => setSelected(t)}>详情</Button>
                        <Button size="sm" onClick={() => t.id && toggle(t.id, !t.enabled)}>
                          {t.enabled ? "停用" : "启用"}
                        </Button>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </Show>
        </Show>
      </Card>
      <Modal open={selected() !== null} title="工具详情" onClose={() => setSelected(null)}>
        <pre class="code-block">{JSON.stringify(selected(), null, 2)}</pre>
      </Modal>
    </>
  );
};

export default ToolsPage;
