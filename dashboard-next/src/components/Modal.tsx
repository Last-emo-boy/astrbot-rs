import { Show, type Component, type JSX } from "solid-js";
import { Portal } from "solid-js/web";

interface ModalProps {
  open: boolean;
  title?: string;
  onClose: () => void;
  children: JSX.Element;
  actions?: JSX.Element;
  width?: string;
}

export const Modal: Component<ModalProps> = (props) => (
  <Show when={props.open}>
    <Portal>
      <div
        class="modal__backdrop"
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
      >
        <div class="modal__panel" style={props.width ? { "min-width": props.width } : undefined}>
          {props.title && <h2 class="modal__title">{props.title}</h2>}
          <div>{props.children}</div>
          {props.actions && <div class="modal__actions">{props.actions}</div>}
        </div>
      </div>
    </Portal>
  </Show>
);
