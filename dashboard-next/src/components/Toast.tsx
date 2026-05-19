import { createSignal, For, type JSX } from "solid-js";

export type ToastKind = "info" | "success" | "danger" | "warning";

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

const [toasts, setToasts] = createSignal<Toast[]>([]);
let nextId = 1;

export function pushToast(message: string, kind: ToastKind = "info"): void {
  const id = nextId++;
  setToasts((list) => [...list, { id, message, kind }]);
  window.setTimeout(() => {
    setToasts((list) => list.filter((t) => t.id !== id));
  }, 4000);
}

export function toastSuccess(message: string): void {
  pushToast(message, "success");
}

export function toastError(err: unknown): void {
  const message =
    err instanceof Error ? err.message : typeof err === "string" ? err : "Unknown error";
  pushToast(message, "danger");
}

export function ToastHost(): JSX.Element {
  return (
    <div class="toast-host">
      <For each={toasts()}>
        {(t) => (
          <div class={`toast toast--${t.kind}`}>
            <div>{t.message}</div>
          </div>
        )}
      </For>
    </div>
  );
}
