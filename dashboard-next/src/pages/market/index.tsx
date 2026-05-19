import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Input } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { toastError, toastSuccess } from "@/components/Toast";

interface MarketItem {
  id?: string;
  name?: string;
  author?: string;
  description?: string;
  repo?: string;
  tags?: string[];
  installed?: boolean;
}

interface MarketResponse {
  items?: MarketItem[];
}

const MarketPage: Component = () => {
  const [q, setQ] = createSignal("");
  const [data, { refetch }] = createResource(q, async (query) => {
    const path = query
      ? `/api/management/market/search?q=${encodeURIComponent(query)}`
      : "/api/management/market/list";
    return apiGet<MarketResponse>(path).catch(() => ({} as MarketResponse));
  });

  const install = async (repo: string) => {
    try {
      await apiPost("/api/management/plugins/install", { url: repo });
      toastSuccess("安装请求已提交");
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader title="插件市场" subtitle="社区贡献的插件目录" />
      <Card>
        <div class="row">
          <Input
            placeholder="搜索插件…"
            value={q()}
            onInput={(e) => setQ(e.currentTarget.value)}
          />
          <Button onClick={() => refetch()}>搜索</Button>
        </div>
      </Card>
      <Show when={!data.loading} fallback={<Loading />}>
        <Show when={data()?.items?.length} fallback={<EmptyState />}>
          <div class="grid-2">
            <For each={data()?.items ?? []}>
              {(item) => (
                <Card title={item.name ?? "-"}>
                  <div class="muted" style={{ "margin-bottom": "8px" }}>
                    by {item.author ?? "?"} · {(item.tags ?? []).join(" / ")}
                  </div>
                  <p>{item.description ?? ""}</p>
                  <div class="row">
                    <Button
                      size="sm"
                      variant="primary"
                      disabled={item.installed}
                      onClick={() => item.repo && install(item.repo)}
                    >
                      {item.installed ? "已安装" : "安装"}
                    </Button>
                    <Show when={item.repo}>
                      <a class="btn btn--ghost btn--sm" href={item.repo} target="_blank" rel="noreferrer">
                        仓库
                      </a>
                    </Show>
                  </div>
                </Card>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </>
  );
};

export default MarketPage;
