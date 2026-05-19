import { createResource, createSignal, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button } from "@/components/Form";
import { Card, Loading, PageHeader } from "@/components/Card";
import { toastError, toastSuccess } from "@/components/Toast";

interface UpdateInfo {
  current?: string;
  latest?: string;
  available?: boolean;
  notes?: string;
}

const UpdatePage: Component = () => {
  const [info, { refetch }] = createResource<UpdateInfo>(async () =>
    apiGet<UpdateInfo>("/api/management/update/check").catch(() => ({}))
  );
  const [busy, setBusy] = createSignal(false);

  const performUpdate = async () => {
    if (!confirm("立即升级 AstrBot Runtime？升级期间服务可能短暂不可用。")) return;
    setBusy(true);
    try {
      await apiPost("/api/management/update/perform", {});
      toastSuccess("升级请求已提交");
    } catch (err) {
      toastError(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <PageHeader
        title="更新"
        subtitle="检查并应用 AstrBot 升级"
        actions={<Button onClick={() => refetch()}>检查</Button>}
      />
      <Card>
        <Show when={!info.loading} fallback={<Loading />}>
          <div class="card__row">
            <span>当前版本</span>
            <span class="text-mono">{info()?.current ?? "-"}</span>
          </div>
          <div class="card__row">
            <span>最新版本</span>
            <span class="text-mono">{info()?.latest ?? "-"}</span>
          </div>
          <div class="card__row">
            <span>可升级</span>
            <span>{info()?.available ? "是" : "否"}</span>
          </div>
          <Show when={info()?.notes}>
            <pre class="code-block">{info()?.notes}</pre>
          </Show>
          <div class="row" style={{ "margin-top": "12px" }}>
            <Button variant="primary" disabled={!info()?.available || busy()} onClick={performUpdate}>
              {busy() ? "升级中…" : "立即升级"}
            </Button>
          </div>
        </Show>
      </Card>
    </>
  );
};

export default UpdatePage;
