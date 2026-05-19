import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface PlatformItem {
  id?: string;
  name?: string;
  kind?: string;
  enabled?: boolean;
  status?: string;
}

interface PlatformsResponse {
  platforms?: PlatformItem[];
}

const PlatformsPage: Component = () => {
  const [data, { refetch }] = createResource<PlatformsResponse>(async () =>
    apiGet<PlatformsResponse>("/api/management/platforms")
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<string>("{}");
  const [editing, setEditing] = createSignal<PlatformItem | null>(null);

  const openEdit = (item: PlatformItem | null) => {
    setEditing(item);
    setDraft(JSON.stringify(item ?? { name: "", kind: "qq_official", config: {} }, null, 2));
    setOpen(true);
  };

  const save = async () => {
    try {
      const payload = JSON.parse(draft());
      await apiPost("/api/management/platforms/upsert", payload);
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("确认删除该平台？")) return;
    try {
      await apiPost("/api/management/platforms/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const toggle = async (id: string, enabled: boolean) => {
    try {
      await apiPost("/api/management/platforms/toggle", { id, enabled });
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="消息平台"
        subtitle="QQ / Lark / Telegram / Discord 等适配器"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.platforms?.length} fallback={<EmptyState message="尚未配置平台" />}>
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
                <For each={data()?.platforms ?? []}>
                  {(p) => (
                    <tr>
                      <td>{p.name ?? p.id ?? "-"}</td>
                      <td><span class="badge">{p.kind ?? "?"}</span></td>
                      <td>{p.status ?? (p.enabled ? "启用" : "禁用")}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => openEdit(p)}>编辑</Button>
                        <Button size="sm" variant="ghost" onClick={() => p.id && toggle(p.id, !p.enabled)}>
                          {p.enabled ? "停用" : "启用"}
                        </Button>
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
        title={editing() ? "编辑平台" : "新增平台"}
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={save}>保存</Button>
          </>
        }
      >
        <Field label="JSON 配置">
          <Textarea rows={20} value={draft()} onInput={(e) => setDraft(e.currentTarget.value)} />
        </Field>
      </Modal>
    </>
  );
};

export default PlatformsPage;
