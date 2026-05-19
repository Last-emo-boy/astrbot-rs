import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface CronJob {
  id?: string;
  name?: string;
  cron?: string;
  next_run?: string;
  enabled?: boolean;
  payload?: Record<string, unknown>;
}

interface CronResponse {
  jobs?: CronJob[];
}

const CronPage: Component = () => {
  const [data, { refetch }] = createResource<CronResponse>(async () =>
    apiGet<CronResponse>("/api/management/cron").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<string>("{}");

  const openEdit = (job: CronJob | null) => {
    setDraft(JSON.stringify(job ?? { name: "", cron: "0 0 * * *", payload: {} }, null, 2));
    setOpen(true);
  };

  const save = async () => {
    try {
      const payload = JSON.parse(draft());
      await apiPost("/api/management/cron/upsert", payload);
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该计划任务？")) return;
    try {
      await apiPost("/api/management/cron/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const trigger = async (id: string) => {
    try {
      await apiPost("/api/management/cron/trigger", { id });
      toastSuccess("已触发");
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="定时任务"
        subtitle="Cron 调度"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.jobs?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>Cron</th><th>下次执行</th><th>状态</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.jobs ?? []}>
                  {(j) => (
                    <tr>
                      <td>{j.name ?? j.id ?? "-"}</td>
                      <td class="text-mono">{j.cron ?? ""}</td>
                      <td class="text-mono">{j.next_run ?? ""}</td>
                      <td>{j.enabled ? "启用" : "禁用"}</td>
                      <td class="row">
                        <Button size="sm" onClick={() => openEdit(j)}>编辑</Button>
                        <Button size="sm" variant="ghost" onClick={() => j.id && trigger(j.id)}>立即执行</Button>
                        <Button size="sm" variant="danger" onClick={() => j.id && del(j.id)}>删除</Button>
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
        title="计划任务"
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

export default CronPage;
