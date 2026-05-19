import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Input } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";

interface TraceEvent {
  id?: string;
  event_id?: string;
  ts?: string;
  span?: string;
  kind?: string;
  detail?: unknown;
}

interface TraceResponse {
  events?: TraceEvent[];
}

interface TraceSettings {
  enabled?: boolean;
  level?: string;
  sample_rate?: number;
}

const TracePage: Component = () => {
  const [trace, { refetch }] = createResource<TraceResponse>(async () =>
    apiGet<TraceResponse>("/api/management/trace")
  );
  const [settings, { refetch: refetchSettings }] = createResource<TraceSettings>(async () =>
    apiGet<TraceSettings>("/api/management/trace/settings").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<TraceSettings>({});
  const [selected, setSelected] = createSignal<TraceEvent | null>(null);

  const save = async () => {
    await apiPost("/api/management/trace/settings", draft());
    await refetchSettings();
    setOpen(false);
  };

  return (
    <>
      <PageHeader
        title="调用轨迹"
        subtitle="Trace 事件 + 采样设置"
        actions={
          <>
            <Button onClick={() => refetch()}>刷新</Button>
            <Button
              onClick={() => {
                setDraft(settings() ?? {});
                setOpen(true);
              }}
            >
              设置
            </Button>
          </>
        }
      />
      <Card>
        <Show when={!trace.loading} fallback={<Loading />}>
          <Show when={trace()?.events?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>Span</th>
                  <th>类型</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                <For each={trace()?.events ?? []}>
                  {(ev) => (
                    <tr>
                      <td class="text-mono">{ev.ts ?? ""}</td>
                      <td>{ev.span ?? ""}</td>
                      <td><span class="badge">{ev.kind ?? "-"}</span></td>
                      <td>
                        <Button size="sm" variant="ghost" onClick={() => setSelected(ev)}>
                          详情
                        </Button>
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
        title="Trace 设置"
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={save}>保存</Button>
          </>
        }
      >
        <Field label="启用">
          <Input
            type="checkbox"
            checked={draft().enabled ?? false}
            onChange={(e) => setDraft({ ...draft(), enabled: e.currentTarget.checked })}
          />
        </Field>
        <Field label="级别">
          <Input
            value={draft().level ?? ""}
            onInput={(e) => setDraft({ ...draft(), level: e.currentTarget.value })}
          />
        </Field>
        <Field label="采样率 (0-1)">
          <Input
            type="number"
            step="0.01"
            value={draft().sample_rate ?? 0}
            onInput={(e) => setDraft({ ...draft(), sample_rate: Number(e.currentTarget.value) })}
          />
        </Field>
      </Modal>
      <Modal open={selected() !== null} title="事件详情" onClose={() => setSelected(null)}>
        <pre class="code-block">{JSON.stringify(selected(), null, 2)}</pre>
      </Modal>
    </>
  );
};

export default TracePage;
