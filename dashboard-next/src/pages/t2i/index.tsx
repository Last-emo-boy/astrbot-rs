import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Input, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface T2iTemplate {
  id?: string;
  name?: string;
  description?: string;
  template?: string;
  engine?: string;
}

interface T2iResponse {
  templates?: T2iTemplate[];
}

const T2iPage: Component = () => {
  const [data, { refetch }] = createResource<T2iResponse>(async () =>
    apiGet<T2iResponse>("/api/management/t2i/templates").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<T2iTemplate>({});
  const [previewHtml, setPreviewHtml] = createSignal<string | null>(null);

  const openEdit = (item: T2iTemplate | null) => {
    setDraft(item ? { ...item } : { name: "", engine: "html", template: "" });
    setOpen(true);
  };

  const save = async () => {
    try {
      await apiPost("/api/management/t2i/templates/upsert", draft());
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该模板？")) return;
    try {
      await apiPost("/api/management/t2i/templates/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const preview = async (item: T2iTemplate) => {
    try {
      const res = await apiPost<{ html?: string; image_url?: string }>(
        "/api/management/t2i/templates/preview",
        { id: item.id }
      );
      setPreviewHtml(res?.html ?? `<img src="${res?.image_url ?? ""}" />`);
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="T2I 模板"
        subtitle="文本转图模板（HTML/Jinja）"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.templates?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>引擎</th><th>描述</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.templates ?? []}>
                  {(t) => (
                    <tr>
                      <td>{t.name ?? t.id ?? "-"}</td>
                      <td><span class="badge">{t.engine ?? "?"}</span></td>
                      <td class="muted">{t.description ?? ""}</td>
                      <td class="row">
                        <Button size="sm" variant="ghost" onClick={() => preview(t)}>预览</Button>
                        <Button size="sm" onClick={() => openEdit(t)}>编辑</Button>
                        <Button size="sm" variant="danger" onClick={() => t.id && del(t.id)}>删除</Button>
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
        title="T2I 模板"
        onClose={() => setOpen(false)}
        actions={
          <>
            <Button onClick={() => setOpen(false)}>取消</Button>
            <Button variant="primary" onClick={save}>保存</Button>
          </>
        }
      >
        <Field label="名称">
          <Input value={draft().name ?? ""} onInput={(e) => setDraft({ ...draft(), name: e.currentTarget.value })} />
        </Field>
        <Field label="引擎">
          <Input value={draft().engine ?? ""} onInput={(e) => setDraft({ ...draft(), engine: e.currentTarget.value })} />
        </Field>
        <Field label="模板源">
          <Textarea
            rows={16}
            value={draft().template ?? ""}
            onInput={(e) => setDraft({ ...draft(), template: e.currentTarget.value })}
          />
        </Field>
      </Modal>
      <Modal open={previewHtml() !== null} title="预览" onClose={() => setPreviewHtml(null)}>
        <div innerHTML={previewHtml() ?? ""} />
      </Modal>
    </>
  );
};

export default T2iPage;
