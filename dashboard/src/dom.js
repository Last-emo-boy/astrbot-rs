export const $ = (selector) => document.querySelector(selector);

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

export function jsonBlock(value) {
  return `<pre>${escapeHtml(JSON.stringify(value, null, 2))}</pre>`;
}

export function showToast(message, kind = "ok") {
  const toast = $("#toast");
  if (!toast) return;
  toast.hidden = false;
  toast.textContent = message;
  toast.className = kind === "error" ? "toast error" : kind === "warn" ? "toast warn" : "toast";
  clearTimeout(showToast.timer);
  showToast.timer = setTimeout(() => {
    toast.hidden = true;
  }, 3600);
}

export function setConnection(kind, text) {
  const node = $("#connection-state");
  if (!node) return;
  node.className = `status-pill ${kind}`;
  node.textContent = text;
}

export function openDialog(target) {
  const node = resolveElement(target);
  if (!node) return null;
  node.hidden = false;
  node.classList.add("open");
  return node;
}

export function closeDialog(target, value = "close") {
  const node = resolveElement(target);
  if (!node) return null;
  node.classList.remove("open");
  node.hidden = true;
  emitUiEvent(node, "ui-dialog-close", { value });
  return node;
}

export function waitForDialogChoice(target) {
  const node = resolveElement(target);
  if (!node) return Promise.resolve("missing");
  return new Promise((resolve) => {
    const handler = (event) => {
      node.removeEventListener("ui-dialog-close", handler);
      resolve(event.detail?.value ?? "close");
    };
    node.addEventListener("ui-dialog-close", handler);
  });
}

export async function confirmDialogChoice(target) {
  openDialog(target);
  return (await waitForDialogChoice(target)) === "confirm";
}

export function activateTab(tabButton) {
  const button = resolveElement(tabButton);
  const container = button?.closest("[data-tabs]");
  const targetSelector = button?.dataset?.tabTarget;
  const panel = targetSelector ? container?.querySelector(targetSelector) : null;
  if (!button || !container || !panel) return false;

  container.querySelectorAll('[role="tab"]').forEach((tab) => {
    const selected = tab === button;
    tab.classList.toggle("active", selected);
    tab.setAttribute("aria-selected", selected ? "true" : "false");
  });
  container.querySelectorAll('[role="tabpanel"]').forEach((item) => {
    item.hidden = item !== panel;
  });
  return true;
}

export function toggleMenu(toggleButton, force = null) {
  const button = resolveElement(toggleButton);
  const targetSelector = button?.dataset?.menuToggle;
  const menu = targetSelector ? document.querySelector(targetSelector) : null;
  if (!button || !menu) return false;
  const next = force === null ? menu.hidden : !force;
  menu.hidden = !next;
  button.setAttribute("aria-expanded", next ? "true" : "false");
  return true;
}

export function bindUiInteractions(root = document) {
  if (!root?.addEventListener) return () => {};
  const handler = async (event) => {
    const target = event.target;
    const dialogChoice = target.closest?.("[data-dialog-value]");
    if (dialogChoice) {
      const dialog = dialogChoice.closest("[data-dialog]");
      closeDialog(dialog, dialogChoice.dataset.dialogValue || "close");
      return;
    }

    const tab = target.closest?.("[data-tab-target]");
    if (tab && activateTab(tab)) return;

    const menuToggle = target.closest?.("[data-menu-toggle]");
    if (menuToggle && toggleMenu(menuToggle)) return;

    const copyButton = target.closest?.("[data-copy-target]");
    if (copyButton) {
      await copyCodeFromButton(copyButton);
    }
  };

  root.addEventListener("click", handler);
  return () => root.removeEventListener("click", handler);
}

function resolveElement(target) {
  if (!target) return null;
  if (typeof target !== "string") return target;
  if (target.startsWith("#") || target.startsWith(".")) {
    return document.querySelector(target);
  }
  return document.getElementById?.(target) || document.querySelector(target);
}

function emitUiEvent(node, name, detail) {
  if (typeof CustomEvent === "function") {
    node.dispatchEvent(new CustomEvent(name, { bubbles: true, detail }));
    return;
  }
  const event = document.createEvent?.("CustomEvent");
  if (event?.initCustomEvent) {
    event.initCustomEvent(name, true, false, detail);
    node.dispatchEvent(event);
  }
}

async function copyCodeFromButton(button) {
  const code = button.closest(".ui-code-block")?.querySelector("code") || button.parentElement?.nextElementSibling?.querySelector?.("code");
  if (!code) return false;
  const text = code.textContent || "";
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      fallbackCopy(text);
    }
    button.textContent = "Copied";
    setTimeout(() => {
      if (document.body.contains(button)) button.textContent = "Copy";
    }, 1600);
    return true;
  } catch {
    button.textContent = "Copy failed";
    return false;
  }
}

function fallbackCopy(text) {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
}
