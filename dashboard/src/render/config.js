import { escapeHtml } from "../dom.js";
import { state } from "../state.js";
import {
  button,
  chip,
  dataTable,
  dialog,
  formField,
  jsonEditor,
  markdownViewer,
  tabs,
  uiState,
} from "./shared.js";

const DEFAULT_T2I_TEMPLATE = `<!doctype html>
<html>
<head>
  <meta charset="utf-8"/>
  <title>New Template</title>
</head>
<body>
  <article>{{ text | safe }}</article>
  <footer>{{ version }}</footer>
</body>
</html>
`;

export function renderConfig() {
  const mode = configMode();
  const abconfs = configInfoList();
  const selectedConfigId = selectedConfigIdForRender(abconfs);
  const selectedConfig = abconfs.find((item) => item.id === selectedConfigId) || { id: "default", name: "default" };
  const parsedConfig = parseConfigEditor();
  const schema = state.schema?.schema;
  const groups = filteredGroups(state.schema?.ui_metadata?.groups || [], state.configSearch || "");
  const t2iTemplates = t2iTemplateList();
  const selectedTemplate = selectedT2iTemplate(t2iTemplates);

  return `
    <div class="config-page" data-page="config" data-config-mode="${escapeHtml(mode)}">
      ${renderSourceConfigToolbar(mode, abconfs, selectedConfig)}
      ${state.configDirty ? renderUnsavedBanner() : ""}
      <div class="grid cols-2 config-main-grid">
        <section class="panel config-editor-panel">
          <div class="panel-title-row">
            <div>
              <h2>${mode === "system" ? "System Config" : "AstrBot Config"}</h2>
              <p class="empty">${escapeHtml(selectedConfig.name || selectedConfig.id)} (${escapeHtml(selectedConfig.id)})</p>
            </div>
            <div class="actions">
              ${button({ label: "Revert", action: "reload-config", variant: "ghost" })}
              ${button({ label: "Preview", action: "preview-config", variant: "secondary" })}
              ${button({ label: "Save", action: "apply-config" })}
            </div>
          </div>
          ${jsonEditor({
            id: "config-editor",
            value: state.configEditor || "{}",
            rows: 18,
            fullscreenAction: "config-editor-fullscreen",
          })}
          <div class="actions mt-16">
            ${button({ label: "Sync form to JSON", action: "sync-config-form", variant: "secondary" })}
            ${button({ label: "Test current config", action: "config-test-chat", variant: "ghost", disabled: mode === "system" })}
          </div>
        </section>
        <section class="config-form-shell">
          ${groups.length ? renderConfigSectionTabs(groups, parsedConfig, schema) : uiState({
            state: state.schema?.unavailable ? "error" : "empty",
            title: state.schema?.unavailable ? "Config schema unavailable" : "No matching config section",
            message: state.schema?.unavailable || "Clear the search keyword or reload schema metadata.",
          })}
        </section>
      </div>
      <div class="grid cols-2">
        ${renderAbconfManager(abconfs, selectedConfigId, mode)}
        ${renderUmoRouteManager()}
      </div>
      ${renderT2iTemplateEditor(t2iTemplates, selectedTemplate)}
      ${renderConfigFullscreenDialog()}
      ${renderT2iFullscreenDialog()}
      ${renderUnsavedDialog()}
      ${state.operation ? `<section class="panel"><h2>Last Operation</h2>${jsonEditor({ id: "config-operation-json", value: state.operation, rows: 8 })}</section>` : ""}
    </div>
  `;
}

function renderSourceConfigToolbar(mode, abconfs, selectedConfig) {
  return `
    <section class="panel config-toolbar-panel">
      <div class="config-source-toolbar">
        <div class="config-mode-switch" role="group" aria-label="Config type">
          ${button({ label: "Normal", action: "config-mode-normal", variant: mode === "normal" ? "primary" : "secondary", attrs: { "aria-pressed": mode === "normal" ? "true" : "false" } })}
          ${button({ label: "System", action: "config-mode-system", variant: mode === "system" ? "primary" : "secondary", attrs: { "aria-pressed": mode === "system" ? "true" : "false" } })}
        </div>
        <div class="form-row compact config-select-row">
          <label for="config-abconf-select">Config</label>
          <select id="config-abconf-select" ${mode === "system" ? "disabled" : ""}>
            ${abconfs.map((item) => `<option value="${escapeHtml(item.id)}" ${item.id === selectedConfig.id ? "selected" : ""}>${escapeHtml(item.name || item.id)}</option>`).join("")}
          </select>
        </div>
        ${button({ label: "Open", action: "config-abconf-open", variant: "secondary", disabled: mode === "system" })}
        ${button({ label: "Manage", action: "config-manage-open", variant: "ghost", disabled: mode === "system" })}
        <div class="form-row compact config-search-row">
          <label for="config-search">Search</label>
          <input id="config-search" value="${escapeHtml(state.configSearch || "")}" placeholder="field, hint, group" />
        </div>
        ${button({ label: "Search", action: "config-search-apply", variant: "ghost" })}
      </div>
    </section>
  `;
}

function renderUnsavedBanner() {
  return `
    <section class="panel config-unsaved-banner" role="status">
      <strong>Unsaved changes</strong>
      <span>Save, discard, or review edits before switching config contexts.</span>
    </section>
  `;
}

function renderConfigSectionTabs(groups, config, schema) {
  return tabs({
    id: "config-section-tabs",
    items: groups.map((group) => ({
      id: group.id,
      label: group.title || group.id,
      body: `
        <section class="panel config-section-panel">
          <div class="panel-title-row">
            <h2>${escapeHtml(group.title || group.id)}</h2>
            ${chip(group.id, "label")}
          </div>
          ${group.fields?.length
            ? group.fields.map((field) => renderConfigField(field, config, schema)).join("")
            : uiState({ state: "empty", title: "Empty section", compact: true })}
          <footer class="config-help-row">
            <a href="https://astrbot.app/" target="_blank" rel="noreferrer">Documentation</a>
            <span>Field controls are generated from runtime schema metadata.</span>
          </footer>
        </section>
      `,
    })),
  });
}

function renderConfigField(field, config, schema) {
  const fieldSchema = schemaField(schema, field.path);
  const value = valueAtPath(config, field.path);
  const effectiveValue = value === undefined ? fieldSchema?.default_value : value;
  const fieldId = `config-field-${field.path.replace(/[^a-z0-9_-]+/gi, "-")}`;
  const attrs = {
    "data-config-path": field.path,
    "data-config-control": field.control,
    "data-config-type": fieldSchema?.value_type || "",
  };
  const label = field.label || field.path;
  const hint = field.hint || fieldSchema?.value_type || "";

  if (field.condition && !conditionMatches(config, field.condition)) {
    return "";
  }

  if (field.path.includes("[]")) {
    return formField({
      id: fieldId,
      label,
      type: field.secret ? "password" : "text",
      value: "",
      hint: "Edit this value inside the list/object JSON field.",
      disabled: true,
    });
  }

  if (field._special === "t2i_template") {
    return `
      <div class="config-row-special">
        <div>
          <strong>${escapeHtml(label)}</strong>
          <p class="empty">${escapeHtml(hint || "Manage text-to-image HTML templates.")}</p>
        </div>
        ${button({ label: "Open T2I Template Editor", action: "config-scroll-t2i", variant: "secondary" })}
      </div>
    `;
  }

  if (field.control === "toggle" || fieldSchema?.value_type === "bool") {
    return formField({
      id: fieldId,
      label,
      type: "switch",
      value: Boolean(effectiveValue),
      hint,
      attrs,
    });
  }

  if (field.control === "number" || fieldSchema?.value_type === "integer") {
    return formField({
      id: fieldId,
      label,
      type: "number",
      value: effectiveValue ?? "",
      hint,
      attrs,
    });
  }

  if (field.control === "list" || field.control === "object" || fieldSchema?.value_type === "list" || fieldSchema?.value_type === "object") {
    return formField({
      id: fieldId,
      label,
      type: "json",
      value: effectiveValue ?? defaultComplexValue(field.control || fieldSchema?.value_type),
      hint,
      fullscreenAction: "config-editor-fullscreen",
      attrs,
    });
  }

  if (field.control === "file" || fieldSchema?.value_type === "file") {
    return formField({ id: fieldId, label, type: "file", value: "", hint, attrs });
  }

  if (field.options?.length) {
    return formField({
      id: fieldId,
      label,
      type: Array.isArray(effectiveValue) ? "combobox" : "select",
      value: effectiveValue ?? "",
      options: field.options,
      hint,
      attrs,
    });
  }

  return formField({
    id: fieldId,
    label,
    type: field.secret ? "password" : "text",
    value: effectiveValue ?? "",
    hint,
    attrs,
  });
}

function renderAbconfManager(abconfs, selectedConfigId, mode) {
  return `
    <section class="panel" id="config-manager">
      <div class="panel-title-row">
        <h2>ABConf</h2>
        ${button({ label: "Refresh", action: "reload-config", variant: "ghost" })}
      </div>
      ${dataTable({
        id: "abconf-table",
        server: true,
        rowKey: "id",
        emptyMessage: "No ABConf records.",
        columns: [
          { key: "name", label: "Name", render: (row) => `<strong>${escapeHtml(row.name || row.id)}</strong><br><span class="metric-label">${escapeHtml(row.id)}</span>`, html: true },
          { key: "state", label: "State", render: (row) => row.id === selectedConfigId ? chip("current", "ok") : chip(row.id === "default" ? "default" : "custom"), html: true },
          { key: "actions", label: "Actions", render: (row) => `
            <div class="button-cell">
              ${button({ label: "Open", action: "config-abconf-select", variant: "secondary", disabled: mode === "system", attrs: { "data-conf-id": row.id } })}
              ${button({ label: "Rename", action: "config-abconf-edit-fill", variant: "ghost", disabled: row.id === "default", attrs: { "data-conf-id": row.id, "data-conf-name": row.name || row.id } })}
              ${button({ label: "Delete", action: "config-abconf-delete", variant: "ghost", disabled: row.id === "default", attrs: { "data-conf-id": row.id } })}
            </div>
          `, html: true },
        ],
        rows: abconfs,
      })}
      <div class="form-grid cols-2 mt-16">
        <div class="form-row"><label for="config-new-name">New config name</label><input id="config-new-name" value="Ops" /></div>
        <div class="form-row"><label for="config-edit-id">Edit ID</label><input id="config-edit-id" value="${escapeHtml(state.configEditId || (selectedConfigId === "default" ? "" : selectedConfigId))}" /></div>
        <div class="form-row"><label for="config-edit-name">Edit name</label><input id="config-edit-name" value="${escapeHtml(state.configEditName || abconfs.find((item) => item.id === selectedConfigId)?.name || "")}" /></div>
      </div>
      <div class="actions mt-16">
        ${button({ label: "Create", action: "config-abconf-create" })}
        ${button({ label: "Update name", action: "config-abconf-rename", variant: "secondary" })}
      </div>
    </section>
  `;
}

function renderUmoRouteManager() {
  const routes = state.configRoutes?.routes || [];
  const routingMap = Object.fromEntries(routes.map((route) => [route.pattern, route.config_id]));
  return `
    <section class="panel">
      <div class="panel-title-row">
        <h2>UMOP Routes</h2>
        ${button({ label: "Reload", action: "load-config-routes", variant: "ghost" })}
      </div>
      ${state.configRoutes?.unavailable ? uiState({ state: "error", message: state.configRoutes.unavailable, compact: true }) : ""}
      ${dataTable({
        id: "config-routes-table",
        server: true,
        rowKey: "pattern",
        emptyMessage: "No UMO routes configured.",
        columns: [
          { key: "pattern", label: "UMO pattern", render: (row) => `<strong>${escapeHtml(row.pattern)}</strong>`, html: true },
          { key: "config_id", label: "Config ID" },
          { key: "actions", label: "Actions", render: (row) => button({ label: "Delete", action: "config-route-delete", variant: "ghost", attrs: { "data-pattern": row.pattern } }), html: true },
        ],
        rows: routes,
      })}
      <div class="form-grid cols-2 mt-16">
        <div class="form-row"><label for="config-route-pattern">Pattern</label><input id="config-route-pattern" value="webchat:group:room-*" /></div>
        <div class="form-row"><label for="config-route-config-id">Config ID</label><input id="config-route-config-id" value="${escapeHtml(state.selectedConfigId || "default")}" /></div>
        <div class="form-row"><label for="config-route-umo">Resolve UMO</label><input id="config-route-umo" value="webchat:group:room-alpha" /></div>
      </div>
      <div class="actions mt-16">
        ${button({ label: "Save route", action: "config-route-upsert" })}
        ${button({ label: "Resolve", action: "config-route-resolve", variant: "secondary" })}
      </div>
      ${jsonEditor({ id: "config-routes-json", value: routingMap, rows: 7 })}
      <div class="actions mt-16">
        ${button({ label: "Replace all routes", action: "config-route-replace", variant: "secondary" })}
      </div>
    </section>
  `;
}

function renderT2iTemplateEditor(templates, selectedTemplate) {
  const active = activeT2iTemplate();
  const content = state.t2iTemplateContent ?? "";
  return `
    <section class="panel t2i-template-editor" id="t2i-template-editor">
      <div class="panel-title-row">
        <div>
          <h2>T2I Template Editor</h2>
          <p class="empty">Manage source-compatible HTML templates and active template selection.</p>
        </div>
        ${chip(`active: ${active}`, "ok")}
      </div>
      <div class="config-source-toolbar">
        <div class="form-row compact">
          <label for="t2i-template-select">Template</label>
          <select id="t2i-template-select">
            ${templates.map((template) => `<option value="${escapeHtml(template.name)}" ${template.name === selectedTemplate ? "selected" : ""}>${escapeHtml(template.name)}${template.is_default ? " (default)" : ""}</option>`).join("")}
          </select>
        </div>
        <div class="form-row compact">
          <label for="t2i-template-new-name">New name</label>
          <input id="t2i-template-new-name" value="custom_template" />
        </div>
        ${button({ label: "Load", action: "t2i-template-load", variant: "secondary" })}
        ${button({ label: "New", action: "t2i-template-new", variant: "ghost" })}
        ${button({ label: "Save", action: "t2i-template-save" })}
        ${button({ label: "Apply", action: "t2i-template-apply", variant: "secondary", disabled: !selectedTemplate })}
        ${button({ label: "Delete", action: "t2i-template-delete", variant: "ghost", disabled: selectedTemplate === "base" || !selectedTemplate })}
        ${button({ label: "Reset base", action: "t2i-template-reset", variant: "ghost" })}
      </div>
      <div class="grid cols-2 mt-16">
        ${jsonEditor({ id: "t2i-template-content", value: content || DEFAULT_T2I_TEMPLATE, language: "html", rows: 18, fullscreenAction: "t2i-template-fullscreen" })}
        <section class="nested-panel">
          <div class="panel-title-row">
            <h2>Live Preview</h2>
            ${button({ label: "Refresh", action: "t2i-template-preview", variant: "ghost" })}
          </div>
          <iframe class="t2i-preview-frame" title="T2I template preview" sandbox="" srcdoc="${escapeHtml(renderTemplatePreview(content || DEFAULT_T2I_TEMPLATE))}"></iframe>
          ${markdownViewer({ markdown: "Use `{{ text | safe }}` and `{{ version }}` placeholders. Saving built-in templates writes a user override." })}
        </section>
      </div>
    </section>
  `;
}

function renderConfigFullscreenDialog() {
  return dialog({
    id: "config-editor-dialog",
    title: "JSON Editor",
    fullscreen: true,
    open: Boolean(state.configEditorFullscreen),
    body: jsonEditor({
      id: "config-editor-fullscreen-text",
      value: state.configEditor || "{}",
      rows: 28,
    }),
    actions: [
      { label: "Close", action: "config-editor-close", variant: "ghost" },
      { label: "Apply to form", action: "config-editor-apply" },
    ],
  });
}

function renderT2iFullscreenDialog() {
  return dialog({
    id: "t2i-template-dialog",
    title: "T2I Template Editor",
    fullscreen: true,
    open: Boolean(state.t2iTemplateFullscreen),
    body: jsonEditor({
      id: "t2i-template-fullscreen-text",
      value: state.t2iTemplateContent || DEFAULT_T2I_TEMPLATE,
      language: "html",
      rows: 28,
    }),
    actions: [
      { label: "Close", action: "t2i-template-close", variant: "ghost" },
      { label: "Apply to template", action: "t2i-template-apply-fullscreen" },
    ],
  });
}

function renderUnsavedDialog() {
  const prompt = state.configUnsavedPrompt;
  return dialog({
    id: "config-unsaved-dialog",
    title: "Unsaved changes",
    persistent: true,
    open: Boolean(prompt?.open),
    body: `
      <p class="ui-dialog-message strong">Save the current config before switching?</p>
      <div class="ui-hint-row">
        <span>Save writes the current JSON first.</span>
        <span>Discard switches without saving.</span>
        <span>Review keeps editing.</span>
      </div>
    `,
    actions: [
      { label: "Review edits", action: "config-unsaved-close", variant: "ghost" },
      { label: "Discard and switch", action: "config-unsaved-discard", variant: "danger" },
      { label: "Save and switch", action: "config-unsaved-save" },
    ],
  });
}

function configMode() {
  return state.configMode || (state.routeFragment === "system" ? "system" : "normal");
}

function selectedConfigIdForRender(abconfs) {
  const selected = state.selectedConfigId || "default";
  return abconfs.some((item) => item.id === selected) ? selected : abconfs[0]?.id || "default";
}

function configInfoList() {
  const info = state.configAbconfs?.info_list || [];
  if (info.length) return info;
  return [{ id: "default", name: "default" }];
}

function parseConfigEditor() {
  try {
    return state.configEditor ? JSON.parse(state.configEditor) : state.config || {};
  } catch {
    return state.config || {};
  }
}

function filteredGroups(groups, keyword) {
  const normalized = String(keyword || "").trim().toLowerCase();
  if (!normalized) return groups;
  return groups
    .map((group) => ({
      ...group,
      fields: (group.fields || []).filter((field) => {
        const text = [group.id, group.title, field.path, field.label, field.hint, field.control]
          .join(" ")
          .toLowerCase();
        return text.includes(normalized);
      }),
    }))
    .filter((group) => group.fields.length);
}

function schemaField(schema, path) {
  return (schema?.fields || []).find((field) => field.path === path) || null;
}

function valueAtPath(source, path) {
  if (!source || path.includes("[]")) return undefined;
  return path.split(".").reduce((cursor, key) => {
    if (cursor === null || typeof cursor !== "object") return undefined;
    return cursor[key];
  }, source);
}

function conditionMatches(source, condition) {
  return Object.entries(condition || {}).every(([path, expected]) => valueAtPath(source, path) === expected);
}

function defaultComplexValue(control) {
  return control === "list" ? [] : {};
}

function t2iTemplateList() {
  const data = state.t2iTemplates?.data || state.t2iTemplates?.templates || state.t2iTemplates || [];
  return Array.isArray(data) && data.length ? data : [{ name: "base", is_default: true }];
}

function activeT2iTemplate() {
  return state.t2iActiveTemplate?.data?.active_template
    || state.t2iActiveTemplate?.active_template
    || state.t2iActiveTemplate
    || "base";
}

function selectedT2iTemplate(templates) {
  const selected = state.t2iSelectedTemplate || activeT2iTemplate();
  return templates.some((template) => template.name === selected) ? selected : templates[0]?.name || "base";
}

function renderTemplatePreview(content) {
  return String(content || "")
    .replace(/\{\{\s*text\s*\|\s*safe\s*\}\}/g, "这是一个示例文本，用于预览模板效果。")
    .replace(/\{\{\s*text\s*\}\}/g, "这是一个示例文本，用于预览模板效果。")
    .replace(/\{\{\s*version\s*\}\}/g, state.t2iPreviewVersion || "v4.0.0");
}
