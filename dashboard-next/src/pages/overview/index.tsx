import { createResource, Show, For, type Component } from "solid-js";
import { apiGet } from "@/api/client";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Button } from "@/components/Form";

interface StatusResponse {
  components?: Record<string, unknown>;
  health?: string;
  uptime_seconds?: number;
  build?: { version?: string; commit?: string };
}

interface Capabilities {
  features?: string[];
  routes?: string[];
}

const OverviewPage: Component = () => {
  const [status, { refetch }] = createResource<StatusResponse>(async () =>
    apiGet<StatusResponse>("/api/management/status").catch(() => ({}))
  );
  const [caps] = createResource<Capabilities>(async () =>
    apiGet<Capabilities>("/api/management/dashboard/capabilities").catch(() => ({}))
  );

  return (
    <>
      <PageHeader
        title="概览"
        subtitle="系统状态与运行摘要"
        actions={<Button onClick={() => refetch()}>刷新</Button>}
      />
      <Show when={!status.loading} fallback={<Loading />}>
        <div class="grid-3">
          <Card title="运行状态">
            <div class="stack">
              <div class="row">
                <span class="muted">健康</span>
                <span class="badge badge--success">{status()?.health ?? "ok"}</span>
              </div>
              <div class="row">
                <span class="muted">在线时长</span>
                <span>{status()?.uptime_seconds ?? "-"} 秒</span>
              </div>
              <div class="row">
                <span class="muted">版本</span>
                <span class="text-mono">{status()?.build?.version ?? "-"}</span>
              </div>
            </div>
          </Card>
          <Card title="组件">
            <Show when={status()?.components} fallback={<EmptyState />}>
              <For each={Object.entries(status()?.components ?? {})}>
                {([name, value]) => (
                  <div class="card__row">
                    <span>{name}</span>
                    <span class="text-mono muted">{JSON.stringify(value)}</span>
                  </div>
                )}
              </For>
            </Show>
          </Card>
          <Card title="能力">
            <Show when={caps()?.features?.length} fallback={<EmptyState />}>
              <For each={caps()?.features ?? []}>
                {(f) => <div class="card__row"><span>{f}</span></div>}
              </For>
            </Show>
          </Card>
        </div>
      </Show>
    </>
  );
};

export default OverviewPage;
