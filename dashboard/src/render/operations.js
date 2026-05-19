import { escapeHtml, jsonBlock } from "../dom.js";
import { state } from "../state.js";
import { chip, dialog, metric, pill, uiState } from "./shared.js";

export function renderConsole() {
  const entries = filteredConsoleEntries();
  const levels = consoleLevels();
  const streamState = state.logStreamStatus || "SSE 未连接";
  return `
    <div class="console-page" data-page="console">
      <section class="panel console-hero-panel">
        <div class="panel-title-row console-title-row">
          <div>
            <h2>平台日志</h2>
            <p class="metric-label">Debug 日志需要在「配置文件 -> 系统 -> 控制台日志级别」中开启。</p>
          </div>
          <div class="actions">
            <label class="check-row console-autoscroll">
              <input id="console-autoscroll" type="checkbox" data-action="console-autoscroll-toggle" ${state.consoleAutoScroll === false ? "" : "checked"} />
              ${state.consoleAutoScroll === false ? "自动滚动已关闭" : "自动滚动已开启"}
            </label>
            <button class="button secondary" type="button" data-action="console-pip-open">安装 pip 库</button>
            <button class="button secondary" type="button" data-action="logs-stream-start">连接 SSE</button>
            <button class="button ghost" type="button" data-action="logs-stream-stop">停止</button>
            <button class="button ghost" type="button" data-action="load-logs">刷新</button>
          </div>
        </div>
        <div class="console-filter-row">
          <div class="console-levels" role="group" aria-label="Log levels">
            ${CONSOLE_LEVELS.map((level) => `<button class="console-level-chip ${levels.has(level) ? "active" : ""} ${logKind(level)}" type="button" data-action="console-level-toggle" data-level="${escapeHtml(level)}">${escapeHtml(level)}</button>`).join("")}
          </div>
          <div class="console-search">
            <input id="console-search" value="${escapeHtml(state.consoleSearch || "")}" placeholder="搜索日志内容 / source / target" />
            <button class="button secondary" type="button" data-action="console-filter">搜索</button>
          </div>
        </div>
        <p class="metric-label" id="console-stream-state">${escapeHtml(streamState)}</p>
        ${state.logs?.unavailable ? `<p class="empty">${escapeHtml(state.logs.unavailable)}</p>` : ""}
      </section>

      <section class="console-terminal-panel">
        <div id="console-terminal" class="console-terminal" data-auto-scroll="${state.consoleAutoScroll === false ? "false" : "true"}">
          ${entries.length ? entries.map(renderConsoleLogLine).join("") : `<p class="empty">暂无日志。</p>`}
        </div>
      </section>
      ${renderConsolePipDialog()}
    </div>
  `;
}

export function logKind(level) {
  const normalized = normalizeLogLevel(level);
  if (normalized === "ERROR" || normalized === "CRITICAL") return "error";
  if (normalized === "WARNING") return "warn";
  if (normalized === "INFO") return "ok";
  return "";
}

export function formatSource(source) {
  if (typeof source === "string") return source;
  return Object.keys(source || {})[0] || "unknown";
}

export function renderTrace() {
  const settings = state.traceSettings || state.trace?.settings || {};
  const groups = traceGroups();
  const enabled = settings.trace_enable ?? settings.enabled ?? true;
  return `
    <div class="trace-page" data-page="trace">
      <section class="panel trace-hero-panel">
        <div class="panel-title-row trace-title-row">
          <div>
            <h2>追踪</h2>
            <p class="metric-label">当前记录主 Agent 的模型调用路径、消息 outline 和关键字段。</p>
          </div>
          <div class="actions">
            <label class="check-row">
              <input id="trace-enabled" type="checkbox" ${enabled ? "checked" : ""} />
              ${enabled ? "记录中" : "已暂停"}
            </label>
            <button class="button secondary" type="button" data-action="trace-settings-save">保存设置</button>
            <button class="button ghost" type="button" data-action="load-trace">刷新</button>
          </div>
        </div>
        <div class="form-grid cols-3 trace-settings-grid">
          <label class="check-row"><input id="trace-outline" type="checkbox" ${settings.capture_message_outline === false ? "" : "checked"} /> 记录消息 outline</label>
          <div class="form-row"><label>Max events</label><input id="trace-max-events" type="number" min="1" value="${settings.max_events || 500}" /></div>
          <div class="form-row"><label>Redact fields</label><input id="trace-redact-fields" value="${escapeHtml((settings.redact_fields || []).join(", "))}" placeholder="api_key, authorization" /></div>
        </div>
        ${state.trace?.unavailable ? `<p class="empty">${escapeHtml(state.trace.unavailable)}</p>` : ""}
      </section>
      <section class="panel trace-table-panel">
        ${groups.length ? `
          <div class="trace-table">
            <div class="trace-row trace-header">
              <div>Time</div>
              <div>Event ID</div>
              <div>UMO</div>
              <div>Sender</div>
              <div>Outline</div>
              <div></div>
            </div>
            ${groups.map(renderTraceGroup).join("")}
          </div>
        ` : uiState({ state: "empty", title: "No trace data yet.", message: "新的 trace 事件会通过 /api/log-history 与 /api/management/trace 展示。" })}
      </section>
    </div>
  `;
}

const CONSOLE_LEVELS = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"];

function consoleLevels() {
  const selected = Array.isArray(state.consoleLevels) && state.consoleLevels.length
    ? state.consoleLevels
    : CONSOLE_LEVELS;
  return new Set(selected.map(normalizeLogLevel));
}

function filteredConsoleEntries() {
  const levels = consoleLevels();
  const search = String(state.consoleSearch || "").trim().toLowerCase();
  return consoleLogEntries().filter((entry) => {
    if (!levels.has(entry.level)) return false;
    if (!search) return true;
    return [entry.data, entry.source, entry.target, entry.level]
      .some((value) => String(value || "").toLowerCase().includes(search));
  });
}

function consoleLogEntries() {
  const sourceRows = state.logs?.source_logs;
  if (Array.isArray(sourceRows) && sourceRows.length) {
    return sourceRows.map(normalizeConsoleLogEntry);
  }
  return (state.logs?.snapshot?.entries || []).map((entry) => normalizeConsoleLogEntry({
    ...entry,
    data: entry.message,
    time: entry.occurred_at_unix || entry.time,
  }));
}

function normalizeConsoleLogEntry(entry = {}) {
  return {
    id: entry.id ?? "",
    level: normalizeLogLevel(entry.level),
    data: String(entry.data ?? entry.message ?? ""),
    source: formatSource(entry.source),
    target: entry.target || "",
    time: entry.time || entry.occurred_at_unix || "",
  };
}

function normalizeLogLevel(level) {
  const value = String(level || "INFO").toUpperCase();
  if (value === "WARN") return "WARNING";
  if (value === "ERROR") return "ERROR";
  if (value === "CRITICAL") return "CRITICAL";
  if (value === "DEBUG") return "DEBUG";
  if (value === "TRACE") return "DEBUG";
  return "INFO";
}

export function renderConsoleLogLine(entry = {}) {
  const normalized = normalizeConsoleLogEntry(entry);
  return `
    <pre class="console-log-line ${logKind(normalized.level)}">
      <span class="console-log-meta">[${escapeHtml(normalized.level)}] ${escapeHtml(formatEventTime(normalized.time))} ${escapeHtml(normalized.source)}${normalized.target ? ` -> ${escapeHtml(normalized.target)}` : ""}</span>
      <span>${escapeHtml(normalized.data)}</span>
    </pre>
  `;
}

function renderConsolePipDialog() {
  return dialog({
    id: "console-pip-dialog",
    title: "安装 Pip 库",
    open: state.consolePipDialog === "install",
    maxWidth: "460px",
    closeAction: "console-pip-close",
    body: `
      <div class="form-row"><label>*库名，如 llmtuner</label><input id="console-pip-package" value="${escapeHtml(state.consolePipPackage || "")}" /></div>
      <div class="form-row"><label>强制 PyPI 软件仓库链接（可选）</label><input id="console-pip-mirror" value="${escapeHtml(state.consolePipMirror || "")}" /></div>
      <p class="metric-label">强制 PyPI 软件仓库链接 > 配置项 PyPI 软件仓库地址</p>
    `,
    actions: [
      { label: "取消", variant: "ghost", action: "console-pip-close" },
      { label: "安装", action: "console-pip-install" },
    ],
  });
}

function traceGroups() {
  const rows = traceRows();
  const groups = new Map();
  for (const row of rows) {
    const spanId = row.span_id || "unknown";
    if (!groups.has(spanId)) {
      groups.set(spanId, {
        span_id: spanId,
        name: row.name || row.span_name || spanId,
        umo: row.umo || row.message_origin || "",
        sender_name: row.sender_name || "",
        message_outline: row.message_outline || "",
        first_time: row.time || row.occurred_at_unix || "",
        records: [],
      });
    }
    const group = groups.get(spanId);
    group.records.push(row);
    if (!group.message_outline && row.message_outline) group.message_outline = row.message_outline;
    if (!group.sender_name && row.sender_name) group.sender_name = row.sender_name;
    if (!group.umo && (row.umo || row.message_origin)) group.umo = row.umo || row.message_origin;
  }
  return [...groups.values()].sort((left, right) => Number(right.first_time || 0) - Number(left.first_time || 0));
}

function traceRows() {
  const sourceRows = state.trace?.source_events;
  if (Array.isArray(sourceRows) && sourceRows.length) {
    return sourceRows.map(normalizeTraceRow);
  }
  return (state.trace?.events || []).map(normalizeTraceRow);
}

function normalizeTraceRow(event = {}) {
  const fields = Array.isArray(event.fields)
    ? Object.fromEntries(event.fields.map(([key, value]) => [key, value]))
    : event.fields || {};
  return {
    span_id: event.span_id || "",
    name: event.name || event.span_name || "",
    span_name: event.span_name || event.name || "",
    action: event.action || "",
    umo: event.umo || event.message_origin || "",
    message_origin: event.message_origin || event.umo || "",
    sender_name: event.sender_name || "",
    message_outline: event.message_outline || "",
    fields,
    time: event.time || event.occurred_at_unix || event.occurred_at || "",
  };
}

function renderTraceGroup(group) {
  const expanded = state.traceExpanded?.[group.span_id] === true;
  return `
    <div class="trace-group ${expanded ? "expanded" : ""}">
      <div class="trace-row trace-event">
        <div>${escapeHtml(formatEventTime(group.first_time))}</div>
        <div><strong title="${escapeHtml(group.span_id)}">${escapeHtml(shortSpan(group.span_id))}</strong><br><span class="metric-label">${escapeHtml(group.name)}</span></div>
        <div>${escapeHtml(group.umo || "-")}</div>
        <div>${escapeHtml(group.sender_name || "-")}</div>
        <div>${escapeHtml(group.message_outline || "-")}</div>
        <div class="trace-controls">
          <button class="button ghost" type="button" data-action="trace-toggle-event" data-span="${escapeHtml(group.span_id)}">${expanded ? "Collapse" : "Expand"}</button>
        </div>
      </div>
      ${expanded ? `
        <div class="trace-records">
          ${group.records.map(renderTraceRecord).join("")}
        </div>
      ` : ""}
    </div>
  `;
}

function renderTraceRecord(record) {
  return `
    <div class="trace-record">
      <div class="trace-record-time">${escapeHtml(formatEventTime(record.time))}</div>
      <div class="trace-record-action">${escapeHtml(record.action || "-")}</div>
      <pre class="trace-record-fields">${escapeHtml(JSON.stringify(record.fields || {}, null, 2))}</pre>
    </div>
  `;
}

function shortSpan(spanId) {
  return String(spanId || "").slice(0, 8) || "-";
}

function formatEventTime(value) {
  if (!value) return "-";
  if (typeof value === "number") return new Date(value * 1000).toLocaleString();
  if (typeof value === "object" && value.secs_since_epoch) {
    return new Date(Number(value.secs_since_epoch) * 1000).toLocaleString();
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? String(value) : date.toLocaleString();
}

export function renderPersonas() {
  const catalog = personaCatalog();
  const currentFolderId = normalizeFolderId(state.personaFolderId);
  const currentFolders = childFolders(catalog.folders, currentFolderId);
  const currentPersonas = catalog.personas
    .filter((profile) => profile.folder_id === currentFolderId)
    .filter((profile) => personaMatches(profile, state.personaSearch));
  if (!state.personas) {
    return `<section class="panel persona-page">${uiState({ state: "loading", title: "正在加载 Persona" })}</section>`;
  }
  return `
    <section class="persona-page" data-page="personas">
      ${state.personas?.unavailable ? `<p class="empty">${escapeHtml(state.personas.unavailable)}</p>` : ""}
      <div class="persona-mobile-nav">
        ${renderPersonaBreadcrumb(currentFolderId, catalog.folders)}
      </div>
      <div class="persona-manager-layout">
        <aside class="panel persona-folder-sidebar">
          <div class="panel-title-row">
            <h2>文件夹</h2>
            <button class="button icon secondary" type="button" data-action="persona-folder-create-open" aria-label="新建文件夹">+</button>
          </div>
          <div class="form-row">
            <label>搜索文件夹 / Persona</label>
            <input id="persona-search-input" class="persona-search-field" value="${escapeHtml(state.personaSearch)}" placeholder="ID / prompt / tool / skill" />
          </div>
          <div class="actions mb-12">
            <button class="button secondary" type="button" data-action="persona-search">搜索</button>
            <button class="button ghost" type="button" data-action="load-personas">刷新</button>
          </div>
          ${renderPersonaFolderTree(catalog.tree, currentFolderId)}
        </aside>
        <main class="persona-main">
          <div class="persona-toolbar">
            <div class="persona-desktop-breadcrumb">
              ${renderPersonaBreadcrumb(currentFolderId, catalog.folders)}
            </div>
            <div class="persona-main-search">
              <input id="persona-search-input-main" class="persona-search-field" value="${escapeHtml(state.personaSearch)}" placeholder="ID / prompt / tool / skill" />
              <button class="button secondary" type="button" data-action="persona-search">搜索</button>
            </div>
            <div class="actions">
              <button class="button" type="button" data-action="persona-create-open">创建 Persona</button>
              <button class="button secondary" type="button" data-action="persona-folder-create-open">新建文件夹</button>
            </div>
          </div>
          <section class="panel persona-content-panel">
            <div class="panel-title-row">
              <h2>${escapeHtml(currentFolderName(currentFolderId, catalog.folders))}</h2>
              <span class="metric-label">${currentFolders.length} folders · ${currentPersonas.length} personas</span>
            </div>
            ${currentFolders.length ? `
              <div class="persona-section-title">子文件夹 (${currentFolders.length})</div>
              <div class="persona-card-grid folder-grid">
                ${currentFolders.map((folder, index) => renderFolderCard(folder, index, currentFolders.length, catalog.folders)).join("")}
              </div>
            ` : ""}
            ${currentPersonas.length ? `
              <div class="persona-section-title">Persona (${currentPersonas.length})</div>
              <div class="persona-card-grid">
                ${currentPersonas.map((profile, index) => renderPersonaCard(profile, index, currentPersonas.length, catalog.folders)).join("")}
              </div>
            ` : ""}
            ${!currentFolders.length && !currentPersonas.length ? renderPersonaEmptyState() : ""}
          </section>
          ${state.operation ? `<section class="panel"><h2>最近 Persona 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
        </main>
      </div>
      ${renderPersonaDialogs(catalog)}
    </section>
  `;
}

function personaMatches(profile, search) {
  const value = (search || "").trim().toLowerCase();
  if (!value) return true;
  return [
    profile.id,
    profile.persona_id,
    profile.system_prompt,
    profile.folder_id,
    ...(profile.tools || []),
    ...(profile.skills || []),
  ].some((item) => String(item || "").toLowerCase().includes(value));
}

function personaCatalog() {
  const folders = (state.personas?.folders || []).map(normalizeFolder);
  const tree = (state.personaFolderTree?.length ? state.personaFolderTree : buildFolderTree(folders)).map(normalizeFolderTreeNode);
  return {
    personas: (state.personas?.personas || []).map(normalizePersona),
    folders,
    tree,
  };
}

function normalizePersona(profile = {}) {
  const id = profile.persona_id || profile.id || "";
  return {
    ...profile,
    id,
    persona_id: id,
    folder_id: normalizeFolderId(profile.folder_id),
    sort_order: Number(profile.sort_order || 0),
    begin_dialogs: normalizeDialogContents(profile.begin_dialogs || []),
  };
}

function normalizeFolder(folder = {}) {
  const id = folder.folder_id || folder.id || "";
  return {
    ...folder,
    id,
    folder_id: id,
    parent_id: normalizeFolderId(folder.parent_id),
    sort_order: Number(folder.sort_order || 0),
  };
}

function normalizeFolderTreeNode(folder = {}) {
  const normalized = normalizeFolder(folder);
  return {
    ...normalized,
    children: (folder.children || []).map(normalizeFolderTreeNode),
  };
}

function normalizeFolderId(folderId) {
  const value = String(folderId || "").trim();
  if (!value || value === "__root" || value === "null") return null;
  return value;
}

function buildFolderTree(folders) {
  const byParent = new Map();
  for (const folder of folders) {
    const parentId = normalizeFolderId(folder.parent_id) || "";
    byParent.set(parentId, [...(byParent.get(parentId) || []), folder]);
  }
  const build = (parentId = "") => (byParent.get(parentId) || [])
    .sort(compareBySortAndName)
    .map((folder) => ({ ...folder, children: build(folder.id) }));
  return build("");
}

function childFolders(folders, parentId) {
  return folders
    .filter((folder) => normalizeFolderId(folder.parent_id) === parentId)
    .sort(compareBySortAndName);
}

function compareBySortAndName(left, right) {
  return (left.sort_order || 0) - (right.sort_order || 0)
    || String(left.name || left.id).localeCompare(String(right.name || right.id));
}

function renderPersonaBreadcrumb(folderId, folders) {
  const path = folderPathItems(folderId, folders);
  return `
    <nav class="ui-folder-breadcrumb" aria-label="Persona folder breadcrumb">
      <button type="button" data-action="persona-open-folder" data-folder="" ${folderId ? "" : 'aria-current="page"'}>⌂ 根目录</button>
      ${path.map((folder, index) => `
        <span>/</span>
        <button type="button" data-action="persona-open-folder" data-folder="${escapeHtml(folder.id)}" ${index === path.length - 1 ? 'aria-current="page"' : ""}>${escapeHtml(folder.name || folder.id)}</button>
      `).join("")}
    </nav>
  `;
}

function folderPathItems(folderId, folders) {
  const items = [];
  let parentId = folderId;
  const seen = new Set();
  while (parentId && !seen.has(parentId)) {
    seen.add(parentId);
    const parent = folders.find((item) => item.id === parentId);
    if (!parent) break;
    items.unshift(parent);
    parentId = parent.parent_id;
  }
  return items;
}

function folderPath(folder, folders) {
  return [...folderPathItems(folder.id, folders).map((item) => item.name || item.id)].join(" / ") || folder.name || folder.id;
}

function currentFolderName(folderId, folders) {
  if (!folderId) return "根目录";
  return folderPathItems(folderId, folders).at(-1)?.name || folderId;
}

function renderPersonaFolderTree(tree, currentFolderId) {
  return `
    <div class="ui-folder-tree persona-folder-tree">
      <ul>
        <li>
          <button class="ui-tree-node root ${currentFolderId ? "" : "active"}" type="button" data-action="persona-open-folder" data-folder="" data-drop-folder="">
            <span class="ui-tree-expander">⌂</span><span>根目录</span>
          </button>
        </li>
        ${tree.length ? tree.map((folder) => renderPersonaTreeNode(folder, currentFolderId, 0)).join("") : ""}
      </ul>
      ${tree.length ? "" : uiState({ state: "empty", message: "暂无文件夹。", compact: true })}
    </div>
  `;
}

function renderPersonaTreeNode(folder, currentFolderId, depth) {
  const children = folder.children || [];
  const active = currentFolderId === folder.id;
  return `
    <li>
      <button class="ui-tree-node ${active ? "active" : ""}" style="--tree-depth:${depth}" type="button" data-action="persona-open-folder" data-folder="${escapeHtml(folder.id)}" data-drop-folder="${escapeHtml(folder.id)}">
        <span class="ui-tree-expander">${children.length ? "▾" : "·"}</span>
        <span>📁</span>
        <span>${escapeHtml(folder.name || folder.id)}</span>
      </button>
      ${children.length ? `<ul>${children.map((child) => renderPersonaTreeNode(child, currentFolderId, depth + 1)).join("")}</ul>` : ""}
    </li>
  `;
}

function renderFolderCard(folder, index, total, folders) {
  return `
    <article class="ui-folder-card persona-folder-card" data-drop-folder="${escapeHtml(folder.id)}">
      <div class="ui-folder-icon" aria-hidden="true"></div>
      <div class="ui-folder-copy">
        <strong>${escapeHtml(folder.name || folder.id)}</strong>
        <span>${escapeHtml(folder.description || folderPath(folder, folders))}</span>
      </div>
      <div class="persona-card-actions">
        <button class="button secondary" type="button" data-action="persona-open-folder" data-folder="${escapeHtml(folder.id)}">打开</button>
        <button class="button ghost" type="button" data-action="persona-folder-rename-open" data-folder="${escapeHtml(folder.id)}">重命名</button>
        <button class="button ghost" type="button" data-action="persona-folder-move-open" data-folder="${escapeHtml(folder.id)}">移动</button>
        <button class="button ghost" type="button" data-action="persona-folder-rank-up" data-folder="${escapeHtml(folder.id)}" ${index === 0 ? "disabled" : ""}>上移</button>
        <button class="button ghost" type="button" data-action="persona-folder-rank-down" data-folder="${escapeHtml(folder.id)}" ${index === total - 1 ? "disabled" : ""}>下移</button>
        <button class="button ghost danger" type="button" data-action="persona-folder-delete-open" data-folder="${escapeHtml(folder.id)}">删除</button>
      </div>
    </article>
  `;
}

function renderPersonaCard(profile, index, total, folders) {
  return `
    <article class="persona-card" draggable="true" data-drag-type="persona" data-persona="${escapeHtml(profile.id)}">
      <header>
        <div class="persona-card-title">
          <strong>${escapeHtml(profile.id)}</strong>
          <span>${escapeHtml(folderLabel(profile.folder_id, folders))} · order ${profile.sort_order || 0}</span>
        </div>
        <button class="button icon ghost" type="button" data-action="persona-preview" data-persona="${escapeHtml(profile.id)}" aria-label="预览 ${escapeHtml(profile.id)}">⋯</button>
      </header>
      <p class="persona-prompt-preview">${escapeHtml(truncate(profile.system_prompt, 160))}</p>
      <div class="ui-chip-row">
        ${renderBeginDialogChip(profile)}
        ${renderAccessChip("工具", profile.tools)}
        ${renderAccessChip("Skills", profile.skills)}
      </div>
      <footer class="persona-card-actions">
        <button class="button secondary" type="button" data-action="persona-preview" data-persona="${escapeHtml(profile.id)}">预览</button>
        <button class="button ghost" type="button" data-action="persona-edit-open" data-persona="${escapeHtml(profile.id)}">编辑</button>
        <button class="button ghost" type="button" data-action="persona-clone-open" data-persona="${escapeHtml(profile.id)}">克隆</button>
        <button class="button ghost" type="button" data-action="persona-move-open" data-persona="${escapeHtml(profile.id)}">移动</button>
        <button class="button ghost" type="button" data-action="persona-rank-up" data-persona="${escapeHtml(profile.id)}" ${index === 0 ? "disabled" : ""}>上移</button>
        <button class="button ghost" type="button" data-action="persona-rank-down" data-persona="${escapeHtml(profile.id)}" ${index === total - 1 ? "disabled" : ""}>下移</button>
        <button class="button ghost danger" type="button" data-action="persona-delete-open" data-persona="${escapeHtml(profile.id)}">删除</button>
      </footer>
    </article>
  `;
}

function renderPersonaEmptyState() {
  return `
    <div class="persona-empty-state">
      <div class="persona-empty-icon">□</div>
      <h3>当前文件夹为空</h3>
      <p>可以创建 Persona 或新建子文件夹。</p>
      <div class="actions">
        <button class="button" type="button" data-action="persona-create-open">创建 Persona</button>
        <button class="button secondary" type="button" data-action="persona-folder-create-open">新建文件夹</button>
      </div>
    </div>
  `;
}

function renderPersonaDialogs(catalog) {
  return [
    renderPersonaFormDialog(catalog),
    renderPersonaPreviewDialog(catalog),
    renderCreateFolderDialog(catalog),
    renderRenameFolderDialog(catalog),
    renderMovePersonaDialog(catalog),
    renderClonePersonaDialog(catalog),
    renderDeletePersonaDialog(catalog),
  ].join("");
}

function renderPersonaFormDialog(catalog) {
  const editing = state.personaEditId ? personaById(catalog.personas, state.personaEditId) : null;
  const profile = editing || {
    id: "",
    persona_id: "",
    system_prompt: "",
    custom_error_message: "",
    begin_dialogs: [],
    tools: null,
    skills: null,
    folder_id: normalizeFolderId(state.personaFolderId),
    sort_order: 0,
  };
  const dialogs = normalizeDialogContents(profile.begin_dialogs || []);
  const pairCount = Math.max(1, Math.ceil(dialogs.length / 2), state.personaDialogPairCount || 1);
  return dialog({
    id: "persona-form-dialog",
    title: editing ? `编辑 Persona：${profile.id}` : "创建 Persona",
    open: state.personaDialog === "form",
    fullscreen: true,
    maxWidth: "1120px",
    closeAction: "persona-dialog-close",
    body: `
      <input id="persona-form-mode" type="hidden" value="${editing ? "edit" : "create"}" />
      <div class="persona-form-layout">
        <div class="persona-form-basic">
          ${!editing ? `<p class="empty compact">创建位置：${escapeHtml(currentFolderName(profile.folder_id, catalog.folders))}</p>` : ""}
          <div class="form-row"><label>Persona ID</label><input id="persona-form-id" value="${escapeHtml(profile.id)}" ${editing ? "readonly" : ""} /></div>
          <div class="form-row"><label>System prompt</label><textarea id="persona-form-prompt" rows="14">${escapeHtml(profile.system_prompt)}</textarea></div>
          <div class="form-row"><label>Custom error message</label><textarea id="persona-form-error" rows="4">${escapeHtml(profile.custom_error_message || "")}</textarea></div>
          <div class="form-grid cols-2">
            <div class="form-row"><label>Folder</label>${renderFolderSelect("persona-form-folder", profile.folder_id, catalog.folders, true)}</div>
            <div class="form-row"><label>Sort order</label><input id="persona-form-sort-order" type="number" value="${escapeHtml(profile.sort_order || 0)}" /></div>
          </div>
        </div>
        <div class="persona-form-panels">
          ${renderSelectionPanel({ kind: "tools", title: "工具", selected: profile.tools, options: toolOptions(), mcpServers: mcpServers() })}
          ${renderSelectionPanel({ kind: "skills", title: "Skills", selected: profile.skills, options: skillOptions() })}
          <section class="persona-form-panel">
            <h3>预设对话</h3>
            <p class="metric-label">按源端规则使用 user / assistant 成对消息。</p>
            <div id="persona-dialog-pairs" class="persona-dialog-pairs">
              ${Array.from({ length: pairCount }, (_, index) => renderDialogPair(index, dialogs)).join("")}
            </div>
            <button class="button secondary" type="button" data-action="persona-dialog-add-pair">添加对话组</button>
          </section>
          ${editing ? renderPersonaQuickPreview(profile, catalog) : ""}
        </div>
      </div>
    `,
    actions: [
      { label: "取消", variant: "ghost", value: "cancel", action: "persona-dialog-close" },
      editing ? `<button class="button danger" type="button" data-action="persona-delete-open" data-persona="${escapeHtml(profile.id)}">删除</button>` : "",
      { label: "保存 Persona", action: "persona-form-save" },
    ].filter(Boolean),
  });
}

function renderSelectionPanel({ kind, title, selected, options, mcpServers: servers = [] }) {
  const all = selected === null;
  const selectedValues = Array.isArray(selected) ? selected : [];
  return `
    <section class="persona-form-panel" data-selector-kind="${escapeHtml(kind)}">
      <h3>${escapeHtml(title)}</h3>
      <div class="persona-radio-row">
        <label><input type="radio" name="persona-${escapeHtml(kind)}-mode" value="all" ${all ? "checked" : ""} /> 默认使用全部</label>
        <label><input type="radio" name="persona-${escapeHtml(kind)}-mode" value="specific" ${all ? "" : "checked"} /> 选择指定项</label>
      </div>
      <div class="form-row">
        <label>搜索 ${escapeHtml(title)}</label>
        <input id="persona-${escapeHtml(kind)}-search" placeholder="按名称、描述或来源过滤" />
      </div>
      ${kind === "tools" && servers.length ? `
        <div class="persona-mcp-servers">
          <span class="metric-label">MCP quick select</span>
          ${servers.map((server) => `<span class="tag">${escapeHtml(server.name)} (${(server.tools || []).length})</span>`).join("")}
        </div>
      ` : ""}
      <div class="persona-option-list">
        ${options.length ? options.map((option) => `
          <label class="persona-option-row">
            <input type="checkbox" name="persona-${escapeHtml(kind)}" value="${escapeHtml(option.name)}" ${all || selectedValues.includes(option.name) ? "checked" : ""} />
            <span>
              <strong>${escapeHtml(option.name)}</strong>
              ${option.description ? `<small>${escapeHtml(truncate(option.description, 90))}</small>` : ""}
            </span>
            ${option.origin || option.origin_name ? `<em>${escapeHtml([option.origin, option.origin_name].filter(Boolean).join(" / "))}</em>` : ""}
          </label>
        `).join("") : `<p class="empty compact">暂无可选 ${escapeHtml(title)}。</p>`}
      </div>
      <div class="ui-chip-row">
        ${all ? chip(`全部 ${title}`, "ok") : selectedValues.length ? selectedValues.map((item) => chip(item)).join("") : chip(`未选择 ${title}`, "warn")}
      </div>
    </section>
  `;
}

function renderDialogPair(index, dialogs) {
  return `
    <div class="persona-dialog-pair" data-persona-dialog-pair="${index}">
      <div class="form-row"><label>User message</label><textarea class="persona-dialog-user" rows="2">${escapeHtml(dialogs[index * 2] || "")}</textarea></div>
      <div class="form-row"><label>Assistant message</label><textarea class="persona-dialog-assistant" rows="2">${escapeHtml(dialogs[index * 2 + 1] || "")}</textarea></div>
    </div>
  `;
}

function renderPersonaQuickPreview(profile, catalog) {
  return `
    <section class="persona-quick-preview">
      <header><small>Quick Preview</small></header>
      <div class="section-title">System prompt</div>
      <pre>${escapeHtml(profile.system_prompt || "")}</pre>
      <div class="section-title">Tools</div>
      <div class="ui-chip-row">${renderAccessList(profile.tools, toolOptions().length, "工具")}</div>
      <div class="section-title">Skills</div>
      <div class="ui-chip-row">${renderAccessList(profile.skills, skillOptions().length, "Skills")}</div>
      <small class="metric-label">${escapeHtml(folderLabel(profile.folder_id, catalog.folders))}</small>
    </section>
  `;
}

function renderPersonaPreviewDialog(catalog) {
  const profile = personaById(catalog.personas, state.personaPreviewId);
  return dialog({
    id: "persona-preview-dialog",
    title: profile ? profile.id : "Persona preview",
    open: state.personaDialog === "preview",
    maxWidth: "720px",
    closeAction: "persona-dialog-close",
    body: profile ? `
      <section class="persona-preview-dialog-body">
        <h3>System prompt</h3>
        <pre class="persona-preview-prompt">${escapeHtml(profile.system_prompt)}</pre>
        ${profile.custom_error_message ? `<h3>Custom error message</h3><pre class="persona-preview-prompt">${escapeHtml(profile.custom_error_message)}</pre>` : ""}
        <h3>预设对话</h3>
        ${renderDialogPreview(profile)}
        <h3>工具</h3>
        <div class="ui-chip-row">${renderAccessList(profile.tools, toolOptions().length, "工具")}</div>
        <h3>Skills</h3>
        <div class="ui-chip-row">${renderAccessList(profile.skills, skillOptions().length, "Skills")}</div>
      </section>
    ` : `<p class="empty">未找到 Persona。</p>`,
    actions: [
      { label: "关闭", variant: "ghost", value: "close", action: "persona-dialog-close" },
      profile ? `<button class="button" type="button" data-action="persona-edit-open" data-persona="${escapeHtml(profile.id)}">编辑</button>` : "",
    ].filter(Boolean),
  });
}

function renderCreateFolderDialog(catalog) {
  return dialog({
    id: "persona-folder-create-dialog",
    title: "创建文件夹",
    open: state.personaDialog === "create-folder",
    closeAction: "persona-dialog-close",
    body: `
      <div class="form-row"><label>名称</label><input id="persona-folder-name" autofocus /></div>
      <div class="form-row"><label>描述</label><textarea id="persona-folder-description" rows="3"></textarea></div>
      <div class="form-row"><label>父文件夹</label>${renderFolderSelect("persona-folder-parent", normalizeFolderId(state.personaFolderId), catalog.folders, true)}</div>
    `,
    actions: [
      { label: "取消", variant: "ghost", value: "cancel", action: "persona-dialog-close" },
      { label: "创建", action: "persona-folder-create-submit" },
    ],
  });
}

function renderRenameFolderDialog(catalog) {
  const folder = folderById(catalog.folders, state.personaRenameFolderId);
  return dialog({
    id: "persona-folder-rename-dialog",
    title: "重命名文件夹",
    open: state.personaDialog === "rename-folder",
    closeAction: "persona-dialog-close",
    body: folder ? `<div class="form-row"><label>名称</label><input id="persona-folder-rename-name" value="${escapeHtml(folder.name || "")}" /></div>` : `<p class="empty">未找到文件夹。</p>`,
    actions: [
      { label: "取消", variant: "ghost", value: "cancel", action: "persona-dialog-close" },
      folder ? { label: "保存", action: "persona-folder-rename-submit" } : "",
    ].filter(Boolean),
  });
}

function renderMovePersonaDialog(catalog) {
  const isFolder = state.personaMoveType === "folder";
  const item = isFolder ? folderById(catalog.folders, state.personaMoveId) : personaById(catalog.personas, state.personaMoveId);
  const disabled = isFolder ? collectFolderAndChildrenIds(catalog.tree, state.personaMoveId) : [];
  return dialog({
    id: "persona-move-dialog",
    title: "移动到文件夹",
    open: state.personaDialog === "move",
    closeAction: "persona-dialog-close",
    body: item ? `
      <p class="empty compact">移动：${escapeHtml(isFolder ? item.name : item.id)}</p>
      <div class="form-row"><label>目标文件夹</label>${renderFolderSelect("persona-move-target", isFolder ? item.parent_id : item.folder_id, catalog.folders, true, disabled)}</div>
      <div class="persona-move-tree">${renderPersonaFolderTree(catalog.tree, normalizeFolderId(isFolder ? item.parent_id : item.folder_id))}</div>
    ` : `<p class="empty">未找到要移动的项目。</p>`,
    actions: [
      { label: "取消", variant: "ghost", value: "cancel", action: "persona-dialog-close" },
      item ? { label: "移动", action: "persona-move-submit" } : "",
    ].filter(Boolean),
  });
}

function renderClonePersonaDialog(catalog) {
  const profile = personaById(catalog.personas, state.personaCloneSourceId);
  return dialog({
    id: "persona-clone-dialog",
    title: "克隆 Persona",
    open: state.personaDialog === "clone",
    closeAction: "persona-dialog-close",
    body: profile ? `
      <p class="empty compact">从 ${escapeHtml(profile.id)} 克隆。</p>
      <div class="form-row"><label>New Persona ID</label><input id="persona-clone-new-id" value="${escapeHtml(`${profile.id}_copy`)}" /></div>
    ` : `<p class="empty">未找到 Persona。</p>`,
    actions: [
      { label: "取消", variant: "ghost", value: "cancel", action: "persona-dialog-close" },
      profile ? { label: "克隆", action: "persona-clone-submit" } : "",
    ].filter(Boolean),
  });
}

function renderDeletePersonaDialog(catalog) {
  const isFolder = state.personaDeleteType === "folder";
  const item = isFolder ? folderById(catalog.folders, state.personaDeleteId) : personaById(catalog.personas, state.personaDeleteId);
  return dialog({
    id: "persona-delete-dialog",
    title: isFolder ? "删除文件夹" : "删除 Persona",
    kind: "danger",
    open: state.personaDialog === "delete",
    closeAction: "persona-dialog-close",
    body: item ? `
      <p class="ui-dialog-message strong">确认删除 ${escapeHtml(isFolder ? item.name : item.id)}？</p>
      ${isFolder ? `<p class="ui-hint-row">删除文件夹后，子文件夹和 Persona 会回到根目录。</p>` : ""}
    ` : `<p class="empty">未找到要删除的项目。</p>`,
    actions: [
      { label: "取消", variant: "ghost", value: "cancel", action: "persona-dialog-close" },
      item ? { label: "删除", variant: "danger", action: "persona-delete-confirm" } : "",
    ].filter(Boolean),
  });
}

function renderFolderSelect(id, value, folders, includeRoot = false, disabledIds = []) {
  return `
    <select id="${escapeHtml(id)}">
      ${includeRoot ? `<option value="" ${value ? "" : "selected"}>根目录</option>` : ""}
      ${folders.map((folder) => `<option value="${escapeHtml(folder.id)}" ${value === folder.id ? "selected" : ""} ${disabledIds.includes(folder.id) ? "disabled" : ""}>${escapeHtml(folderPath(folder, folders))}</option>`).join("")}
    </select>
  `;
}

function renderDialogPreview(profile) {
  const dialogs = normalizeDialogContents(profile.begin_dialogs || []);
  if (!dialogs.length) return `<p class="empty compact">暂无预设对话。</p>`;
  return dialogs.map((content, index) => `
    <div class="persona-dialog-preview-row">
      <span class="tag">${index % 2 === 0 ? "User" : "Assistant"}</span>
      <div>${escapeHtml(content)}</div>
    </div>
  `).join("");
}

function renderBeginDialogChip(profile) {
  const count = Math.floor(normalizeDialogContents(profile.begin_dialogs || []).length / 2);
  return count ? chip(`${count} 组对话`, "accent") : "";
}

function renderAccessChip(label, value) {
  if (value === null) return chip(`全部 ${label}`, "ok");
  return Array.isArray(value) && value.length ? chip(`${value.length} ${label}`) : chip(`无 ${label}`, "warn");
}

function renderAccessList(value, allCount, label) {
  if (value === null) return chip(`全部 ${label} (${allCount})`, "ok");
  return Array.isArray(value) && value.length ? value.map((item) => chip(item)).join("") : chip(`未选择 ${label}`, "warn");
}

function normalizeDialogContents(items) {
  return (items || [])
    .map((item) => typeof item === "string" ? item : item?.content)
    .map((item) => String(item || "").trim())
    .filter(Boolean);
}

function folderLabel(folderId, folders) {
  if (!folderId) return "根目录";
  const folder = folders.find((item) => item.id === folderId);
  return folder ? folderPath(folder, folders) : folderId;
}

function personaById(personas, personaId) {
  return personas.find((profile) => profile.id === personaId) || null;
}

function folderById(folders, folderId) {
  return folders.find((folder) => folder.id === folderId) || null;
}

function toolOptions() {
  return (state.tools?.tools || state.tools?.data || []).map((tool) => ({
    name: tool.name || tool.id || "",
    description: tool.description || "",
    origin: tool.origin || tool.source || "",
    origin_name: tool.origin_name || "",
  })).filter((tool) => tool.name);
}

function skillOptions() {
  const payload = Array.isArray(state.skills?.data) ? state.skills.data : state.skills?.skills || [];
  return payload.map((skill) => ({
    name: skill.name || skill.id || "",
    description: skill.description || "",
  })).filter((skill) => skill.name && skill.active !== false);
}

function mcpServers() {
  return state.mcp?.servers || state.mcp?.data || [];
}

function collectFolderAndChildrenIds(tree, folderId) {
  const ids = [];
  const visit = (nodes) => {
    for (const node of nodes || []) {
      if (node.id === folderId || ids.includes(node.parent_id)) {
        ids.push(node.id);
      }
      visit(node.children || []);
    }
  };
  visit(tree);
  return ids;
}

function truncate(value, maxLength) {
  const text = String(value || "");
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

export function renderCron() {
  const cron = state.cron || {};
  const jobs = cron.jobs || [];
  const proactivePlatforms = cron.proactive_platforms || [];
  const platformText = proactivePlatforms
    .map((platform) => `${platform.display_name || platform.name || platform.id}(${platform.id})`)
    .join(" / ");
  return `
    <div class="cron-page" data-page="cron">
      <section class="panel cron-hero-panel">
        <div class="panel-title-row cron-title-row">
          <div>
            <div class="cron-title-inline">
              <h2>Cron Jobs</h2>
              ${chip("Beta", "warn")}
            </div>
            <p class="metric-label">
              管理定时任务和主动唤醒。${platformText ? `支持主动消息的平台：${escapeHtml(platformText)}` : "当前没有平台声明支持主动消息。"}
            </p>
          </div>
          <div class="actions">
            <button class="button" type="button" data-action="cron-create-open">创建任务</button>
            <button class="button ghost" type="button" data-action="load-cron">刷新</button>
          </div>
        </div>
      </section>

      <div class="grid cols-4 cron-metrics">
        ${metric("Scheduler", cron.state || "-", "runtime lifecycle")}
        ${metric("Jobs", jobs.length, "persistent repository")}
        ${metric("Scheduled", cron.scheduled_jobs?.length ?? 0, "driver snapshot")}
        ${metric("Proactive", proactivePlatforms.length, "platform support")}
      </div>

      <section class="panel cron-table-panel">
        <div class="panel-title-row">
          <h2>任务列表</h2>
          <div class="actions">
            <button class="button secondary" type="button" data-action="cron-start">Start</button>
            <button class="button secondary" type="button" data-action="cron-tick">Tick due</button>
            <button class="button ghost" type="button" data-action="cron-shutdown">Shutdown</button>
          </div>
        </div>
        ${cron.unavailable ? `<p class="empty">${escapeHtml(cron.unavailable)}</p>` : ""}
        ${jobs.length ? `
          <div class="table-wrap cron-table-wrap">
            <table class="table cron-jobs-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Type</th>
                  <th>Cron / Time</th>
                  <th>Session</th>
                  <th>Next Run</th>
                  <th>Last Run</th>
                  <th>Note</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>${jobs.map(renderCronJobRow).join("")}</tbody>
            </table>
          </div>
        ` : uiState({ state: "empty", title: "暂无 Cron job", message: "创建任务后会在这里显示定时表达式、主动唤醒会话和运行记录。" })}
      </section>

      <section class="panel cron-scheduled-panel">
        <div class="panel-title-row">
          <h2>Scheduler Snapshot</h2>
          ${pill(cron.state || "unknown", cron.state === "running" ? "ok" : "warn")}
        </div>
        ${cron.scheduled_jobs?.length ? jsonBlock(cron.scheduled_jobs) : `<p class="empty">scheduler 未启动或没有可计划任务。</p>`}
      </section>

      ${state.operation ? `<section class="panel"><h2>最近 Cron 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
      ${renderCronFormDialog()}
      ${renderCronDeleteDialog()}
    </div>
  `;
}

function renderCronJobRow(job) {
  return `
    <tr>
      <td>
        <strong>${escapeHtml(job.name || "active_agent_task")}</strong>
        <br><span class="metric-label">${escapeHtml(job.job_id)}</span>
        ${job.last_error ? `<br>${pill("failed", "error")}` : ""}
      </td>
      <td>${chip(cronTypeLabel(job), job.run_once ? "warn" : "ok")}</td>
      <td>
        <div>${escapeHtml(cronJobScheduleLabel(job))}</div>
        <span class="metric-label">${escapeHtml(job.timezone || "local")}</span>
      </td>
      <td>${escapeHtml(job.session || "-")}</td>
      <td>${escapeHtml(formatCronTime(job.next_run_time))}</td>
      <td>${escapeHtml(formatCronTime(job.last_run_at))}</td>
      <td>${escapeHtml(job.note || "-")}</td>
      <td class="button-cell cron-action-cell">
        <label class="ui-switch cron-enabled-switch" title="Enabled" data-action="cron-toggle" data-job="${escapeHtml(job.job_id)}" data-enabled="${job.enabled === false ? "false" : "true"}">
          <input type="checkbox" ${job.enabled === false ? "" : "checked"} />
          <span class="ui-switch-track"><span class="ui-switch-thumb"></span></span>
        </label>
        <button class="button secondary" type="button" data-action="cron-edit-open" data-job="${escapeHtml(job.job_id)}">Edit</button>
        <button class="button secondary" type="button" data-action="cron-run" data-job="${escapeHtml(job.job_id)}">Run</button>
        <button class="button ghost danger" type="button" data-action="cron-delete-open" data-job="${escapeHtml(job.job_id)}">Delete</button>
      </td>
    </tr>
  `;
}

function renderCronFormDialog() {
  const open = state.cronDialog === "form";
  const editing = state.cronFormMode === "edit" || Boolean(state.cronEditId);
  const job = editing ? cronJobById(state.cronEditId) : null;
  const defaults = job || {
    name: "",
    note: "",
    cron_expression: "0 9 * * *",
    run_at: "",
    session: "",
    timezone: "Asia/Shanghai",
    enabled: true,
    run_once: false,
  };
  return dialog({
    id: "cron-form-dialog",
    title: editing ? "编辑 Cron Job" : "创建 Cron Job",
    open,
    maxWidth: "620px",
    closeAction: "cron-dialog-close",
    body: `
      <input id="cron-form-mode" type="hidden" value="${editing ? "edit" : "create"}" />
      <input id="cron-form-job-id" type="hidden" value="${escapeHtml(job?.job_id || "")}" />
      <div class="form-grid cols-2 cron-form-grid">
        <label class="check-row wide-field"><input id="cron-form-run-once" type="checkbox" ${defaults.run_once ? "checked" : ""} /> Run once</label>
        <div class="form-row"><label>Name</label><input id="cron-form-name" value="${escapeHtml(defaults.name || "")}" placeholder="active_agent_task" /></div>
        <div class="form-row"><label>Session</label><input id="cron-form-session" value="${escapeHtml(defaults.session || "")}" placeholder="webchat:conversation-id[:group]" /></div>
        <div class="form-row"><label>Cron expression</label><input id="cron-form-cron" value="${escapeHtml(defaults.cron_expression || "")}" placeholder="0 9 * * *" /></div>
        <div class="form-row"><label>Run at</label><input id="cron-form-run-at" type="datetime-local" value="${escapeHtml(datetimeLocalValue(defaults.run_at || ""))}" /></div>
        <div class="form-row"><label>Timezone</label><input id="cron-form-timezone" value="${escapeHtml(defaults.timezone || "")}" placeholder="Asia/Shanghai" /></div>
        <label class="check-row"><input id="cron-form-enabled" type="checkbox" ${defaults.enabled === false ? "" : "checked"} /> Enabled</label>
        <div class="form-row wide-field"><label>Note</label><textarea id="cron-form-note" rows="4">${escapeHtml(defaults.note || "")}</textarea></div>
      </div>
    `,
    actions: [
      { label: "取消", variant: "ghost", action: "cron-dialog-close" },
      { label: editing ? "保存修改" : "创建任务", action: "cron-form-save" },
    ],
  });
}

function renderCronDeleteDialog() {
  const job = cronJobById(state.cronDeleteId);
  return dialog({
    id: "cron-delete-dialog",
    title: "删除 Cron Job",
    open: state.cronDialog === "delete" && Boolean(job),
    maxWidth: "460px",
    closeAction: "cron-dialog-close",
    body: `<p class="dialog-message strong">确认删除 ${escapeHtml(job?.name || state.cronDeleteId || "")}？</p>`,
    actions: [
      { label: "取消", variant: "ghost", action: "cron-dialog-close" },
      { label: "删除", variant: "danger", action: "cron-delete-confirm" },
    ],
  });
}

function cronJobById(jobId) {
  return (state.cron?.jobs || []).find((job) => job.job_id === jobId) || null;
}

function cronTypeLabel(job) {
  if (job.run_once) return "Run once";
  if (job.job_type === "workflow") return "Workflow";
  return job.job_type === "active_agent" ? "Active Agent" : (job.job_type || "Unknown");
}

function cronJobScheduleLabel(job) {
  if (job.run_once) return job.run_at || cronScheduleLabel(job.schedule);
  return job.cron_expression || cronScheduleLabel(job.schedule);
}

export function cronScheduleLabel(schedule) {
  if (schedule?.spec?.cron) return schedule.spec.cron.expression;
  if (schedule?.spec?.run_once) return schedule.spec.run_once.run_at;
  if (schedule?.spec?.Cron) return schedule.spec.Cron.expression;
  if (schedule?.spec?.RunOnce) return schedule.spec.RunOnce.run_at;
  return JSON.stringify(schedule?.spec || {});
}

function formatCronTime(value) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return date.toLocaleString();
}

function datetimeLocalValue(value) {
  if (!value) return "";
  const text = String(value);
  const match = text.match(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/);
  return match ? match[0] : "";
}
