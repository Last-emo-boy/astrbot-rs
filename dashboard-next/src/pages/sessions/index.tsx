import { createResource, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { toastError, toastSuccess } from "@/components/Toast";

interface SessionItem {
  id?: string;
  platform?: string;
  user?: string;
  conversation_id?: string;
  enabled?: boolean;
  last_active?: string;
}

interface SessionsResponse {
  sessions?: SessionItem[];
}

const SessionsPage: Component = () => {
  const [data, { refetch }] = createResource<SessionsResponse>(async () =>
    apiGet<SessionsResponse>("/api/management/sessions").catch(() => ({}))
  );

  const toggle = async (id: string, enabled: boolean) => {
    try {
      await apiPost("/api/management/sessions/toggle", { id, enabled });
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const reset = async (id: string) => {
    if (!confirm("重置该会话？")) return;
    try {
      await apiPost("/api/management/sessions/reset", { id });
      toastSuccess("已重置");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="会话管理"
        subtitle="平台用户会话状态"
        actions={<Button onClick={() => refetch()}>刷新</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.sessions?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr><th>平台</th><th>用户</th><th>对话</th><th>最近活跃</th><th>状态</th><th>操作</th></tr>
              </thead>
              <tbody>
                <For each={data()?.sessions ?? []}>
                  {(s) => (
                    <tr>
                      <td><span class="badge">{s.platform ?? "?"}</span></td>
                      <td>{s.user ?? "-"}</td>
                      <td class="muted">{s.conversation_id ?? "-"}</td>
                      <td class="text-mono">{s.last_active ?? ""}</td>
                      <td>{s.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => s.id && toggle(s.id, !s.enabled)}>
                          {s.enabled ? "停用" : "启用"}
                        </Button>
                        <Button size="sm" variant="danger" onClick={() => s.id && reset(s.id)}>重置</Button>
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

export default SessionsPage;
