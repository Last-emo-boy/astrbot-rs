import { createResource, createSignal, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Textarea } from "@/components/Form";
import { Card, Loading, PageHeader } from "@/components/Card";
import { toastError, toastSuccess } from "@/components/Toast";

interface ConfigCurrentResponse {
  config?: unknown;
  routes?: unknown;
}

const ConfigPage: Component = () => {
  const [current, { refetch }] = createResource<ConfigCurrentResponse>(async () =>
    apiGet<ConfigCurrentResponse>("/api/management/config/current")
  );
  const [draft, setDraft] = createSignal<string>("");

  const initialize = (data: ConfigCurrentResponse | undefined) => {
    if (!data) return;
    setDraft(JSON.stringify(data.config ?? data, null, 2));
  };

  const preview = async () => {
    try {
      const parsed = JSON.parse(draft());
      const res = await apiPost("/api/management/config/preview", { config: parsed });
      toastSuccess("预检通过");
      console.log("preview", res);
    } catch (err) {
      toastError(err);
    }
  };

  const apply = async () => {
    try {
      const parsed = JSON.parse(draft());
      await apiPost("/api/management/config/apply", { config: parsed });
      toastSuccess("已应用");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="全局配置"
        subtitle="编辑 abconfs 与运行时配置（JSON 直接编辑，需自行保证结构有效）"
        actions={
          <>
            <Button onClick={() => initialize(current())}>载入</Button>
            <Button onClick={preview}>预检</Button>
            <Button variant="primary" onClick={apply}>应用</Button>
          </>
        }
      />
      <Show when={!current.loading} fallback={<Loading />}>
        <Card title="配置 JSON">
          <Textarea
            rows={28}
            value={draft() || JSON.stringify(current()?.config ?? current() ?? {}, null, 2)}
            onInput={(e) => setDraft(e.currentTarget.value)}
          />
        </Card>
      </Show>
    </>
  );
};

export default ConfigPage;
