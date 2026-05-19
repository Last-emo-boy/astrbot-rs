import { createMemo, createResource, For, Show, type Component } from "solid-js";
import { apiGet } from "@/api/client";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { EChart } from "@/components/EChart";

interface ProviderStat {
  name?: string;
  total_tokens?: number;
  calls?: number;
}

interface StatsResponse {
  tokens?: Record<string, number>;
  providers?: ProviderStat[];
  totals?: Record<string, number>;
}

const ObservabilityPage: Component = () => {
  const [stats, { refetch }] = createResource<StatsResponse>(async () =>
    apiGet<StatsResponse>("/api/management/stats").catch(() => ({}))
  );

  const tokenChartOption = createMemo(() => {
    const tokens = stats()?.tokens ?? {};
    const entries = Object.entries(tokens);
    return {
      grid: { left: 40, right: 20, top: 20, bottom: 32 },
      xAxis: {
        type: "category" as const,
        data: entries.map(([key]) => key),
        axisLabel: { color: "#9ca3af" },
      },
      yAxis: {
        type: "value" as const,
        axisLabel: { color: "#9ca3af" },
      },
      tooltip: { trigger: "axis" as const },
      series: [
        {
          type: "bar" as const,
          data: entries.map(([, value]) => value),
          itemStyle: { color: "#5b8def" },
        },
      ],
    };
  });

  const providerChartOption = createMemo(() => {
    const providers = stats()?.providers ?? [];
    return {
      tooltip: { trigger: "item" as const },
      legend: { bottom: 0, textStyle: { color: "#9ca3af" } },
      series: [
        {
          name: "Provider 调用",
          type: "pie" as const,
          radius: ["35%", "65%"],
          data: providers.map((p) => ({
            name: p.name ?? "(unknown)",
            value: p.calls ?? 0,
          })),
          label: { color: "#9ca3af" },
        },
      ],
    };
  });

  return (
    <>
      <PageHeader title="观测" subtitle="Token 用量与 Provider 调用统计" />
      <Show when={!stats.loading} fallback={<Loading />}>
        <div class="grid-2">
          <Card title="Token 用量">
            <Show
              when={Object.keys(stats()?.tokens ?? {}).length > 0}
              fallback={<EmptyState message="后端尚未上报指标" />}
            >
              <EChart option={tokenChartOption()} />
            </Show>
          </Card>
          <Card title="Provider 调用分布">
            <Show
              when={(stats()?.providers?.length ?? 0) > 0}
              fallback={<EmptyState />}
            >
              <EChart option={providerChartOption()} />
            </Show>
          </Card>
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
          <Card title="Provider 明细">
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
