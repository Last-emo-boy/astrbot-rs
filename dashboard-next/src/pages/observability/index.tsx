import { createResource, For, Show, type Component } from "solid-js";
import { apiGet } from "@/api/client";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";

interface StatsResponse {
  tokens?: Record<string, number>;
  providers?: Array<{ name?: string; total_tokens?: number; calls?: number }>;
  totals?: Record<string, number>;
}

const ObservabilityPage: Component = () => {
  const [stats, { refetch }] = createResource<StatsResponse>(async () =>
    apiGet<StatsResponse>("/api/management/stats").catch(() => ({}))
  );

  return (
    <>
      <PageHeader title="观测" subtitle="Token 用量与 Provider 调用统计" />
      <Show when={!stats.loading} fallback={<Loading />}>
        <div class="grid-2">
          <Card title="总览">
            <Show when={stats()?.totals} fallback={<EmptyState message="后端尚未上报指标" />}>
              <For each={Object.entries(stats()?.totals ?? {})}>
                {([key, val]) => (
                  <div class="card__row">
                    <span class="muted">{key}</span>
                    <span class="text-mono">{val}</span>
                  </div>
                )}
              </For>
            </Show>
          </Card>
          <Card title="Provider 调用">
            <Show when={stats()?.providers?.length} fallback={<EmptyState />}>
              <table class="table">
                <thead>
                  <tr><th>Provider</th><th>调用</th><th>Token</th></tr>
                </thead>
                <tbody>
                  <For each={stats()?.providers ?? []}>
                    {(p) => (
                      <tr>
                        <td>{p.name}</td>
                        <td class="text-mono">{p.calls ?? 0}</td>
                        <td class="text-mono">{p.total_tokens ?? 0}</td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </Show>
          </Card>
        </div>
      </Show>
      <button class="btn" onClick={() => refetch()}>刷新</button>
    </>
  );
};

export default ObservabilityPage;
