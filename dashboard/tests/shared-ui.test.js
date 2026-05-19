import assert from "node:assert/strict";
import test from "node:test";

import {
  cardGrid,
  confirmDialog,
  dataTable,
  folderBreadcrumb,
  folderCard,
  folderTree,
  form,
  markdownViewer,
  renderUiBaseShowcase,
  tabs,
  uiState,
  unsavedChangesDialog,
} from "../src/render/shared.js";

test("shared ui base renders dialog and unsaved-change semantics", () => {
  const confirm = confirmDialog({
    title: "Delete <plugin>",
    message: "Remove <script>alert(1)</script>?",
  });
  const unsaved = unsavedChangesDialog({
    hints: ["Confirm", "Cancel", "Close"],
  });

  assert.match(confirm, /role="dialog"/);
  assert.match(confirm, /data-dialog-value="confirm"/);
  assert.match(confirm, /data-dialog-value="cancel"/);
  assert.match(confirm, /Delete &lt;plugin&gt;/);
  assert.match(confirm, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
  assert.doesNotMatch(confirm, /<script>alert/);

  assert.match(unsaved, /data-persistent="true"/);
  assert.match(unsaved, /Unsaved changes/);
  assert.match(unsaved, /Discard changes/);
});

test("shared ui base covers tabs, server tables, cards, and folder controls", () => {
  const html = [
    tabs({
      id: "config-tabs",
      activeId: "normal",
      items: [
        { id: "normal", label: "Normal", body: "<p>Normal config</p>" },
        { id: "system", label: "System", body: "<p>System config</p>" },
      ],
    }),
    dataTable({
      server: true,
      columns: [{ key: "name", label: "Name" }, { key: "status", label: "Status" }],
      rows: [{ id: "row-1", name: "<b>Plugin</b>", status: "ready" }],
      pagination: { page: 2, pageSize: 25, total: 51 },
    }),
    cardGrid({
      cards: [{ title: "Card", subtitle: "Grid", body: "Reusable surface" }],
    }),
    folderBreadcrumb({ items: [{ id: "persona", label: "Persona" }] }),
    folderCard({ folder: { folder_id: "ops", name: "Ops", description: "Drop target" } }),
    folderTree({ folders: [{ folder_id: "ops", name: "Ops", children: [] }], currentId: "ops" }),
  ].join("");

  assert.match(html, /role="tablist"/);
  assert.match(html, /ui-window-item/);
  assert.match(html, /server-table/);
  assert.match(html, /Page 2/);
  assert.match(html, /&lt;b&gt;Plugin&lt;\/b&gt;/);
  assert.match(html, /ui-card-grid/);
  assert.match(html, /ui-folder-breadcrumb/);
  assert.match(html, /ui-folder-card/);
  assert.match(html, /ui-folder-tree/);
});

test("shared ui base renders form validation, file input, combobox, and JSON editor", () => {
  const html = form({
    title: "Provider",
    errors: ["API key is required"],
    fields: [
      { id: "name", label: "Name", required: true, error: "Missing" },
      { id: "type", label: "Type", type: "select", value: "mock", options: ["mock", "openai"] },
      { id: "models", label: "Models", type: "combobox", value: ["gpt"], options: ["gpt", "mini"] },
      { id: "enabled", label: "Enabled", type: "switch", value: true },
      { id: "config", label: "Config", type: "json", value: { timeout: 30 }, fullscreenAction: "open-config" },
      { id: "file", label: "File", type: "file", accept: ".json" },
    ],
  });

  assert.match(html, /ui-form-errors/);
  assert.match(html, /aria-invalid="true"/);
  assert.match(html, /role="combobox"/);
  assert.match(html, /ui-switch/);
  assert.match(html, /data-editor="monaco-fallback"/);
  assert.match(html, /ui-file-input/);
  assert.match(html, /API key is required/);
});

test("shared ui base renders markdown and state surfaces safely", () => {
  const html = [
    markdownViewer({ markdown: "# Title\n\nUse `code`.\n\n<script>alert(1)</script>" }),
    uiState({ state: "loading", title: "Loading" }),
    uiState({ state: "empty", title: "Empty" }),
    uiState({ state: "error", message: "Failed" }),
    renderUiBaseShowcase(),
  ].join("");

  assert.match(html, /markdown-body/);
  assert.match(html, /<code>code<\/code>/);
  assert.match(html, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
  assert.match(html, /ui-state loading/);
  assert.match(html, /ui-state empty/);
  assert.match(html, /ui-state error/);
  assert.match(html, /Shared UI Base/);
  assert.doesNotMatch(html, /<script>alert/);
});
