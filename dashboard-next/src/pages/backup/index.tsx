import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiFetch, apiGet, apiPost, apiPostMultipart } from "@/api/client";
import { Button, Input } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { toastError, toastSuccess } from "@/components/Toast";

interface BackupEntry {
  id?: string;
  name?: string;
  size?: number;
  created_at?: string;
}

interface BackupResponse {
  backups?: BackupEntry[];
}

const BackupPage: Component = () => {
  const [data, { refetch }] = createResource<BackupResponse>(async () =>
    apiGet<BackupResponse>("/api/management/backups").catch(() => ({}))
  );
  const [file, setFile] = createSignal<File | null>(null);

  const create = async () => {
    try {
      await apiPost("/api/management/backups/create", {});
      toastSuccess("已创建备份");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const download = async (id: string) => {
    try {
      const res = await apiFetch(`/api/management/backups/${id}/download`);
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${id}.zip`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      toastError(err);
    }
  };

  const restore = async () => {
    const f = file();
    if (!f) return;
    if (!confirm("从该备份恢复将覆盖当前数据，确认继续？")) return;
    const form = new FormData();
    form.append("file", f);
    try {
      await apiPostMultipart("/api/management/backups/restore", form);
      toastSuccess("恢复已开始");
      setFile(null);
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该备份？")) return;
    try {
      await apiPost("/api/management/backups/delete", { id });
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="备份"
        subtitle="导出 / 恢复运行时数据"
        actions={<Button variant="primary" onClick={create}>立即备份</Button>}
      />
      <Card title="恢复">
        <div class="row">
          <Input type="file" accept=".zip" onChange={(e) => setFile(e.currentTarget.files?.[0] ?? null)} />
          <Button variant="danger" disabled={!file()} onClick={restore}>恢复</Button>
        </div>
      </Card>
      <Card title="备份列表">
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.backups?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>大小</th><th>时间</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.backups ?? []}>
                  {(b) => (
                    <tr>
                      <td>{b.name ?? b.id ?? "-"}</td>
                      <td class="text-mono">{b.size ?? 0}</td>
                      <td class="text-mono">{b.created_at ?? ""}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => b.id && download(b.id)}>下载</Button>
                        <Button size="sm" variant="danger" onClick={() => b.id && del(b.id)}>删除</Button>
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

export default BackupPage;
