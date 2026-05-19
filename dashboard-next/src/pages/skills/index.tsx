import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface Skill {
  id?: string;
  name?: string;
  description?: string;
  enabled?: boolean;
  triggers?: string[];
}

interface SkillsResponse {
  skills?: Skill[];
}

const SkillsPage: Component = () => {
  const [data, { refetch }] = createResource<SkillsResponse>(async () =>
    apiGet<SkillsResponse>("/api/management/skills").catch(() => ({}))
  );
  const [selected, setSelected] = createSignal<Skill | null>(null);

  const toggle = async (id: string, enabled: boolean) => {
    try {
      await apiPost("/api/management/skills/toggle", { id, enabled });
      toastSuccess(enabled ? "已启用" : "已禁用");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader title="技能" subtitle="独立可调用的技能函数" />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.skills?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>描述</th><th>状态</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.skills ?? []}>
                  {(s) => (
                    <tr>
                      <td>{s.name ?? s.id ?? "-"}</td>
                      <td class="muted">{s.description ?? ""}</td>
                      <td>{s.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" variant="ghost" onClick={() => setSelected(s)}>详情</Button>
                        <Button size="sm" onClick={() => s.id && toggle(s.id, !s.enabled)}>
                          {s.enabled ? "停用" : "启用"}
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
      <Modal open={selected() !== null} title="技能详情" onClose={() => setSelected(null)}>
        <pre class="code-block">{JSON.stringify(selected(), null, 2)}</pre>
      </Modal>
    </>
  );
};

export default SkillsPage;
