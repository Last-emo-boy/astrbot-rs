import { createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiGet, apiPost } from "@/api/client";
import { Button, Field, Input, Textarea } from "@/components/Form";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";

interface Project {
  id?: string;
  name?: string;
  description?: string;
  persona_id?: string;
  knowledge_bases?: string[];
}

interface ProjectsResponse {
  projects?: Project[];
}

const ProjectsPage: Component = () => {
  const [data, { refetch }] = createResource<ProjectsResponse>(async () =>
    apiGet<ProjectsResponse>("/api/management/projects").catch(() => ({}))
  );
  const [open, setOpen] = createSignal(false);
  const [draft, setDraft] = createSignal<Project>({});

  const openEdit = (item: Project | null) => {
    setDraft(item ? { ...item } : { name: "", description: "" });
    setOpen(true);
  };

  const save = async () => {
    try {
      await apiPost("/api/management/projects/upsert", draft());
      toastSuccess("已保存");
      setOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const del = async (id: string) => {
    if (!confirm("删除该项目？")) return;
    try {
      await apiPost("/api/management/projects/delete", { id });
      toastSuccess("已删除");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  return (
    <>
      <PageHeader
        title="对话项目"
        subtitle="将人格 + 知识库 + 工具组合为项目"
        actions={<Button variant="primary" onClick={() => openEdit(null)}>新增</Button>}
      />
      <Card>
        <Show when={!data.loading} fallback={<Loading />}>
          <Show when={data()?.projects?.length} fallback={<EmptyState />}>
            <table class="table">
              <thead><tr><th>名称</th><th>描述</th><th>知识库</th><th>操作</th></tr></thead>
              <tbody>
                <For each={data()?.projects ?? []}>
                  {(p) => (
                    <tr>
                      <td>{p.name ?? p.id ?? "-"}</td>
                      <td class="muted">{p.description ?? ""}</td>
                      <td class="text-mono">{(p.knowledge_bases ?? []).length}</td>
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
        title="项目"
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
        <Field label="描述">
          <Textarea
            rows={4}
            value={draft().description ?? ""}
            onInput={(e) => setDraft({ ...draft(), description: e.currentTarget.value })}
          />
        </Field>
        <Field label="人格 ID">
          <Input
            value={draft().persona_id ?? ""}
            onInput={(e) => setDraft({ ...draft(), persona_id: e.currentTarget.value })}
          />
        </Field>
        <Field label="知识库 ID 列表（逗号分隔）">
          <Input
            value={(draft().knowledge_bases ?? []).join(",")}
            onInput={(e) =>
              setDraft({
                ...draft(),
                knowledge_bases: e.currentTarget.value.split(",").map((s) => s.trim()).filter(Boolean),
              })
            }
          />
        </Field>
      </Modal>
    </>
  );
};

export default ProjectsPage;
