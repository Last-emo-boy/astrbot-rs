import { createEffect, createMemo, createResource, createSignal, For, Show, type Component } from "solid-js";
import { apiPost } from "@/api/client";
import { Card, EmptyState, Loading, PageHeader } from "@/components/Card";
import { Button, Field, Input, Textarea } from "@/components/Form";
import { Modal } from "@/components/Modal";
import { toastError, toastSuccess } from "@/components/Toast";
import { FolderTree, type FolderNode, type TreeItem } from "@/features/folder-tree/FolderTree";

interface Persona {
  id?: string;
  name?: string;
  system_prompt?: string;
  tags?: string[];
  folder_id?: string | null;
  sort_order?: number;
}

interface PersonasResponse {
  personas?: Persona[];
  folders?: Array<{ id: string; name: string; parent_id?: string | null; description?: string; sort_order?: number }>;
}

type ContextTarget = { type: "folder" | "item"; id: string } | null;

const SELECTED_FOLDER_KEY = "astrbot.dashboard.persona.selected-folder";
const SELECTED_ITEM_KEY = "astrbot.dashboard.persona.selected-item";

function readStorage(key: string): string | null {
  try {
    const value = localStorage.getItem(key);
    return value === "__root__" ? null : value;
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string | null): void {
  try {
    localStorage.setItem(key, value ?? "__root__");
  } catch {
    /* ignore storage errors */
  }
}

function personaId(persona: Persona): string {
  return persona.id ?? persona.name ?? "";
}

function personaName(persona: Persona): string {
  return persona.name ?? persona.id ?? "-";
}

function idFromName(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "") || `persona-${Date.now()}`;
}

function folderDepth(folders: FolderNode[], folderId: string | null): number {
  let depth = 0;
  let current = folders.find((folder) => folder.id === folderId);
  while (current) {
    depth += 1;
    current = folders.find((folder) => folder.id === current?.parentId);
  }
  return depth;
}

function isDescendant(folders: FolderNode[], folderId: string, maybeParentId: string | null): boolean {
  let current = folders.find((folder) => folder.id === maybeParentId);
  while (current) {
    if (current.id === folderId) return true;
    current = folders.find((folder) => folder.id === current?.parentId);
  }
  return false;
}

const PersonaPage: Component = () => {
  const [data, { refetch }] = createResource<PersonasResponse>(async () =>
    apiPost<PersonasResponse>("/api/management/personas", {}).catch(() => ({}))
  );
  const [selectedFolderId, setSelectedFolderId] = createSignal<string | null>(readStorage(SELECTED_FOLDER_KEY));
  const [selectedItemId, setSelectedItemId] = createSignal<string | null>(readStorage(SELECTED_ITEM_KEY));
  const [open, setOpen] = createSignal(false);
  const [folderOpen, setFolderOpen] = createSignal(false);
  const [folderDraft, setFolderDraft] = createSignal<{ id?: string; name: string; parentId: string | null }>({
    name: "",
    parentId: null,
  });
  const [draft, setDraft] = createSignal<Persona>({});
  const [contextMenu, setContextMenu] = createSignal<{ x: number; y: number; target: ContextTarget } | null>(null);

  createEffect(() => writeStorage(SELECTED_FOLDER_KEY, selectedFolderId()));
  createEffect(() => writeStorage(SELECTED_ITEM_KEY, selectedItemId()));

  const personas = createMemo(() => data()?.personas ?? []);
  const folders = createMemo<FolderNode[]>(() =>
    (data()?.folders ?? []).map((folder) => ({
      id: folder.id,
      name: folder.name,
      parentId: folder.parent_id ?? null,
    }))
  );
  const treeItems = createMemo<TreeItem[]>(() =>
    personas().map((persona) => ({
      id: personaId(persona),
      name: personaName(persona),
      folderId: persona.folder_id ?? null,
    })).filter((item) => item.id)
  );
  const selectedPersona = createMemo(() => personas().find((persona) => personaId(persona) === selectedItemId()));
  const visiblePersonas = createMemo(() =>
    selectedFolderId() === null ? personas() : personas().filter((persona) => persona.folder_id === selectedFolderId())
  );
  const breadcrumb = createMemo(() => {
    const path: FolderNode[] = [];
    let current = folders().find((folder) => folder.id === selectedFolderId());
    while (current) {
      path.unshift(current);
      current = folders().find((folder) => folder.id === current?.parentId);
    }
    return path;
  });

  const openEdit = (item: Persona | null) => {
    setDraft(item ? { ...item, name: personaName(item) } : { name: "", system_prompt: "", folder_id: selectedFolderId() });
    setOpen(true);
    setContextMenu(null);
  };

  const save = async () => {
    try {
      const current = draft();
      const id = personaId(current) || idFromName(current.name ?? "");
      await apiPost("/api/management/personas/upsert", {
        id,
        system_prompt: current.system_prompt ?? "",
        folder_id: current.folder_id ?? null,
        sort_order: current.sort_order ?? 0,
      });
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
      if (selectedItemId() === id) setSelectedItemId(null);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const movePersona = async (id: string, folderId: string | null) => {
    const persona = personas().find((item) => personaId(item) === id);
    if (!persona) return;
    try {
      await apiPost("/api/management/personas/move", { id, folder_id: folderId });
      toastSuccess("已移动");
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const openFolderModal = (folder: FolderNode | null, parentId = selectedFolderId()) => {
    setFolderDraft(folder ? { id: folder.id, name: folder.name, parentId: folder.parentId } : { name: "", parentId });
    setFolderOpen(true);
    setContextMenu(null);
  };

  const saveFolder = async () => {
    const draftFolder = folderDraft();
    const parentDepth = folderDepth(folders(), draftFolder.parentId);
    if (!draftFolder.name.trim()) return;
    if (parentDepth >= 3) {
      toastError("最多支持三层文件夹");
      return;
    }
    try {
      await apiPost("/api/management/personas/folders/upsert", {
        id: draftFolder.id ?? idFromName(draftFolder.name),
        name: draftFolder.name.trim(),
        parent_id: draftFolder.parentId,
      });
      toastSuccess("已保存文件夹");
      setFolderOpen(false);
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const deleteFolder = async (id: string) => {
    if (!confirm("删除该文件夹？人格会移动到根目录。")) return;
    try {
      await apiPost("/api/management/personas/folders/delete", { id });
      if (selectedFolderId() === id) setSelectedFolderId(null);
      toastSuccess("已删除文件夹");
      await refetch();
    } catch (err) {
      toastError(err);
    }
    setContextMenu(null);
  };

  const moveFolder = async (id: string, parentId: string | null) => {
    if (id === parentId || isDescendant(folders(), id, parentId)) return;
    if (folderDepth(folders(), parentId) >= 3) {
      toastError("最多支持三层文件夹");
      return;
    }
    try {
      await apiPost("/api/management/personas/folders/move", { id, parent_id: parentId });
      await refetch();
    } catch (err) {
      toastError(err);
    }
  };

  const showContextMenu = (event: MouseEvent, target: ContextTarget) => {
    event.preventDefault();
    setContextMenu({ x: event.clientX, y: event.clientY, target });
  };

  return (
    <>
      <PageHeader
        title="人格"
        subtitle="System Prompt 模板"
        actions={
          <>
            <Button onClick={() => openFolderModal(null)}>新建文件夹</Button>
            <Button variant="primary" onClick={() => openEdit(null)}>新增人格</Button>
          </>
        }
      />
      <div class="persona-layout">
        <Card title="文件夹">
          <FolderTree
            folders={folders()}
            items={treeItems()}
            selectedFolderId={selectedFolderId()}
            selectedItemId={selectedItemId()}
            onSelectFolder={setSelectedFolderId}
            onSelectItem={setSelectedItemId}
            onMoveItem={movePersona}
            onMoveFolder={moveFolder}
            onContextMenu={showContextMenu}
          />
        </Card>
        <div>
          <Card
            title="列表"
            actions={
              <div class="breadcrumb">
                <button type="button" onClick={() => setSelectedFolderId(null)}>全部人格</button>
                <For each={breadcrumb()}>
                  {(folder) => (
                    <>
                      <span>/</span>
                      <button type="button" onClick={() => setSelectedFolderId(folder.id)}>{folder.name}</button>
                    </>
                  )}
                </For>
              </div>
            }
          >
            <Show when={!data.loading} fallback={<Loading />}>
              <Show when={visiblePersonas().length} fallback={<EmptyState />}>
                <table class="table">
                  <thead>
                    <tr><th>名称</th><th>提示词预览</th><th>操作</th></tr>
                  </thead>
                  <tbody>
                    <For each={visiblePersonas()}>
                      {(p) => (
                        <tr class={selectedItemId() === personaId(p) ? "table-row--active" : ""} onClick={() => personaId(p) && setSelectedItemId(personaId(p))}>
                          <td>{personaName(p)}</td>
                          <td class="muted persona-prompt-preview">{p.system_prompt ?? ""}</td>
                          <td class="row">
                            <Button size="sm" onClick={() => openEdit(p)}>编辑</Button>
                            <Button size="sm" variant="danger" onClick={() => personaId(p) && del(personaId(p))}>删除</Button>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </Show>
            </Show>
          </Card>
          <Card title="预览">
            <Show when={selectedPersona()} fallback={<EmptyState message="选择人格查看完整提示词" />}>
              {(persona) => (
                <div class="stack">
                  <div class="row">
                    <strong>{personaName(persona())}</strong>
                    <Show when={persona().tags?.length}>
                      <For each={persona().tags}>{(tag) => <span class="badge">{tag}</span>}</For>
                    </Show>
                  </div>
                  <pre class="code-block">{persona().system_prompt ?? ""}</pre>
                </div>
              )}
            </Show>
          </Card>
        </div>
      </div>
      <Show when={contextMenu()}>
        {(menu) => (
          <div class="context-menu" style={{ left: `${menu().x}px`, top: `${menu().y}px` }}>
            <button type="button" onClick={() => openFolderModal(null, menu().target?.type === "folder" ? menu().target?.id ?? null : selectedFolderId())}>
              新建文件夹
            </button>
            <Show when={menu().target?.type === "folder"}>
              <button
                type="button"
                onClick={() => {
                  const folder = folders().find((item) => item.id === menu().target?.id);
                  if (folder) openFolderModal(folder);
                }}
              >
                重命名
              </button>
              <button type="button" onClick={() => deleteFolder(menu().target?.id ?? "")}>删除</button>
            </Show>
            <Show when={menu().target?.type === "item"}>
              <button
                type="button"
                onClick={() => {
                  const persona = personas().find((item) => personaId(item) === menu().target?.id);
                  if (persona) openEdit(persona);
                }}
              >
                编辑人格
              </button>
            </Show>
          </div>
        )}
      </Show>
      <Modal
        open={folderOpen()}
        title={folderDraft().id ? "重命名文件夹" : "新建文件夹"}
        onClose={() => setFolderOpen(false)}
        actions={
          <>
            <Button onClick={() => setFolderOpen(false)}>取消</Button>
            <Button variant="primary" onClick={saveFolder}>保存</Button>
          </>
        }
      >
        <Field label="名称" hint="最多三层嵌套，支持拖拽移动。">
          <Input
            value={folderDraft().name}
            onInput={(e) => setFolderDraft({ ...folderDraft(), name: e.currentTarget.value })}
          />
        </Field>
      </Modal>
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
        <Field label="所属文件夹">
          <select
            class="input"
            value={draft().folder_id ?? ""}
            onChange={(e) => setDraft({ ...draft(), folder_id: e.currentTarget.value || null })}
          >
            <option value="">全部人格</option>
            <For each={folders()}>
              {(folder) => <option value={folder.id}>{`${"　".repeat(folderDepth(folders(), folder.parentId))}${folder.name}`}</option>}
            </For>
          </select>
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
