import { For, Show, type Component } from "solid-js";
import {
  DragDropProvider,
  DragDropSensors,
  closestCenter,
  createDraggable,
  createDroppable,
  type DragEvent as SolidDragEvent,
} from "@thisbeyond/solid-dnd";

export interface FolderNode {
  id: string;
  name: string;
  parentId: string | null;
}

export interface TreeItem {
  id: string;
  name: string;
  folderId: string | null;
}

interface FolderTreeProps {
  folders: FolderNode[];
  items: TreeItem[];
  selectedFolderId: string | null;
  selectedItemId: string | null;
  onSelectFolder: (id: string | null) => void;
  onSelectItem: (id: string) => void;
  onMoveItem: (itemId: string, folderId: string | null) => void;
  onMoveFolder: (folderId: string, parentId: string | null) => void;
  onContextMenu: (event: MouseEvent, target: { type: "folder" | "item"; id: string } | null) => void;
}

function childrenOf(folders: FolderNode[], parentId: string | null): FolderNode[] {
  return folders.filter((folder) => folder.parentId === parentId);
}

function itemsOf(items: TreeItem[], folderId: string | null): TreeItem[] {
  return items.filter((item) => item.folderId === folderId);
}

const folderDropId = (id: string | null): string => `folder:${id ?? "__root__"}`;
const personaDragId = (id: string): string => `persona:${id}`;

function parseFolderDropId(id: unknown): string | null | undefined {
  if (typeof id !== "string" || !id.startsWith("folder:")) return undefined;
  const value = id.slice("folder:".length);
  return value === "__root__" ? null : value;
}

function parsePersonaDragId(id: unknown): string | undefined {
  return typeof id === "string" && id.startsWith("persona:") ? id.slice("persona:".length) : undefined;
}

const TreeBranch: Component<{
  depth: number;
  parentId: string | null;
  props: FolderTreeProps;
}> = (branch) => {
  const folders = () => childrenOf(branch.props.folders, branch.parentId);
  const items = () => itemsOf(branch.props.items, branch.parentId);

  return (
    <div class="folder-tree__branch">
      <For each={folders()}>
        {(folder) => {
          const draggable = createDraggable(folderDropId(folder.id));
          const droppable = createDroppable(folderDropId(folder.id));
          return (
            <div>
              <button
              type="button"
              class={`folder-tree__node ${branch.props.selectedFolderId === folder.id ? "folder-tree__node--active" : ""}`}
              style={{ "padding-left": `${8 + branch.depth * 16}px` }}
              ref={(element) => {
                draggable.ref(element);
                droppable.ref(element);
              }}
              {...draggable.dragActivators}
              draggable
              onClick={() => branch.props.onSelectFolder(folder.id)}
              onContextMenu={(event) => {
                event.stopPropagation();
                branch.props.onContextMenu(event, { type: "folder", id: folder.id });
              }}
              onDragStart={(event) => event.dataTransfer?.setData("application/x-folder-id", folder.id)}
              onDragOver={(event) => {
                event.preventDefault();
                event.stopPropagation();
              }}
              onDrop={(event) => {
                event.preventDefault();
                event.stopPropagation();
                const itemId = event.dataTransfer?.getData("application/x-persona-id");
                const folderId = event.dataTransfer?.getData("application/x-folder-id");
                if (itemId) branch.props.onMoveItem(itemId, folder.id);
                if (folderId) branch.props.onMoveFolder(folderId, folder.id);
              }}
            >
              <span class="folder-tree__icon">▸</span>
              <span>{folder.name}</span>
            </button>
            <TreeBranch depth={branch.depth + 1} parentId={folder.id} props={branch.props} />
          </div>
          );
        }}
      </For>
      <For each={items()}>
        {(item) => {
          const draggable = createDraggable(personaDragId(item.id));
          return (
            <button
            type="button"
            class={`folder-tree__node folder-tree__node--item ${
              branch.props.selectedItemId === item.id ? "folder-tree__node--active" : ""
            }`}
            style={{ "padding-left": `${28 + branch.depth * 16}px` }}
            ref={draggable.ref}
            {...draggable.dragActivators}
            draggable
            onClick={() => branch.props.onSelectItem(item.id)}
            onContextMenu={(event) => {
              event.stopPropagation();
              branch.props.onContextMenu(event, { type: "item", id: item.id });
            }}
            onDragStart={(event) => event.dataTransfer?.setData("application/x-persona-id", item.id)}
          >
            <span>{item.name}</span>
          </button>
          );
        }}
      </For>
    </div>
  );
};

export const FolderTree: Component<FolderTreeProps> = (props) => {
  const rootDroppable = createDroppable(folderDropId(null));
  const onDragEnd = (event: SolidDragEvent) => {
    const targetFolderId = parseFolderDropId(event.droppable?.id);
    if (targetFolderId === undefined) return;

    const personaId = parsePersonaDragId(event.draggable.id);
    if (personaId) {
      props.onMoveItem(personaId, targetFolderId);
      return;
    }

    const folderId = parseFolderDropId(event.draggable.id);
    if (typeof folderId === "string" && folderId !== targetFolderId) {
      props.onMoveFolder(folderId, targetFolderId);
    }
  };

  return (
    <DragDropProvider collisionDetector={closestCenter} onDragEnd={onDragEnd}>
      <DragDropSensors />
        <div
          class="folder-tree"
          ref={rootDroppable.ref}
        onContextMenu={(event) => props.onContextMenu(event, null)}
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          const itemId = event.dataTransfer?.getData("application/x-persona-id");
          const folderId = event.dataTransfer?.getData("application/x-folder-id");
          if (itemId) props.onMoveItem(itemId, null);
          if (folderId) props.onMoveFolder(folderId, null);
        }}
      >
        <button
          type="button"
          class={`folder-tree__node ${props.selectedFolderId === null ? "folder-tree__node--active" : ""}`}
          onClick={() => props.onSelectFolder(null)}
        >
          <span class="folder-tree__icon">⌂</span>
          <span>全部人格</span>
        </button>
        <TreeBranch depth={0} parentId={null} props={props} />
        <Show when={!props.folders.length && !props.items.length}>
          <div class="empty-state">暂无人格</div>
        </Show>
      </div>
    </DragDropProvider>
  );
};
