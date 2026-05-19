import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Input, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface SubAgent {
  id?: string;
  name?: string;
  prompt?: string;
  tools?: string[];
  enabled?: boolean;
}

interface SubAgentsResponse {
  subagents?: SubAgent[];
}

const SubAgentPage: Component = () => {
  const [data, { refetch }] = createResource<SubAgentsResponse>(async () =>
    apiGet<SubAgentsResponse>("/api/management/subagents").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<SubAgent>({});

  const openEdit = (s: SubAgent | null) => {
    setDraft(s ? { ...s } : { name: "", prompt: "", tools: [] });
    setOpen(true);
  };

  const save = async () => {
    try {
      await apiPost("/api/management/subagents/upsert", draft());
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该子代理？")) return;
    try {
      await apiPost("/api/management/subagents/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="子代理"
        subtitle="复用的 LLM 子代理"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.subagents?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>工具数</th><th>状态</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.subagents ?? []}>
                  {(s) => (
                    <tr>
                      <td>{s.name ?? s.id ?? "-"}</td>
                      <td class="text-mono">{s.tools?.length ?? 0}</td>
                      <td>{s.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => openEdit(s)}>编辑</Button>
                        <Button size="sm" variant="danger" onClick={() => s.id && del(s.id)}>删除</Button>
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
        title="子代理"
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={save}>保存</Button>
          </>
        }
      >
        <Field label="名称">
          <Input
            value={draft().name ?? ""}
            onInput={(e) => setDraft({ ...draft(), name: e.currentTarget.value })}
          />
        </Field>
        <Field label="Prompt">
          <Textarea
            rows={10}
            value={draft().prompt ?? ""}
            onInput={(e) => setDraft({ ...draft(), prompt: e.currentTarget.value })}
          />
        </Field>
        <Field label="工具（以逗号分隔）">
          <Input
            value={(draft().tools ?? []).join(",")}
            onInput={(e) =>
              setDraft({
                ...draft(),
                tools: e.currentTarget.value.split(",").map((s) => s.trim()).filter(Boolean),
              })
            }
          />
        </Field>
      </Modal>
    </>
  );
};

export default SubAgentPage;
