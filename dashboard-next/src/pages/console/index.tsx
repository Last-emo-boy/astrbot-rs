import { createResource, createSignal, For, onCleanup, Show, type Component } from "solid-js";
import { apiGet, buildEventSource } from "@/api/client";
import { Button } from "@/components/Form";
import { Card, Loading, PageHeader } from "@/components/Card";

interface LogEntry {
  level?: string;
  message?: string;
  ts?: string;
  timestamp?: string;
  module?: string;
}

interface LogResponse {
  logs?: LogEntry[];
  entries?: LogEntry[];
}

const ConsolePage: Component = () => {
  const [level, setLevel] = createSignal<string>("info");
  const [streaming, setStreaming] = createSignal(false);
  const [liveEntries, setLiveEntries] = createSignal<LogEntry[]>([]);
  let es: EventSource | null = null;

  const [snapshot, { refetch }] = createResource<LogResponse>(
    async () => apiGet<LogResponse>(`/api/management/logs?level=${encodeURIComponent(level())}&limit=200`)
  );

  const entries = () => {
    const live = liveEntries();
    if (live.length) return live;
    return snapshot()?.logs ?? snapshot()?.entries ?? [];
  };

  const toggleStream = () => {
    if (streaming()) {
      es?.close();
      es = null;
      setStreaming(false);
      return;
    }
    setLiveEntries([]);
    es = buildEventSource(`/api/management/logs/stream?level=${encodeURIComponent(level())}`);
    es.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data) as LogEntry;
        setLiveEntries((list) => [...list.slice(-499), data]);
      } catch {
        setLiveEntries((list) => [
          ...list.slice(-499),
          { level: "info", message: event.data },
        ]);
      }
    };
    es.onerror = () => {
      setStreaming(false);
      es?.close();
    };
    setStreaming(true);
  };

  onCleanup(() => {
    es?.close();
  });

  return (
    <>
      <PageHeader
        title="控制台日志"
        subtitle="实时查看运行日志（SSE 流）"
        actions={
          <>
            <select class="input" value={level()} onChange={(e) => setLevel(e.currentTarget.value)}>
              <option value="trace">trace</option>
              <option value="debug">debug</option>
              <option value="info">info</option>
              <option value="warn">warn</option>
              <option value="error">error</option>
            </select>
            <Button onClick={() => refetch()}>快照</Button>
            <Button variant={streaming() ? "danger" : "primary"} onClick={toggleStream}>
              {streaming() ? "停止" : "开始流"}
            </Button>
          </>
        }
      />
      <Card>
        <Show when={!snapshot.loading} fallback={<Loading />}>
          <div class="code-block" style={{ "max-height": "calc(100vh - 240px)" }}>
            <For each={entries()}>
              {(entry) => (
                <div>
                  <span class={`badge badge--${entry.level === "error" ? "danger" : entry.level === "warn" ? "danger" : "accent"}`}>
                    {entry.level ?? "info"}
                  </span>{" "}
                  <span class="muted text-mono">{entry.ts ?? entry.timestamp ?? ""}</span>{" "}
                  <span class="muted">{entry.module ?? ""}</span>{" "}
                  <span>{entry.message ?? ""}</span>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Card>
    </>
  );
};

export default ConsolePage;
