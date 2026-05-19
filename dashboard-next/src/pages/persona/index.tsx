import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Input, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface Persona {
  id?: string;
  name?: string;
  system_prompt?: string;
  tags?: string[];
}

interface PersonasResponse {
  personas?: Persona[];
}

const PersonaPage: Component = () => {
  const [data, { refetch }] = createResource<PersonasResponse>(async () =>
    apiGet<PersonasResponse>("/api/management/personas").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<Persona>({});

  const openEdit = (item: Persona | null) => {
    setDraft(item ? { ...item } : { name: "", system_prompt: "" });
    setOpen(true);
  };

  const save = async () => {
    try {
      await apiPost("/api/management/personas/upsert", draft());
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该人格？")) return;
    try {
      await apiPost("/api/management/personas/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="人格"
        subtitle="System Prompt 模板"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.personas?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead>
                <tr><th>名称</th><th>提示词预览</th><th>操作</th></tr>
              </thead>
              <tbody>
                <For each={data()?.personas ?? []}>
                  {(p) => (
                    <tr>
                      <td>{p.name ?? p.id ?? "-"}</td>
                      <td class="muted" style={{ "max-width": "480px", "white-space": "nowrap", overflow: "hidden", "text-overflow": "ellipsis" }}>
                        {p.system_prompt ?? ""}
                      </td>
                      <td class="row">
                        <Button size="sm" onClick={() => openEdit(p)}>编辑</Button>
                        <Button size="sm" variant="danger" onClick={() => p.id && del(p.id)}>删除</Button>
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
        title="编辑人格"
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
        <Field label="System Prompt">
          <Textarea
            rows={12}
            value={draft().system_prompt ?? ""}
            onInput={(e) => setDraft({ ...draft(), system_prompt: e.currentTarget.value })}
          />
        </Field>
      </Modal>
    </>
  );
};

export default PersonaPage;
