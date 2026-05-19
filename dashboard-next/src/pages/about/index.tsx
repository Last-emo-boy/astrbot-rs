import { createResource, Show, type Component } from "solid-js";
import { apiGet } from "@/api/client";
import { Card, Loading, PageHeader } from "@/components/Card";

interface BuildInfo {
  build?: { version?: string; commit?: string; rust_version?: string };
  uptime_seconds?: number;
}

const AboutPage: Component = () => {
  const [info] = createResource<BuildInfo>(async () =>
    apiGet<BuildInfo>("/api/management/status").catch(() => ({}))
  );
  return (
    <>
      <PageHeader title="关于 AstrBot" subtitle="构建信息与版权" />
      <Show when={!info.loading} fallback={<Loading />}>
        <Card title="版本">
          <div class="card__row">
            <span>版本号</span>
            <span class="text-mono">{info()?.build?.version ?? "-"}</span>
          </div>
          <div class="card__row">
            <span>提交</span>
            <span class="text-mono">{info()?.build?.commit ?? "-"}</span>
          </div>
          <div class="card__row">
            <span>Rust</span>
            <span class="text-mono">{info()?.build?.rust_version ?? "-"}</span>
          </div>
        </Card>
        <Card title="项目">
          <p>
            AstrBot 是开源的多平台 LLM 聊天机器人框架。Dashboard Next 基于 Solid + Vite +
            TypeScript 构建。
          </p>
          <p>
            <a href="https://github.com/AstrBotDevs/AstrBot" target="_blank" rel="noreferrer">
              GitHub Repository
            </a>
          </p>
        </Card>
      </Show>
    </>
  );
};

export default AboutPage;
