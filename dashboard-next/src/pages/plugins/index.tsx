import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost, apiPostMultipart } from "@/api/client";
import { Button, Field, Input } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface Plugin {
  id?: string;
  name?: string;
  version?: string;
  description?: string;
  enabled?: boolean;
  source?: string;
}

interface PluginsResponse {
  plugins?: Plugin[];
}

const PluginsPage: Component = () => {
  const [data, { refetch }] = createResource<PluginsResponse>(async () =>
    apiGet<PluginsResponse>("/api/management/plugins").catch(() => ({}))
  );
  const [installOpen, setInstallOpen] = createSignal(false);
  const [repoUrl, setRepoUrl] = createSignal("");
  const [file, setFile] = createSignal<File | null>(null);

  const toggle = async (id: string, enabled: boolean) => {
    try {
      await apiPost("/api/management/plugins/toggle", { id, enabled });
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const uninstall = async (id: string) => {
    if (!confirm("卸载该插件？")) return;
    try {
      await apiPost("/api/management/plugins/uninstall", { id });
      toastSuccess("已卸载");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const installFromUrl = async () => {
    try {
      await apiPost("/api/management/plugins/install", { url: repoUrl() });
      toastSuccess("安装请求已提交");
      setInstallOpen(false);
      setRepoUrl("");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const uploadZip = async () => {
    const f = file();
    if (!f) return;
    const form = new FormData();
    form.append("file", f);
    try {
      await apiPostMultipart("/api/management/plugins/upload", form);
      toastSuccess("已上传安装");
      setInstallOpen(false);
      setFile(null);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="插件"
        subtitle="本地已安装插件"
        actions={<Button variant="primary" onClick={() => setInstallOpen(true)}>安装插件</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.plugins?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr><th>名称</th><th>版本</th><th>来源</th><th>状态</th><th>操作</th></tr>
              </thead>
              <tbody>
                <For each={data()?.plugins ?? []}>
                  {(p) => (
                    <tr>
                      <td>{p.name ?? p.id ?? "-"}</td>
                      <td class="text-mono">{p.version ?? "-"}</td>
                      <td><span class="badge">{p.source ?? "?"}</span></td>
                      <td>{p.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => p.id && toggle(p.id, !p.enabled)}>
                          {p.enabled ? "停用" : "启用"}
                        </Button>
                        <Button size="sm" variant="danger" onClick={() => p.id && uninstall(p.id)}>卸载</Button>
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
        open={installOpen()}
        title="安装插件"
        onClose={() => setInstallOpen(false)}
        actions={
          <>
            <Button onClick={() => setInstallOpen(false)}>取消</Button>
            <Button variant="primary" onClick={installFromUrl} disabled={!repoUrl()}>从 URL 安装</Button>
            <Button variant="primary" onClick={uploadZip} disabled={!file()}>上传安装</Button>
          </>
        }
      >
        <Field label="Git 仓库 URL">
          <Input value={repoUrl()} onInput={(e) => setRepoUrl(e.currentTarget.value)} placeholder="https://github.com/..." />
        </Field>
        <Field label="或上传 zip 包">
          <Input type="file" accept=".zip" onChange={(e) => setFile(e.currentTarget.files?.[0] ?? null)} />
        </Field>
      </Modal>
    </>
  );
};

export default PluginsPage;
