import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface McpServer {
  id?: string;
  name?: string;
  transport?: string;
  endpoint?: string;
  status?: string;
  tools?: Array<{ name?: string; description?: string }>;
}

interface McpResponse {
  servers?: McpServer[];
}

const McpPage: Component = () => {
  const [data, { refetch }] = createResource<McpResponse>(async () =>
    apiGet<McpResponse>("/api/management/mcp/servers").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<string>("{}");

  const openAdd = () => {
    setDraft(JSON.stringify({ name: "", transport: "stdio", endpoint: "", env: {} }, null, 2));
    setOpen(true);
  };

  const save = async () => {
    try {
      const payload = JSON.parse(draft());
      await apiPost("/api/management/mcp/upsert", payload);
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该 MCP 服务？")) return;
    try {
      await apiPost("/api/management/mcp/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const reconnect = async (id: string) => {
    try {
      await apiPost("/api/management/mcp/reconnect", { id });
      toastSuccess("已发起重连");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="MCP 服务"
        subtitle="Model Context Protocol 上游"
        actions={
          <>
            <Button onClick={() => refetch()}>刷新</Button>
            <Button variant="primary" onClick={openAdd}>新增</Button>
          </>
        }
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.servers?.length} fallback={<EmptyState message="尚未配置 MCP 服务" />}>
            <table class="table">
              <thead>
                <tr><th>名称</th><th>Transport</th><th>状态</th><th>工具</th><th>操作</th></tr>
              </thead>
              <tbody>
                <For each={data()?.servers ?? []}>
                  {(s) => (
                    <tr>
                      <td>{s.name ?? s.id ?? "-"}</td>
                      <td><span class="badge">{s.transport ?? "?"}</span></td>
                      <td>{s.status ?? "-"}</td>
                      <td class="text-mono">{s.tools?.length ?? 0}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => s.id && reconnect(s.id)}>重连</Button>
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
        title="新增 MCP 服务"
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={save}>保存</Button>
          </>
        }
      >
        <Field label="JSON 配置">
          <Textarea rows={16} value={draft()} onInput={(e) => setDraft(e.currentTarget.value)} />
        </Field>
      </Modal>
    </>
  );
};

export default McpPage;
