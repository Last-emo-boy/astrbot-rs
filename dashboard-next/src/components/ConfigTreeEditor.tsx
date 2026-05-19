import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  type Component,
  type JSX,
} from "solid-js";
import { createStore, produce, unwrap, type SetStoreFunction } from "solid-js/store";
import { Button, Input } from "./Form";

/**
 * Generic config object — what the management API returns for config
 * sections. We treat it as a plain JSON-shape tree.
 */
export type ConfigValue =
  | string
  | number
  | boolean
  | null
  | ConfigValue[]
  | { [key: string]: ConfigValue };

export interface ConfigTreeEditorProps {
  /** The initial config object. The component holds its own working copy. */
  value: { [key: string]: ConfigValue };
  /** Called when the user clicks Save. Receives the working copy. */
  onSave?: (next: { [key: string]: ConfigValue }) => void | Promise<void>;
  /** Optional path → human label override (e.g. {"foo.bar": "Some label"}). */
  labels?: Record<string, string>;
  /** Optional path → hint text. */
  hints?: Record<string, string>;
  /** Disable interaction. */
  disabled?: boolean | undefined;
}

/**
 * Recursive config tree editor. Each leaf renders a typed input
 * (`string` → text, `number` → number, `boolean` → checkbox); arrays
 * render as a list with add/remove controls; objects nest. The component
 * tracks a dirty flag and offers Save / Revert.
 */
export const ConfigTreeEditor: Component<ConfigTreeEditorProps> = (props) => {
  const [working, setWorking] = createStore<{ [key: string]: ConfigValue }>(
    deepClone(props.value)
  );
  const [snapshot, setSnapshot] = createSignal<{ [key: string]: ConfigValue }>(
    deepClone(props.value)
  );
  const [saving, setSaving] = createSignal(false);
  const [pendingDirty, setPendingDirty] = createSignal(false);

  createEffect(() => {
    // If the parent passes in a brand-new value (object identity changed),
    // reset our working copy and snapshot.
    const incoming = props.value;
    setSnapshot(deepClone(incoming));
    setWorking(produce((draft) => {
      for (const key of Object.keys(draft)) {
        delete draft[key];
      }
      Object.assign(draft, deepClone(incoming));
    }));
    setPendingDirty(false);
  });

  const isDirty = createMemo(() => {
    return pendingDirty() || !shallowJsonEqual(working, snapshot());
  });

  const revert = () => {
    setWorking(produce((draft) => {
      for (const key of Object.keys(draft)) {
        delete draft[key];
      }
      Object.assign(draft, deepClone(snapshot()));
    }));
    setPendingDirty(false);
  };

  const save = async () => {
    if (!props.onSave || saving()) return;
    setSaving(true);
    try {
      const snapshotCopy = deepClone(unwrap(working));
      await props.onSave(snapshotCopy);
      setSnapshot(snapshotCopy);
      setPendingDirty(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class="config-tree">
      <ConfigBranch
        path=""
        value={working}
        setRoot={setWorking}
        labels={props.labels ?? {}}
        hints={props.hints ?? {}}
        disabled={props.disabled === true}
        markDirty={() => setPendingDirty(true)}
      />
      <div class="row" style={{ "margin-top": "12px", gap: "8px" }}>
        <Button
          variant="primary"
          disabled={!isDirty() || saving() || props.disabled === true}
          onClick={save}
        >
          {saving() ? "保存中…" : "保存"}
        </Button>
        <Button
          variant="ghost"
          disabled={!isDirty() || saving() || props.disabled === true}
          onClick={revert}
        >
          还原
        </Button>
        <Show when={isDirty()}>
          <span class="muted text-mono">未保存的修改</span>
        </Show>
      </div>
    </div>
  );
};

interface BranchProps {
  path: string;
  value: ConfigValue;
  setRoot: SetStoreFunction<{ [key: string]: ConfigValue }>;
  labels: Record<string, string>;
  hints: Record<string, string>;
  disabled: boolean;
  markDirty: () => void;
}

const ConfigBranch: Component<BranchProps> = (props): JSX.Element => {
  const value = () => props.value;
  return (
    <Show
      when={isObject(value())}
      fallback={
        <Show
          when={Array.isArray(value())}
          fallback={
            <ConfigLeaf
              path={props.path}
              value={value() as Exclude<ConfigValue, object>}
              setRoot={props.setRoot}
              labels={props.labels}
              hints={props.hints}
              disabled={props.disabled}
              markDirty={props.markDirty}
            />
          }
        >
          <ConfigArray
            path={props.path}
            value={value() as ConfigValue[]}
            setRoot={props.setRoot}
            labels={props.labels}
            hints={props.hints}
            disabled={props.disabled}
            markDirty={props.markDirty}
          />
        </Show>
      }
    >
      <ConfigObject
        path={props.path}
        value={value() as { [key: string]: ConfigValue }}
        setRoot={props.setRoot}
        labels={props.labels}
        hints={props.hints}
        disabled={props.disabled}
        markDirty={props.markDirty}
      />
    </Show>
  );
};

interface ObjectProps extends Omit<BranchProps, "value"> {
  value: { [key: string]: ConfigValue };
}

const ConfigObject: Component<ObjectProps> = (props) => {
  const keys = createMemo(() => Object.keys(props.value));
  return (
    <div class="config-object">
      <For each={keys()}>
        {(key) => {
          const childPath = props.path ? `${props.path}.${key}` : key;
          return (
            <div class="config-row">
              <div class="config-row__label">
                {props.labels[childPath] ?? key}
                <Show when={props.hints[childPath]}>
                  <div class="config-row__hint muted">
                    {props.hints[childPath]}
                  </div>
                </Show>
              </div>
              <div class="config-row__value">
                <ConfigBranch
                  path={childPath}
                  value={props.value[key] as ConfigValue}
                  setRoot={props.setRoot}
                  labels={props.labels}
                  hints={props.hints}
                  disabled={props.disabled}
                  markDirty={props.markDirty}
                />
              </div>
            </div>
          );
        }}
      </For>
    </div>
  );
};

interface ArrayProps extends Omit<BranchProps, "value"> {
  value: ConfigValue[];
}

const ConfigArray: Component<ArrayProps> = (props) => {
  const segments = () => splitPath(props.path);
  const addItem = () => {
    props.setRoot(
      produce((draft) => {
        const target = navigateMut(draft, segments()) as ConfigValue[] | undefined;
        if (Array.isArray(target)) {
          // Mirror the last element's shape so the new row makes sense.
          const last = target[target.length - 1];
          target.push(deepClone(last ?? ""));
        }
      })
    );
    props.markDirty();
  };
  const removeItem = (index: number) => {
    props.setRoot(
      produce((draft) => {
        const target = navigateMut(draft, segments()) as ConfigValue[] | undefined;
        if (Array.isArray(target)) {
          target.splice(index, 1);
        }
      })
    );
    props.markDirty();
  };
  return (
    <div class="config-array">
      <For each={props.value}>
        {(item, idx) => (
          <div class="config-array__row">
            <ConfigBranch
              path={`${props.path}[${idx()}]`}
              value={item}
              setRoot={props.setRoot}
              labels={props.labels}
              hints={props.hints}
              disabled={props.disabled}
              markDirty={props.markDirty}
            />
            <Button
              size="sm"
              variant="ghost"
              disabled={props.disabled}
              onClick={() => removeItem(idx())}
            >
              删除
            </Button>
          </div>
        )}
      </For>
      <Button
        size="sm"
        variant="ghost"
        disabled={props.disabled}
        onClick={addItem}
      >
        + 添加
      </Button>
    </div>
  );
};

interface LeafProps extends Omit<BranchProps, "value"> {
  value: string | number | boolean | null;
}

const ConfigLeaf: Component<LeafProps> = (props): JSX.Element => {
  const segments = createMemo(() => splitPath(props.path));
  const commit = (next: ConfigValue) => {
    props.setRoot(
      produce((draft) => {
        setByPath(draft, segments(), next);
      })
    );
    props.markDirty();
  };
  if (props.value === null) {
    return (
      <Input
        type="text"
        value=""
        placeholder="(null)"
        disabled={props.disabled}
        onInput={(event) => commit(event.currentTarget.value)}
      />
    );
  }
  if (typeof props.value === "boolean") {
    return (
      <label class="row">
        <input
          type="checkbox"
          checked={props.value}
          disabled={props.disabled}
          onChange={(event) => commit(event.currentTarget.checked)}
        />
        <span class="muted">{props.value ? "开启" : "关闭"}</span>
      </label>
    );
  }
  if (typeof props.value === "number") {
    return (
      <Input
        type="number"
        value={props.value}
        disabled={props.disabled}
        onInput={(event) => {
          const text = event.currentTarget.value;
          const parsed = text === "" ? 0 : Number(text);
          commit(Number.isFinite(parsed) ? parsed : 0);
        }}
      />
    );
  }
  return (
    <Input
      type="text"
      value={props.value}
      disabled={props.disabled}
      onInput={(event) => commit(event.currentTarget.value)}
    />
  );
};

// ===== Helpers =====

function deepClone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function isObject(value: unknown): value is { [key: string]: ConfigValue } {
  return (
    typeof value === "object" && value !== null && !Array.isArray(value)
  );
}

function shallowJsonEqual(a: unknown, b: unknown): boolean {
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}

interface PathSegment {
  kind: "key" | "index";
  value: string | number;
}

export function splitPath(path: string): PathSegment[] {
  if (!path) return [];
  const out: PathSegment[] = [];
  let buffer = "";
  let i = 0;
  while (i < path.length) {
    const ch = path[i];
    if (ch === "[") {
      if (buffer) {
        out.push({ kind: "key", value: buffer });
        buffer = "";
      }
      const close = path.indexOf("]", i + 1);
      if (close === -1) {
        // Malformed — bail.
        break;
      }
      const index = Number(path.slice(i + 1, close));
      out.push({ kind: "index", value: Number.isFinite(index) ? index : 0 });
      i = close + 1;
      if (path[i] === ".") i++;
      continue;
    }
    if (ch === ".") {
      if (buffer) {
        out.push({ kind: "key", value: buffer });
        buffer = "";
      }
      i++;
      continue;
    }
    buffer += ch;
    i++;
  }
  if (buffer) {
    out.push({ kind: "key", value: buffer });
  }
  return out;
}

function navigateMut(root: ConfigValue, segments: PathSegment[]): ConfigValue | undefined {
  let cursor: ConfigValue | undefined = root;
  for (const seg of segments) {
    if (cursor === null || cursor === undefined || typeof cursor !== "object") {
      return undefined;
    }
    if (seg.kind === "index") {
      if (!Array.isArray(cursor)) return undefined;
      cursor = cursor[seg.value as number];
    } else {
      cursor = (cursor as { [key: string]: ConfigValue })[seg.value as string];
    }
  }
  return cursor;
}

function setByPath(
  root: { [key: string]: ConfigValue },
  segments: PathSegment[],
  next: ConfigValue
): void {
  if (segments.length === 0) return;
  let cursor: ConfigValue | undefined = root;
  for (let i = 0; i < segments.length - 1; i++) {
    const seg = segments[i];
    if (!seg || cursor === undefined) return;
    if (seg.kind === "index") {
      if (!Array.isArray(cursor)) return;
      cursor = cursor[seg.value as number];
    } else {
      cursor = (cursor as { [key: string]: ConfigValue })[seg.value as string];
    }
  }
  const last = segments[segments.length - 1];
  if (!last || cursor === undefined) return;
  if (last.kind === "index" && Array.isArray(cursor)) {
    cursor[last.value as number] = next;
  } else if (
    last.kind === "key" &&
    cursor !== null &&
    typeof cursor === "object" &&
    !Array.isArray(cursor)
  ) {
    (cursor as { [key: string]: ConfigValue })[last.value as string] = next;
  }
}

export default ConfigTreeEditor;
