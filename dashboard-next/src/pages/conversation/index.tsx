import { createResource, For, Show, type Component } from "solid-js";
import { A } from "@solidjs/router";
import { apiGet, apiPost } from "@/api/client";
import { Button } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { toastError, toastSuccess } from "@/components/Toast";

interface Conversation {
  id?: string;
  title?: string;
  updated_at?: string;
  message_count?: number;
}

interface ConversationsResponse {
  conversations?: Conversation[];
}

const ConversationPage: Component = () => {
  const [data, { refetch }] = createResource<ConversationsResponse>(async () =>
    apiGet<ConversationsResponse>("/api/management/conversations").catch(() => ({}))
  );

  const del = async (id: string) => {
    if (!confirm("删除该对话？")) return;
    try {
      await apiPost("/api/management/conversations/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="对话历史"
        subtitle="所有平台的会话记录"
        actions={<Button onClick={() => refetch()}>刷新</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.conversations?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr>
                  <th>标题</th>
                  <th>消息数</th>
                  <th>更新时间</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                <For each={data()?.conversations ?? []}>
                  {(c) => (
                    <tr>
                      <td>
                        <A href={`/chat/${c.id ?? ""}`}>{c.title ?? c.id ?? "-"}</A>
                      </td>
                      <td class="text-mono">{c.message_count ?? 0}</td>
                      <td class="text-mono">{c.updated_at ?? ""}</td>
                      <td>
                        <Button size="sm" variant="danger" onClick={() => c.id && del(c.id)}>删除</Button>
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

export default ConversationPage;
