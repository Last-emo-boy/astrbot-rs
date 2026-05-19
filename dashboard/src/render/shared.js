import { escapeHtml } from "../dom.js";

const STATE_COPY = {
  empty: ["Empty", "No records match the current filters."],
  loading: ["Loading", "Waiting for the runtime response."],
  error: ["Request failed", "The dashboard could not load this section."],
  success: ["Ready", "The requested state is available."],
};

export function metric(label, value, hint) {
  return `
    <section class="panel metric">
      <div class="metric-value">${escapeHtml(value)}</div>
      <div class="metric-label">${escapeHtml(label)}</div>
      <div class="tag">${escapeHtml(hint)}</div>
    </section>
  `;
}

export function statusItem(label, value) {
  return `<div class="status-item"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`;
}

export function pill(text, kind = "") {
  return `<span class="status-pill ${escapeHtml(kind)}">${escapeHtml(text)}</span>`;
}

export function closurePill(level) {
  const labels = {
    runtime: ["Runtime", "ok"],
    in_memory: ["In-memory", ""],
    plan_only: ["Plan-only", "warn"],
    unavailable: ["Unavailable", "error"],
  };
  const [label, kind] = labels[level] || [level, "warn"];
  return pill(label, kind);
}

export function classNames(...values) {
  return values
    .flatMap((value) => {
      if (!value) return [];
      if (Array.isArray(value)) return value;
      if (typeof value === "object") {
        return Object.entries(value)
          .filter(([, enabled]) => Boolean(enabled))
          .map(([name]) => name);
      }
      return String(value).split(/\s+/);
    })
    .map((value) => value.trim())
    .filter(Boolean)
    .join(" ");
}

export function button({
  label,
  action = "",
  variant = "primary",
  type = "button",
  disabled = false,
  icon = "",
  attrs = {},
} = {}) {
  const classes = classNames("button", variant !== "primary" && variant);
  return `<button class="${escapeHtml(classes)}" type="${escapeHtml(type)}"${action ? ` data-action="${escapeHtml(action)}"` : ""}${disabled ? " disabled" : ""}${renderAttrs(attrs)}>${icon ? `<span class="button-icon">${escapeHtml(icon)}</span>` : ""}${escapeHtml(label || "Button")}</button>`;
}

export function chip(text, kind = "", { removable = false, label = false, attrs = {} } = {}) {
  const classes = classNames("ui-chip", kind, { label, removable });
  return `<span class="${escapeHtml(classes)}"${renderAttrs(attrs)}>${escapeHtml(text)}${removable ? '<button type="button" class="ui-chip-remove" aria-label="Remove">x</button>' : ""}</span>`;
}

export function uiState({
  state = "empty",
  title,
  message,
  action = null,
  compact = false,
} = {}) {
  const [fallbackTitle, fallbackMessage] = STATE_COPY[state] || STATE_COPY.empty;
  const role = state === "error" ? "alert" : "status";
  const actionHtml = typeof action === "string" ? action : action?.label ? `<div class="ui-state-action">${button(action)}</div>` : "";
  return `
    <div class="${escapeHtml(classNames("ui-state", state, { compact }))}" role="${role}" aria-live="${state === "loading" ? "polite" : "off"}">
      <div class="ui-state-icon" aria-hidden="true">${state === "loading" ? "" : escapeHtml(state.slice(0, 1).toUpperCase())}</div>
      <div class="ui-state-copy">
        <strong>${escapeHtml(title || fallbackTitle)}</strong>
        <span>${escapeHtml(message || fallbackMessage)}</span>
      </div>
      ${actionHtml}
    </div>
  `;
}

export function dialog({
  id = "dialog",
  title = "Dialog",
  body = "",
  actions = [],
  open = false,
  persistent = false,
  fullscreen = false,
  maxWidth = "480px",
  closeLabel = "Close",
  closeAction = "",
  kind = "",
} = {}) {
  const dialogId = slug(id);
  const bodyHtml = Array.isArray(body) ? body.join("") : body;
  return `
    <div id="${escapeHtml(dialogId)}" class="${escapeHtml(classNames("ui-dialog-backdrop", { open }, kind))}" data-dialog data-persistent="${persistent ? "true" : "false"}" ${open ? "" : "hidden"}>
      <section class="${escapeHtml(classNames("ui-dialog", { fullscreen }))}" role="dialog" aria-modal="true" aria-labelledby="${escapeHtml(dialogId)}-title" style="--dialog-width: ${escapeHtml(maxWidth)}">
        <header class="ui-dialog-header">
          <h2 id="${escapeHtml(dialogId)}-title">${escapeHtml(title)}</h2>
          <button type="button" class="button icon ghost" data-dialog-value="close"${closeAction ? ` data-action="${escapeHtml(closeAction)}"` : ""} aria-label="${escapeHtml(closeLabel)}">x</button>
        </header>
        <div class="ui-dialog-body">${bodyHtml || ""}</div>
        ${actions.length ? `<footer class="ui-dialog-actions">${actions.map(renderDialogAction).join("")}</footer>` : ""}
      </section>
    </div>
  `;
}

export function confirmDialog({
  id = "confirm-dialog",
  title = "Confirm action",
  message = "This operation cannot be undone.",
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  open = true,
  danger = true,
} = {}) {
  return dialog({
    id,
    title,
    open,
    kind: danger ? "danger" : "",
    body: `<p class="ui-dialog-message">${escapeHtml(message)}</p>`,
    actions: [
      { label: cancelLabel, variant: "ghost", value: "cancel" },
      { label: confirmLabel, variant: danger ? "danger" : "primary", value: "confirm" },
    ],
  });
}

export function unsavedChangesDialog({
  id = "unsaved-changes-dialog",
  title = "Unsaved changes",
  message = "Leave this page and discard your current edits?",
  confirmLabel = "Discard changes",
  cancelLabel = "Stay",
  closeLabel = "Review edits",
  hints = ["Confirm discards the draft.", "Cancel keeps editing.", "Close returns to the form."],
  open = true,
} = {}) {
  return dialog({
    id,
    title,
    open,
    persistent: true,
    maxWidth: "520px",
    body: `
      <p class="ui-dialog-message strong">${escapeHtml(message)}</p>
      <div class="ui-hint-row">${hints.map((hint) => `<span>${escapeHtml(hint)}</span>`).join("")}</div>
    `,
    actions: [
      { label: cancelLabel, variant: "ghost", value: "cancel" },
      { label: closeLabel, variant: "secondary", value: "close" },
      { label: confirmLabel, variant: "danger", value: "confirm" },
    ],
  });
}

export function tabs({ id = "tabs", items = [], activeId = "" } = {}) {
  const rootId = slug(id);
  const active = activeId || items[0]?.id || "";
  return `
    <section class="ui-tabs" data-tabs="${escapeHtml(rootId)}">
      <div class="ui-tab-list" role="tablist">
        ${items.map((item) => {
          const itemId = slug(item.id || item.label);
          const selected = item.id === active || itemId === active;
          return `<button type="button" id="${escapeHtml(rootId)}-${escapeHtml(itemId)}-tab" class="${selected ? "active" : ""}" role="tab" aria-selected="${selected ? "true" : "false"}" aria-controls="${escapeHtml(rootId)}-${escapeHtml(itemId)}-panel" data-tab-target="#${escapeHtml(rootId)}-${escapeHtml(itemId)}-panel">${escapeHtml(item.label || item.id)}</button>`;
        }).join("")}
      </div>
      <div class="ui-window">
        ${items.map((item) => {
          const itemId = slug(item.id || item.label);
          const selected = item.id === active || itemId === active;
          return `<section id="${escapeHtml(rootId)}-${escapeHtml(itemId)}-panel" class="ui-window-item" role="tabpanel" aria-labelledby="${escapeHtml(rootId)}-${escapeHtml(itemId)}-tab" ${selected ? "" : "hidden"}>${item.body || ""}</section>`;
        }).join("")}
      </div>
    </section>
  `;
}

export function dataTable({
  id = "data-table",
  columns = [],
  rows = [],
  loading = false,
  error = "",
  emptyMessage = "No rows to display.",
  server = false,
  pagination = null,
  caption = "",
  rowKey = "id",
} = {}) {
  const tableId = slug(id);
  const stateHtml = loading
    ? uiState({ state: "loading", title: "Loading rows", compact: true })
    : error
      ? uiState({ state: "error", message: error, compact: true })
      : !rows.length
        ? uiState({ state: "empty", message: emptyMessage, compact: true })
        : "";

  return `
    <div class="${escapeHtml(classNames("ui-table-shell", { "server-table": server }))}" id="${escapeHtml(tableId)}">
      <div class="table-scroll">
        <table class="table ui-data-table">
          ${caption ? `<caption>${escapeHtml(caption)}</caption>` : ""}
          <thead><tr>${columns.map((column) => `<th scope="col">${escapeHtml(column.label || column.key)}</th>`).join("")}</tr></thead>
          <tbody>
            ${rows.map((row, index) => `<tr data-row-key="${escapeHtml(row[rowKey] ?? index)}">${columns.map((column) => `<td>${renderCell(column, row)}</td>`).join("")}</tr>`).join("")}
          </tbody>
        </table>
      </div>
      ${stateHtml}
      ${server || pagination ? renderTableFooter(pagination) : ""}
    </div>
  `;
}

export function cardGrid({ cards = [], columns = "auto" } = {}) {
  return `
    <div class="${escapeHtml(classNames("ui-card-grid", columns !== "auto" && `cols-${columns}`))}">
      ${cards.map(renderCard).join("")}
    </div>
  `;
}

export function form({
  id = "form",
  title = "",
  hint = "",
  fields = [],
  errors = [],
  actions = [],
} = {}) {
  return `
    <form id="${escapeHtml(slug(id))}" class="ui-form" novalidate>
      ${title || hint ? `<header class="ui-form-header">${title ? `<h2>${escapeHtml(title)}</h2>` : ""}${hint ? `<p>${escapeHtml(hint)}</p>` : ""}</header>` : ""}
      ${errors.length ? `<div class="ui-form-errors" role="alert"><strong>Validation failed</strong><ul>${errors.map((error) => `<li>${escapeHtml(error)}</li>`).join("")}</ul></div>` : ""}
      <div class="ui-form-grid">${fields.map(formField).join("")}</div>
      ${actions.length ? `<div class="actions">${actions.map((action) => typeof action === "string" ? action : button(action)).join("")}</div>` : ""}
    </form>
  `;
}

export function formField(field = {}) {
  const id = slug(field.id || field.name || field.label || "field");
  const type = field.type || "text";
  const invalid = Boolean(field.error);
  const describedBy = [
    field.hint ? `${id}-hint` : "",
    field.error ? `${id}-error` : "",
  ].filter(Boolean).join(" ");
  return `
    <div class="${escapeHtml(classNames("ui-form-field", type, { invalid, required: field.required }))}">
      ${type !== "checkbox" ? `<label for="${escapeHtml(id)}">${escapeHtml(field.label || field.name || "Field")}${field.required ? '<span aria-hidden="true">*</span>' : ""}</label>` : ""}
      ${renderFieldControl({ ...field, id, describedBy })}
      ${field.hint ? `<small id="${escapeHtml(id)}-hint" class="ui-field-hint">${escapeHtml(field.hint)}</small>` : ""}
      ${field.error ? `<small id="${escapeHtml(id)}-error" class="ui-field-error">${escapeHtml(field.error)}</small>` : ""}
    </div>
  `;
}

export function menu({ id = "menu", label = "Open menu", items = [], open = false } = {}) {
  const menuId = slug(id);
  return `
    <div class="ui-menu" data-menu="${escapeHtml(menuId)}">
      <button class="button ghost" type="button" aria-haspopup="menu" aria-expanded="${open ? "true" : "false"}" data-menu-toggle="#${escapeHtml(menuId)}-list">${escapeHtml(label)}</button>
      <div id="${escapeHtml(menuId)}-list" class="ui-menu-list" role="menu" ${open ? "" : "hidden"}>
        ${items.map((item) => `<button type="button" role="menuitem" class="${escapeHtml(classNames({ danger: item.danger }))}"${item.action ? ` data-action="${escapeHtml(item.action)}"` : ""}${item.disabled ? " disabled" : ""}>${item.icon ? `<span>${escapeHtml(item.icon)}</span>` : ""}${escapeHtml(item.label)}</button>`).join("")}
      </div>
    </div>
  `;
}

export function jsonEditor({
  id = "json-editor",
  value = "",
  language = "json",
  theme = "vs-light",
  rows = 8,
  fullscreenAction = "",
  attrs = {},
} = {}) {
  const editorId = slug(id);
  return `
    <div class="ui-code-editor" data-editor="monaco-fallback" data-language="${escapeHtml(language)}" data-theme="${escapeHtml(theme)}">
      <div class="ui-code-toolbar">
        <span>${escapeHtml(language.toUpperCase())}</span>
        ${fullscreenAction ? button({ label: "Fullscreen", action: fullscreenAction, variant: "ghost" }) : ""}
      </div>
      <textarea id="${escapeHtml(editorId)}" class="json-editor" spellcheck="false" rows="${escapeHtml(rows)}"${renderAttrs(attrs)}>${escapeHtml(formatEditorValue(value))}</textarea>
    </div>
  `;
}

export function codeBlock({ code = "", language = "", copyAction = "" } = {}) {
  const lang = language || "text";
  return `
    <figure class="code-block-wrapper ui-code-block">
      <figcaption>
        <span class="code-lang-label">${escapeHtml(lang)}</span>
        <button class="copy-code-btn" type="button"${copyAction ? ` data-action="${escapeHtml(copyAction)}"` : ""} data-copy-target="next">Copy</button>
      </figcaption>
      <pre class="hljs"><code class="language-${escapeHtml(lang)}">${escapeHtml(code)}</code></pre>
    </figure>
  `;
}

export function markdownViewer({ markdown = "", html = "", allowHtml = false, emptyMessage = "No markdown content." } = {}) {
  const content = allowHtml && html ? html : renderMarkdownLite(markdown || html);
  return `<article class="markdown-body">${content || `<p class="empty">${escapeHtml(emptyMessage)}</p>`}</article>`;
}

export function renderMarkdownLite(markdown = "") {
  const lines = String(markdown || "").split(/\r?\n/);
  const blocks = [];
  let inCode = false;
  let codeLang = "";
  let codeLines = [];
  let listLines = [];

  const flushList = () => {
    if (!listLines.length) return;
    blocks.push(`<ul>${listLines.map((item) => `<li>${renderInlineMarkdown(item)}</li>`).join("")}</ul>`);
    listLines = [];
  };

  for (const line of lines) {
    const fence = line.match(/^```([\w-]+)?\s*$/);
    if (fence) {
      if (inCode) {
        blocks.push(codeBlock({ code: codeLines.join("\n"), language: codeLang }));
        codeLines = [];
        codeLang = "";
        inCode = false;
      } else {
        flushList();
        inCode = true;
        codeLang = fence[1] || "text";
      }
      continue;
    }
    if (inCode) {
      codeLines.push(line);
      continue;
    }
    if (!line.trim()) {
      flushList();
      continue;
    }
    const heading = line.match(/^(#{1,6})\s+(.+)$/);
    if (heading) {
      flushList();
      const level = heading[1].length;
      blocks.push(`<h${level}>${renderInlineMarkdown(heading[2])}</h${level}>`);
      continue;
    }
    const list = line.match(/^\s*[-*]\s+(.+)$/);
    if (list) {
      listLines.push(list[1]);
      continue;
    }
    const quote = line.match(/^>\s+(.+)$/);
    if (quote) {
      flushList();
      blocks.push(`<blockquote>${renderInlineMarkdown(quote[1])}</blockquote>`);
      continue;
    }
    flushList();
    blocks.push(`<p>${renderInlineMarkdown(line)}</p>`);
  }

  flushList();
  if (inCode) {
    blocks.push(codeBlock({ code: codeLines.join("\n"), language: codeLang }));
  }
  return blocks.join("");
}

export function folderBreadcrumb({ items = [], rootLabel = "Root" } = {}) {
  const allItems = [{ id: "", label: rootLabel }, ...items];
  return `
    <nav class="ui-folder-breadcrumb" aria-label="Folder breadcrumb">
      ${allItems.map((item, index) => `<button type="button" data-folder="${escapeHtml(item.id || "")}" ${index === allItems.length - 1 ? 'aria-current="page"' : ""}>${escapeHtml(item.label || item.name || rootLabel)}</button>`).join('<span aria-hidden="true">/</span>')}
    </nav>
  `;
}

export function folderCard({ folder = {}, actions = [] } = {}) {
  return `
    <article class="ui-folder-card" draggable="true" data-folder="${escapeHtml(folder.folder_id || folder.id || "")}">
      <div class="ui-folder-icon" aria-hidden="true"></div>
      <div class="ui-folder-copy">
        <strong>${escapeHtml(folder.name || "Folder")}</strong>
        ${folder.description ? `<span>${escapeHtml(folder.description)}</span>` : ""}
      </div>
      ${actions.length ? menu({ id: `folder-${folder.folder_id || folder.id || "menu"}`, label: "Actions", items: actions }) : ""}
    </article>
  `;
}

export function folderTree({
  folders = [],
  currentId = "",
  expandedIds = [],
  loading = false,
  emptyMessage = "No folders.",
  rootLabel = "Root",
} = {}) {
  const expanded = new Set(expandedIds);
  if (loading) return `<div class="ui-folder-tree">${uiState({ state: "loading", title: "Loading folders", compact: true })}</div>`;
  return `
    <div class="ui-folder-tree">
      <button type="button" class="${escapeHtml(classNames("ui-tree-node", "root", { active: !currentId }))}" data-folder="">${escapeHtml(rootLabel)}</button>
      ${folders.length ? `<ul>${folders.map((folder) => renderFolderNode(folder, currentId, expanded, 0)).join("")}</ul>` : uiState({ state: "empty", message: emptyMessage, compact: true })}
    </div>
  `;
}

export function drawerToggle({ expanded = false, label = "Navigation" } = {}) {
  return `<button class="button ghost mobile-sidebar-toggle" type="button" aria-controls="nav" aria-expanded="${expanded ? "true" : "false"}" data-action="toggle-drawer"><span class="button-icon">=</span>${escapeHtml(label)}</button><div class="drawer-scrim" data-action="toggle-drawer" hidden></div>`;
}

export function renderUiBaseShowcase() {
  return `
    ${unsavedChangesDialog({
      message: "The config form has local edits.",
      hints: ["Discard resets the draft.", "Stay keeps the form open.", "Review returns to the editor."],
      open: true,
    })}
    <div class="dashboard-banner">
      <div>
        <div class="eyebrow">Shared UI Base</div>
        <h2>Vuetify parity primitives</h2>
        <p>Dialogs, forms, tabs, server tables, cards, markdown, code, and responsive shell controls.</p>
      </div>
      <div class="banner-actions">
        ${drawerToggle({ label: "Menu" })}
        ${button({ label: "Refresh", variant: "secondary", action: "refresh-fixture" })}
      </div>
    </div>
    <div class="grid cols-2">
      <section class="panel">
        <h2>Tabs and Window</h2>
        ${tabs({
          id: "fixture-tabs",
          activeId: "form",
          items: [
            { id: "form", label: "Form", body: form({
              id: "provider-form",
              title: "Provider form",
              hint: "Validation mirrors compact Vuetify fields.",
              errors: ["API key is required."],
              fields: [
                { id: "provider-name", label: "Name", required: true, value: "OpenAI" },
                { id: "provider-type", label: "Type", type: "select", value: "openai", options: [{ value: "openai", label: "OpenAI compatible" }, { value: "mock", label: "Mock" }] },
                { id: "provider-models", label: "Models", type: "combobox", value: ["gpt-4.1"], options: ["gpt-4.1", "gpt-4.1-mini"] },
                { id: "provider-file", label: "Credential file", type: "file", accept: ".json", hint: "Optional local fixture." },
                { id: "provider-enabled", label: "Enabled", type: "switch", value: true },
                { id: "provider-json", label: "Advanced JSON", type: "json", value: { timeout: 120 }, fullscreenAction: "open-json" },
              ],
              actions: [{ label: "Save", action: "save-fixture" }, { label: "Cancel", variant: "ghost" }],
            }) },
            { id: "readme", label: "Markdown", body: markdownViewer({ markdown: "# README\n\nUse `code` safely.\n\n```json\n{\"ok\": true}\n```" }) },
          ],
        })}
      </section>
      <section class="panel">
        <h2>Folder Controls</h2>
        ${folderBreadcrumb({ items: [{ id: "persona", label: "Persona" }, { id: "ops", label: "Ops" }] })}
        ${folderTree({
          currentId: "ops",
          expandedIds: ["persona"],
          folders: [{ folder_id: "persona", name: "Persona", children: [{ folder_id: "ops", name: "Ops", children: [] }] }],
        })}
        ${cardGrid({ cards: [
          { title: "Plugin card", subtitle: "Installed", body: "Card grid item with menu and chips.", chips: [chip("runtime", "ok"), chip("update", "warn")], actions: [button({ label: "Open", variant: "secondary" })] },
          { title: "Folder card", body: folderCard({ folder: { folder_id: "ops", name: "Ops", description: "Drop target" }, actions: [{ label: "Open" }, { label: "Delete", danger: true }] }) },
        ] })}
      </section>
    </div>
    <section class="panel">
      <h2>Data Table States</h2>
      ${dataTable({
        id: "providers",
        server: true,
        columns: [{ key: "name", label: "Name" }, { key: "status", label: "Status", html: true, render: (row) => chip(row.status, row.status === "ready" ? "ok" : "warn") }],
        rows: [{ id: "chat", name: "Chat provider", status: "ready" }],
        pagination: { page: 1, pageSize: 10, total: 1 },
      })}
      <div class="grid cols-3 mt-16">
        ${uiState({ state: "loading", title: "Loading state" })}
        ${uiState({ state: "empty", title: "Empty state" })}
        ${uiState({ state: "error", title: "Error state", message: "Backend returned 500." })}
      </div>
    </section>
  `;
}

function renderDialogAction(action) {
  if (typeof action === "string") return action;
  const classes = classNames("button", action.variant && action.variant !== "primary" && action.variant);
  return `<button type="button" class="${escapeHtml(classes)}"${action.value ? ` data-dialog-value="${escapeHtml(action.value)}"` : ""}${action.action ? ` data-action="${escapeHtml(action.action)}"` : ""}${action.disabled ? " disabled" : ""}>${escapeHtml(action.label || "Action")}</button>`;
}

function renderCard(card = {}) {
  return `
    <article class="${escapeHtml(classNames("ui-card", { selected: card.selected }))}" ${card.id ? `data-card="${escapeHtml(card.id)}"` : ""}>
      <header>
        <h3>${escapeHtml(card.title || "Card")}</h3>
        ${card.subtitle ? `<p>${escapeHtml(card.subtitle)}</p>` : ""}
      </header>
      ${card.body ? `<div class="ui-card-body">${card.body}</div>` : ""}
      ${card.chips?.length ? `<div class="ui-chip-row">${card.chips.join("")}</div>` : ""}
      ${card.actions?.length ? `<footer class="actions">${card.actions.join("")}</footer>` : ""}
    </article>
  `;
}

function renderCell(column, row) {
  const value = typeof column.render === "function" ? column.render(row) : row[column.key];
  return column.html ? String(value ?? "") : escapeHtml(value ?? "");
}

function renderTableFooter(pagination) {
  const page = pagination?.page ?? 1;
  const pageSize = pagination?.pageSize ?? pagination?.page_size ?? 25;
  const total = pagination?.total ?? 0;
  return `<footer class="ui-table-footer"><span>Page ${escapeHtml(page)}</span><span>${escapeHtml(pageSize)} per page</span><strong>${escapeHtml(total)} total</strong></footer>`;
}

function renderFieldControl(field) {
  const id = field.id;
  const value = field.value ?? "";
  const baseAttrs = {
    ...(field.attrs || {}),
    id,
    name: field.name || id,
    "aria-invalid": field.error ? "true" : "false",
    "aria-describedby": field.describedBy || null,
    placeholder: field.placeholder || null,
    disabled: field.disabled || null,
    readonly: field.readonly || null,
    required: field.required || null,
  };

  if (field.type === "textarea") {
    return `<textarea${renderAttrs(baseAttrs)} rows="${escapeHtml(field.rows || 3)}">${escapeHtml(value)}</textarea>`;
  }
  if (field.type === "select") {
    return `<select${renderAttrs({ ...baseAttrs, multiple: field.multiple || null })}>${field.placeholder ? `<option value="">${escapeHtml(field.placeholder)}</option>` : ""}${renderOptions(field.options, value)}</select>`;
  }
  if (field.type === "autocomplete" || field.type === "combobox") {
    const listId = `${id}-options`;
    const current = Array.isArray(value) ? value.join(", ") : value;
    return `<input${renderAttrs({ ...baseAttrs, value: current, list: listId, role: "combobox", "aria-autocomplete": "list" })} /><datalist id="${escapeHtml(listId)}">${renderOptions(field.options, "")}</datalist>${Array.isArray(value) ? `<div class="ui-chip-row">${value.map((item) => chip(item)).join("")}</div>` : ""}`;
  }
  if (field.type === "file") {
    return `<label class="ui-file-input"><input${renderAttrs({ ...baseAttrs, type: "file", accept: field.accept || null, multiple: field.multiple || null })} /><span class="ui-file-input-label">${escapeHtml(field.fileLabel || "Choose file")}</span>${field.accept ? `<span class="ui-file-input-meta">${escapeHtml(field.accept)}</span>` : ""}</label>`;
  }
  if (field.type === "checkbox") {
    return `<label class="ui-checkbox"><input${renderAttrs({ ...baseAttrs, type: "checkbox", checked: Boolean(value) || null })} /><span>${escapeHtml(field.checkboxLabel || field.label || "Enabled")}</span></label>`;
  }
  if (field.type === "switch") {
    return `<label class="ui-switch"><input${renderAttrs({ ...baseAttrs, type: "checkbox", checked: Boolean(value) || null })} /><span class="ui-switch-track"><span class="ui-switch-thumb"></span></span></label>`;
  }
  if (field.type === "json") {
    return jsonEditor({ id, value, language: field.language || "json", theme: field.theme || "vs-light", fullscreenAction: field.fullscreenAction || "", attrs: field.attrs || {} });
  }
  if (field.type === "markdown") {
    return `<textarea${renderAttrs(baseAttrs)} class="ui-markdown-source" rows="${escapeHtml(field.rows || 6)}">${escapeHtml(value)}</textarea>${markdownViewer({ markdown: value })}`;
  }
  return `<input${renderAttrs({ ...baseAttrs, type: field.type || "text", value })} />`;
}

function renderOptions(options = [], value) {
  const selectedValues = new Set(Array.isArray(value) ? value.map(String) : [String(value ?? "")]);
  return options.map(normalizeOption).map((option) => `<option value="${escapeHtml(option.value)}"${selectedValues.has(String(option.value)) ? " selected" : ""}>${escapeHtml(option.label)}</option>`).join("");
}

function normalizeOption(option) {
  if (option && typeof option === "object") {
    return {
      value: option.value ?? option.id ?? option.key ?? option.label ?? option.title ?? "",
      label: option.label ?? option.title ?? option.value ?? option.id ?? option.key ?? "",
    };
  }
  return { value: option ?? "", label: option ?? "" };
}

function renderFolderNode(folder, currentId, expanded, depth) {
  const id = folder.folder_id || folder.id || "";
  const children = folder.children || [];
  const isExpanded = expanded.has(id);
  return `
    <li>
      <button type="button" class="${escapeHtml(classNames("ui-tree-node", { active: currentId === id }))}" data-folder="${escapeHtml(id)}" style="--tree-depth: ${escapeHtml(depth)}">
        <span class="ui-tree-expander">${children.length ? (isExpanded ? "-" : "+") : ""}</span>
        <span>${escapeHtml(folder.name || id || "Folder")}</span>
      </button>
      ${children.length ? `<ul ${isExpanded ? "" : "hidden"}>${children.map((child) => renderFolderNode(child, currentId, expanded, depth + 1)).join("")}</ul>` : ""}
    </li>
  `;
}

function renderInlineMarkdown(value) {
  return escapeHtml(value)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
}

function formatEditorValue(value) {
  if (typeof value === "string") return value;
  return JSON.stringify(value ?? {}, null, 2);
}

function slug(value) {
  return String(value || "item")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "") || "item";
}

function renderAttrs(attrs = {}) {
  return Object.entries(attrs)
    .filter(([, value]) => value !== null && value !== undefined && value !== false)
    .map(([name, value]) => {
      if (!/^[:A-Za-z_][-\w:.]*$/.test(name)) return "";
      if (value === true) return ` ${name}`;
      return ` ${name}="${escapeHtml(value)}"`;
    })
    .join("");
}
