import { escapeHtml, jsonBlock } from "../dom.js";
import { openApiSecret } from "../api.js";
import { state } from "../state.js";
import { chip, dialog, markdownViewer, metric, pill, uiState } from "./shared.js";
export { renderConfig } from "./config.js";

export function renderChat() {
  return renderChatSurface({ chatboxMode: false });
}

export function renderConversation() {
  const payload = conversationPayload();
  const conversations = payload.conversations;
  const pagination = payload.pagination;
  const filters = state.conversationFilters || {};
  const selected = new Set(state.conversationSelectedKeys || []);
  const active = activeConversation(conversations);
  return `
    <div class="conversation-page" data-page="conversation">
      <div class="grid cols-3">
        ${metric("Conversations", pagination.total ?? conversations.length, `${conversations.length} visible`)}
        ${metric("Selected", selected.size, "batch target")}
        ${metric("Page", `${pagination.page || 1}/${pagination.total_pages || 1}`, `${pagination.page_size || 20} per page`)}
      </div>
      <section class="panel conversation-filter-panel">
        <div class="panel-title-row">
          <h2>Conversation History</h2>
          <div class="actions">
            <button class="button secondary" type="button" data-action="conversation-filter-apply">筛选</button>
            <button class="button ghost" type="button" data-action="load-conversations">刷新</button>
            ${selected.size ? `<button class="button secondary" type="button" data-action="conversation-export-selected">导出 ${selected.size}</button>` : ""}
            ${selected.size ? `<button class="button danger" type="button" data-action="conversation-batch-delete-open">删除 ${selected.size}</button>` : ""}
          </div>
        </div>
        <div class="form-grid cols-4">
          <div class="form-row"><label>Platform</label><input id="conversation-filter-platforms" value="${escapeHtml((filters.platforms || []).join(", "))}" placeholder="webchat, aiocqhttp" /></div>
          <div class="form-row"><label>Message type</label><select id="conversation-filter-message-type"><option value="" ${filters.messageTypes?.length ? "" : "selected"}>全部</option><option value="FriendMessage" ${(filters.messageTypes || []).includes("FriendMessage") ? "selected" : ""}>FriendMessage</option><option value="GroupMessage" ${(filters.messageTypes || []).includes("GroupMessage") ? "selected" : ""}>GroupMessage</option></select></div>
          <div class="form-row"><label>Search</label><input id="conversation-filter-search" value="${escapeHtml(filters.search || "")}" placeholder="title / user_id / cid / content" /></div>
          <div class="form-row"><label>Page size</label><select id="conversation-page-size">${[10, 20, 50, 100].map((size) => `<option value="${size}" ${Number(filters.pageSize || pagination.page_size || 20) === size ? "selected" : ""}>${size}</option>`).join("")}</select></div>
        </div>
        ${state.conversations?.unavailable ? `<p class="empty">${escapeHtml(state.conversations.unavailable)}</p>` : ""}
      </section>
      <div class="grid cols-2 conversation-page-grid">
        <section class="panel conversation-list-panel">
          <div class="panel-title-row">
            <h2>列表</h2>
            <div class="actions">
              <button class="button ghost" type="button" data-action="conversation-page-prev" ${Number(pagination.page || 1) <= 1 ? "disabled" : ""}>上一页</button>
              <button class="button ghost" type="button" data-action="conversation-page-next" ${Number(pagination.page || 1) >= Number(pagination.total_pages || 1) ? "disabled" : ""}>下一页</button>
            </div>
          </div>
          ${conversations.length ? `
            <table class="table">
              <thead>
                <tr>
                  <th><input type="checkbox" data-action="conversation-select-all" ${conversations.every((item) => selected.has(conversationKey(item))) ? "checked" : ""} /></th>
                  <th>Title</th>
                  <th>UMO</th>
                  <th>Time</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                ${conversations.map((conversation) => renderConversationRow(conversation, selected)).join("")}
              </tbody>
            </table>
            <div class="conversation-pagination">
              <span>显示 ${Math.min(((pagination.page || 1) - 1) * (pagination.page_size || 20) + 1, pagination.total || conversations.length)}-${Math.min((pagination.page || 1) * (pagination.page_size || 20), pagination.total || conversations.length)} / ${pagination.total || conversations.length}</span>
              <span>${pagination.page || 1} / ${pagination.total_pages || 1}</span>
            </div>
          ` : `<p class="empty">暂无 Conversation 记录，或筛选条件没有匹配项。</p>`}
        </section>
        <section class="panel">
          <div class="panel-title-row">
            <h2>详情</h2>
            ${active ? `<button class="button secondary" type="button" data-action="conversation-view" data-user="${escapeHtml(active.user_id)}" data-cid="${escapeHtml(active.cid)}">重新加载详情</button>` : ""}
          </div>
          ${active ? renderConversationSummary(active) : `<p class="empty">选择列表中的 Conversation 以查看详情、编辑标题或更新 history JSON。</p>`}
        </section>
      </div>
      ${renderConversationDialogs()}
      ${state.operation ? `<section class="panel"><h2>最近 Conversation 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
    </div>
  `;
}

function renderConversationRow(conversation, selected) {
  const key = conversationKey(conversation);
  const session = conversationSessionInfo(conversation.user_id);
  return `
    <tr class="${state.conversationDetail?.cid === conversation.cid ? "selected-row" : ""}">
      <td><input type="checkbox" data-action="conversation-select" data-key="${escapeHtml(key)}" ${selected.has(key) ? "checked" : ""} /></td>
      <td>
        <strong>${escapeHtml(conversation.title || "Untitled Conversation")}</strong>
        <br><span class="metric-label">${escapeHtml(conversation.cid)}</span>
        ${conversation.persona_id ? `<br>${pill(conversation.persona_id)}` : ""}
      </td>
      <td>
        <span class="tag">${escapeHtml(session.platform)}</span>
        <span class="tag">${escapeHtml(session.messageType)}</span>
        <br><span class="metric-label">${escapeHtml(conversation.user_id || session.sessionId)}</span>
      </td>
      <td>
        <span>${escapeHtml(formatConversationTimestamp(conversation.updated_at))}</span>
        <br><span class="metric-label">created ${escapeHtml(formatConversationTimestamp(conversation.created_at))}</span>
      </td>
      <td class="button-cell">
        <button class="button secondary" type="button" data-action="conversation-view" data-user="${escapeHtml(conversation.user_id)}" data-cid="${escapeHtml(conversation.cid)}">查看</button>
        <button class="button secondary" type="button" data-action="conversation-edit-open" data-user="${escapeHtml(conversation.user_id)}" data-cid="${escapeHtml(conversation.cid)}">编辑</button>
        <button class="button ghost" type="button" data-action="conversation-delete-open" data-user="${escapeHtml(conversation.user_id)}" data-cid="${escapeHtml(conversation.cid)}">删除</button>
      </td>
    </tr>
  `;
}

function renderConversationSummary(conversation) {
  const history = parseConversationHistory(conversation.history);
  const session = conversationSessionInfo(conversation.user_id);
  return `
    <div class="conversation-summary">
      <div class="grid cols-3">
        ${metric("Messages", history.length, conversation.cid)}
        ${metric("Platform", session.platform, session.messageType)}
        ${metric("Persona", conversation.persona_id || "-", "linked persona")}
      </div>
      <div class="form-grid cols-2">
        <div class="form-row"><label>User ID</label><input value="${escapeHtml(conversation.user_id)}" readonly /></div>
        <div class="form-row"><label>CID</label><input value="${escapeHtml(conversation.cid)}" readonly /></div>
      </div>
      <div class="messages conversation-preview">
        ${history.length ? history.map((message, index) => renderMessage(conversationHistoryMessage(message), index)).join("") : `<p class="empty">当前 conversation 暂无历史。</p>`}
      </div>
      <details class="mt-16" open>
        <summary>Raw history JSON</summary>
        ${jsonBlock(history)}
      </details>
    </div>
  `;
}

function renderConversationDialogs() {
  const detail = state.conversationDetail || {};
  const editTarget = state.conversationEditTarget || detail || {};
  const deleteTarget = state.conversationDeleteTarget || detail || {};
  return `
    ${dialog({
      id: "conversation-history-dialog",
      title: detail.title || "Conversation Details",
      open: state.conversationDialog === "history",
      maxWidth: "920px",
      body: `
        <div class="conversation-dialog-toolbar">
          <button class="button secondary" type="button" data-action="conversation-history-preview">预览</button>
          <button class="button secondary" type="button" data-action="conversation-history-edit">编辑 JSON</button>
          <button class="button" type="button" data-action="conversation-history-save">保存 History</button>
        </div>
        ${state.conversationHistoryMode === "edit" ? `
          <textarea id="conversation-history-editor" class="json-editor conversation-history-editor" spellcheck="false">${escapeHtml(state.conversationHistoryDraft || detail.history || "[]")}</textarea>
        ` : `
          <div class="messages conversation-preview">${parseConversationHistory(detail.history).map((message, index) => renderMessage(conversationHistoryMessage(message), index)).join("") || `<p class="empty">当前 conversation 暂无历史。</p>`}</div>
        `}
      `,
      actions: [
        { label: "关闭", variant: "ghost", action: "conversation-dialog-close" },
      ],
    })}
    ${dialog({
      id: "conversation-edit-dialog",
      title: "Edit Conversation",
      open: state.conversationDialog === "edit",
      body: `
        <div class="form-row"><label>User ID</label><input id="conversation-edit-user-id" value="${escapeHtml(editTarget.user_id || "")}" readonly /></div>
        <div class="form-row"><label>CID</label><input id="conversation-edit-cid" value="${escapeHtml(editTarget.cid || "")}" readonly /></div>
        <div class="form-row"><label>Title</label><input id="conversation-edit-title" value="${escapeHtml(editTarget.title || "")}" placeholder="Conversation title" /></div>
        <div class="form-row"><label>Persona ID</label><input id="conversation-edit-persona" value="${escapeHtml(editTarget.persona_id || "")}" /></div>
      `,
      actions: [
        { label: "取消", variant: "ghost", action: "conversation-dialog-close" },
        { label: "保存", action: "conversation-edit-save" },
      ],
    })}
    ${dialog({
      id: "conversation-delete-dialog",
      title: "删除 Conversation",
      open: state.conversationDialog === "delete",
      body: `<p>确认删除 ${escapeHtml(deleteTarget.title || deleteTarget.cid || "selected conversation")}？</p>`,
      actions: [
        { label: "取消", variant: "ghost", action: "conversation-dialog-close" },
        { label: "删除", variant: "danger", action: "conversation-delete-confirm" },
      ],
    })}
    ${dialog({
      id: "conversation-batch-delete-dialog",
      title: "批量删除 Conversation",
      open: state.conversationDialog === "batch-delete",
      body: `<p>确认删除 ${escapeHtml(String((state.conversationSelectedKeys || []).length))} 个 selected conversations？</p>`,
      actions: [
        { label: "取消", variant: "ghost", action: "conversation-dialog-close" },
        { label: "批量删除", variant: "danger", action: "conversation-batch-delete-confirm" },
      ],
    })}
  `;
}

function conversationPayload() {
  const payload = state.conversations || {};
  const data = payload.data || payload;
  const conversations = Array.isArray(data.conversations) ? data.conversations.map(normalizeConversationRecord) : [];
  const fallbackPagination = {
    page: state.conversationFilters?.page || 1,
    page_size: state.conversationFilters?.pageSize || 20,
    total: conversations.length,
    total_pages: Math.max(1, Math.ceil(conversations.length / (state.conversationFilters?.pageSize || 20))),
  };
  return {
    conversations,
    pagination: data.pagination || fallbackPagination,
  };
}

function normalizeConversationRecord(conversation) {
  const platformId = conversation.platform_id || conversation.platform || "webchat";
  const cid = conversation.cid || conversation.conversation_id || conversation.session_id || conversation.id || "";
  return {
    ...conversation,
    cid,
    conversation_id: cid,
    platform_id: platformId,
    user_id: conversation.user_id || `${platformId}:FriendMessage:${cid}`,
    history: typeof conversation.history === "string" ? conversation.history : JSON.stringify(conversation.history || []),
    created_at: conversation.created_at ?? "",
    updated_at: conversation.updated_at ?? "",
  };
}

function activeConversation(conversations) {
  const detail = state.conversationDetail;
  if (detail?.cid) return normalizeConversationRecord(detail);
  return conversations[0] || null;
}

function conversationKey(conversation) {
  return `${conversation.user_id || ""}\u001f${conversation.cid || conversation.conversation_id || ""}`;
}

function conversationSessionInfo(userId = "") {
  const parts = String(userId || "").split(":");
  if (parts.length >= 3) {
    return {
      platform: parts[0] || "default",
      messageType: parts[1] || "default",
      sessionId: parts.slice(2).join(":"),
    };
  }
  return { platform: "default", messageType: "default", sessionId: userId || "" };
}

function parseConversationHistory(history) {
  if (Array.isArray(history)) return history;
  if (!history) return [];
  try {
    const parsed = JSON.parse(history);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function conversationHistoryMessage(message = {}) {
  return {
    id: message.id || message.message_id,
    created_at: message.created_at || message.timestamp || "",
    content: {
      type: message.role === "assistant" || message.role === "bot" ? "bot" : "user",
      message: contentToParts(message.content),
      reasoning: message.reasoning || "",
      refs: message.refs || [],
    },
  };
}

function contentToParts(content) {
  if (typeof content === "string") return [{ type: "plain", text: content }];
  if (Array.isArray(content)) {
    return content.map((part) => {
      if (part?.type === "text") return { type: "plain", text: part.text || "" };
      if (part?.type === "image_url") return { type: "image", url: part.image_url?.url || "" };
      return part;
    });
  }
  if (content && typeof content === "object") {
    return Object.entries(content)
      .filter(([, value]) => typeof value === "string")
      .map(([key, value]) => ({ type: key.includes("image") ? "image" : "plain", text: value, url: value }));
  }
  return [{ type: "plain", text: "" }];
}

function formatConversationTimestamp(value) {
  if (!value) return "-";
  if (typeof value === "number") return new Date(value * 1000).toLocaleString();
  if (/^\d+$/.test(String(value))) return new Date(Number(value) * 1000).toLocaleString();
  return String(value);
}

export function renderChatBox() {
  const subscriptions = state.realtime?.subscriptions || [];
  const elicitations = state.realtime?.elicitations || [];
  return `
    <div class="chatbox-metrics grid cols-3">
      ${metric("Subscriptions", subscriptions.length, state.realtime?.last?.response_mode || "OpenAPI")}
      ${metric("Stop", state.realtime?.lastStop?.status || "-", `${state.realtime?.lastStop?.interrupted_events ?? 0} events`)}
      ${metric("Elicitation", elicitations.length, elicitations[0]?.status || "pending")}
    </div>
    ${renderChatSurface({ chatboxMode: true })}
  `;
}

function renderChatSurface({ chatboxMode = false } = {}) {
  const messages = state.messages || [];
  const chat = state.chat || {};
  const activeProject = currentChatProject();
  return `
    <div data-page="${chatboxMode ? "chatbox" : "chat"}" class="chat-shell ${chatboxMode ? "chat-shell-standalone" : ""}">
      ${renderConversationSidebar({ chatboxMode })}
      <main class="chat-main-panel">
        ${chat.liveModeOpen ? renderLiveMode() : `
          ${activeProject ? renderProjectView(activeProject, { chatboxMode }) : `
            ${renderChatHeader({ chatboxMode })}
            <div class="messages chat-message-list" data-chat-messages>
              ${messages.length ? messages.map((message, index) => renderMessage(message, index)).join("") : renderWelcomeView({ chatboxMode })}
            </div>
            ${renderChatInput({ chatboxMode })}
          `}
        `}
      </main>
      ${renderRefsSidebar()}
      ${renderChatDialogs()}
    </div>
  `;
}

function renderConversationSidebar({ chatboxMode = false } = {}) {
  const sessions = chatSessions();
  const activeId = state.chat.conversationId || "";
  const selected = new Set(state.chat.batchSelectedSessionIds || []);
  const projects = chatProjects();
  const activeProject = projects.find((project) => project.project_id === state.activeProjectId);
  const projectsExpanded = state.chat.projectsExpanded !== false;
  return `
    <aside class="chat-sidebar-panel">
      <div class="chat-sidebar-top">
        <button class="button secondary" type="button" data-action="chat-new-session"><span class="button-icon">+</span>新对话</button>
        <button class="button ghost" type="button" data-action="chat-batch-toggle" aria-pressed="${state.chat.batchMode ? "true" : "false"}">多选</button>
      </div>
      ${state.chat.batchMode ? `
        <div class="chat-batch-bar">
          <span>${selected.size} selected</span>
          <button class="button ghost" type="button" data-action="chat-batch-select-all">全选</button>
          <button class="button danger" type="button" data-action="chat-batch-delete">删除</button>
        </div>
      ` : ""}
      <section class="chat-project-block">
        <button class="chat-project-toggle" type="button" data-action="chat-projects-toggle" aria-expanded="${projectsExpanded ? "true" : "false"}">
          <span>📁 Projects</span>
          <span>${projectsExpanded ? "⌃" : "⌄"}</span>
        </button>
        ${projectsExpanded ? `
          <div class="chat-project-list">
            <button type="button" class="chat-project-create" data-action="chat-project-dialog-open" data-mode="create">
              <span>+</span>
              <strong>创建项目</strong>
            </button>
            ${projects.length ? projects.map((project) => renderChatProjectItem(project)).join("") : `<p class="empty compact">暂无项目</p>`}
          </div>
        ` : ""}
      </section>
      <section class="chat-session-block">
        <div class="chat-section-heading">
          <span>Conversations</span>
          <button class="button icon ghost" type="button" data-action="chat-sessions-refresh" aria-label="Refresh">↻</button>
        </div>
        <div class="chat-session-list">
          ${sessions.length ? sessions.map((session) => `
            <article class="${session.session_id === activeId ? "active" : ""}">
              <button type="button" class="chat-session-select" data-action="chat-select-session" data-session="${escapeHtml(session.session_id)}">
                ${state.chat.batchMode ? `<input type="checkbox" data-action="chat-batch-select" data-session="${escapeHtml(session.session_id)}" ${selected.has(session.session_id) ? "checked" : ""} />` : ""}
                <span>
                  <strong>${escapeHtml(session.display_name || session.title || session.session_id)}</strong>
                  <small>${escapeHtml(session.platform_id || "webchat")} · ${escapeHtml(session.updated_at || "")}</small>
                </span>
              </button>
              ${state.chat.batchMode ? "" : `
                <div class="chat-session-actions">
                  <button class="button icon ghost" type="button" data-action="chat-rename-open" data-session="${escapeHtml(session.session_id)}" data-title="${escapeHtml(session.display_name || session.title || "")}" aria-label="Rename">✎</button>
                  <button class="button icon ghost" type="button" data-action="chat-delete-session" data-session="${escapeHtml(session.session_id)}" aria-label="Delete">×</button>
                </div>
              `}
            </article>
          `).join("") : `<p class="empty compact">暂无历史</p>`}
        </div>
      </section>
      <footer class="chat-sidebar-footer">
        <label class="form-row compact">
          <span>Transport</span>
          <select id="chat-transport-mode">
            <option value="sse" ${state.chat.transportMode === "websocket" ? "" : "selected"}>SSE</option>
            <option value="websocket" ${state.chat.transportMode === "websocket" ? "selected" : ""}>WebSocket</option>
          </select>
        </label>
        <label class="form-row compact">
          <span>Send key</span>
          <select id="chat-send-shortcut">
            <option value="shift_enter" ${state.chat.sendShortcut === "enter" ? "" : "selected"}>Shift+Enter</option>
            <option value="enter" ${state.chat.sendShortcut === "enter" ? "selected" : ""}>Enter</option>
          </select>
        </label>
        <button class="button secondary" type="button" data-action="chat-settings-apply">应用设置</button>
        ${activeProject ? `<p class="empty compact">当前项目：${escapeHtml(activeProject.title || activeProject.project_id)}</p>` : ""}
        ${chatboxMode ? `<button class="button ghost" type="button" data-action="chatbox-fullscreen-toggle">ChatBox</button>` : ""}
      </footer>
    </aside>
  `;
}

function renderChatHeader({ chatboxMode = false } = {}) {
  const activeProject = state.chat.currentSessionProject || currentChatProject();
  return `
    <header class="chat-header">
      <div>
        <div class="chat-breadcrumb">
          <span>${chatboxMode ? "ChatBox" : "Chat"}</span>
          <span>/</span>
          <strong>${escapeHtml(state.chat.conversationId || "new")}</strong>
          ${activeProject ? `<span>/</span><span>${escapeHtml(activeProject.title || activeProject.project_id)}</span>` : ""}
        </div>
        <div class="chat-header-meta">
          ${chip(`${(state.messages || []).length} messages`, "label")}
          ${chip(state.chat.enableStreaming === false ? "Streaming off" : "Streaming on", state.chat.enableStreaming === false ? "warn" : "active")}
          ${chip(state.chat.transportMode === "websocket" ? "WebSocket" : "SSE", "label")}
        </div>
      </div>
      <div class="chat-header-actions">
        <button class="button secondary" type="button" data-action="load-chat">刷新历史</button>
        <button class="button secondary" type="button" data-action="chat-live-open">Live</button>
      </div>
    </header>
  `;
}

function renderWelcomeView({ chatboxMode = false } = {}) {
  return `
    <section class="chat-welcome-view">
      ${uiState({
        state: "empty",
        title: chatboxMode ? "ChatBox ready" : "Ask AstrBot",
        message: "选择历史会话或发送第一条消息。",
        compact: true,
      })}
      <div class="chat-welcome-actions">
        <button class="button secondary" type="button" data-action="chat-new-session">新对话</button>
        <button class="button ghost" type="button" data-action="chat-live-open">Live Mode</button>
      </div>
    </section>
  `;
}

function renderProjectView(project, { chatboxMode = false } = {}) {
  const sessions = projectSessions();
  return `
    <section class="chat-project-view" data-project-view="${escapeHtml(project.project_id)}">
      <header class="chat-project-view-header">
        <div>
          <div class="chat-project-view-title">
            <span>${escapeHtml(project.emoji || "📁")}</span>
            <h2>${escapeHtml(project.title || project.project_id)}</h2>
          </div>
          ${project.description ? `<p>${escapeHtml(project.description)}</p>` : ""}
        </div>
        <button class="button ghost" type="button" data-action="chat-clear-project">退出项目</button>
      </header>
      <div class="chat-project-input-slot">
        ${renderChatInput({ chatboxMode })}
      </div>
      <div class="chat-project-session-list">
        ${sessions.length ? sessions.map((session) => `
          <article class="chat-project-session-item">
            <button type="button" data-action="project-session-select" data-project="${escapeHtml(project.project_id)}" data-session="${escapeHtml(session.session_id)}">
              <strong>${escapeHtml(session.display_name || session.session_id)}</strong>
              <small>${escapeHtml(session.updated_at || "")}</small>
            </button>
            <div class="chat-session-actions">
              <button class="button icon ghost" type="button" data-action="chat-rename-open" data-session="${escapeHtml(session.session_id)}" data-title="${escapeHtml(session.display_name || "")}" aria-label="Rename">✎</button>
              <button class="button icon ghost" type="button" data-action="project-session-remove" data-project="${escapeHtml(project.project_id)}" data-session="${escapeHtml(session.session_id)}" aria-label="Remove">×</button>
            </div>
          </article>
        `).join("") : `
          <div class="chat-project-empty">
            <strong>暂无项目会话</strong>
            <span>发送第一条消息后会自动创建会话并加入此项目。</span>
          </div>
        `}
      </div>
    </section>
  `;
}

function renderChatInput({ chatboxMode = false } = {}) {
  return `
    <section class="chat-input-area">
      ${state.chat.replyTo ? `
        <div class="chat-reply-preview">
          <span>Reply #${escapeHtml(state.chat.replyTo.messageId)}</span>
          <strong>${escapeHtml(state.chat.replyTo.selectedText || "")}</strong>
          <button class="button icon ghost" type="button" data-action="chat-reply-clear" aria-label="Clear reply">×</button>
        </div>
      ` : ""}
      <div class="chat-input-container">
        ${chatboxMode ? `
          <div class="form-row chat-openapi-secret">
            <label for="openapi-secret">OpenAPI Secret</label>
            <input id="openapi-secret" type="password" value="${escapeHtml(state.openApiSecretDraft || openApiSecret())}" />
          </div>
        ` : ""}
        <div class="chat-context-row">
          <label>Conversation <input id="conversation-id" value="${escapeHtml(state.chat.conversationId)}" /></label>
          <label>Sender <input id="sender-id" value="${escapeHtml(state.chat.senderId)}" /></label>
          ${renderConfigSelector()}
          <button class="button secondary" type="button" data-action="chat-config-apply">Apply config</button>
          ${renderProviderModelMenu()}
        </div>
        <textarea id="chat-text" class="chat-textarea" spellcheck="false" placeholder="Ask AstrBot...">${escapeHtml(state.chat.text)}</textarea>
        <div class="chat-attachment-toolbar">
          <input id="chat-attachment-url" placeholder="https://example.test/image.png or file URL" value="" />
          <button class="button secondary" type="button" data-action="chat-stage-url">添加 URL</button>
          <input id="chat-file-upload" type="file" multiple />
          <button class="button secondary" type="button" data-action="chat-upload-file">上传</button>
          <label class="check-row"><input id="chat-enable-streaming" type="checkbox" ${state.chat.enableStreaming === false ? "" : "checked"} /> streaming</label>
        </div>
        ${renderAttachmentPreview()}
        <details class="chat-message-parts-editor" ${state.chat.messagePartsJson ? "open" : ""}>
          <summary>Message Parts JSON</summary>
          <textarea id="chat-message-parts" class="json-editor" spellcheck="false" placeholder='[{"type":"reply","message_id":"42","selected_text":"quoted"},{"type":"plain","text":"hello"}]'>${escapeHtml(state.chat.messagePartsJson || "")}</textarea>
        </details>
        <div class="chat-send-row">
          <button class="button" type="button" data-action="send-chat"><span class="button-icon">➤</span>发送</button>
          <button class="button secondary" type="button" data-action="chat-stop">Stop</button>
          ${chatboxMode ? `<button class="button secondary" type="button" data-action="openapi-stream-chat">OpenAPI Stream</button>` : ""}
          <button class="button ghost" type="button" data-action="load-chat">读取历史</button>
        </div>
        ${chatboxMode ? renderOpenApiControls() : ""}
      </div>
    </section>
  `;
}

function renderConfigSelector() {
  const options = configOptions();
  const selected = state.chat.selectedConfigId || state.selectedConfigId || "default";
  return `
    <label>Config
      <select id="chat-config-select">
        ${options.map((config) => `<option value="${escapeHtml(config.id)}" ${config.id === selected ? "selected" : ""}>${escapeHtml(config.name || config.id)}</option>`).join("")}
      </select>
    </label>
  `;
}

function renderProviderModelMenu() {
  const providers = providerOptions();
  const selectedProvider = state.chat.selectedProviderId || providers[0]?.id || "";
  const activeProvider = providers.find((provider) => provider.id === selectedProvider) || providers[0] || {};
  const modelOptions = [...new Set([
    activeProvider.model,
    ...(activeProvider.models || []),
    ...(state.providerModels || []),
  ].filter(Boolean))];
  const selectedModel = state.chat.selectedModelName || activeProvider.model || modelOptions[0] || "";
  return `
    <label>Provider
      <select id="chat-provider-select">
        <option value="">Default</option>
        ${providers.map((provider) => `<option value="${escapeHtml(provider.id)}" ${provider.id === selectedProvider ? "selected" : ""}>${escapeHtml(provider.id)}${provider.model ? ` · ${escapeHtml(provider.model)}` : ""}</option>`).join("")}
      </select>
    </label>
    <label>Model
      <select id="chat-model-select">
        <option value="">Default</option>
        ${modelOptions.map((model) => `<option value="${escapeHtml(model)}" ${model === selectedModel ? "selected" : ""}>${escapeHtml(model)}</option>`).join("")}
      </select>
    </label>
  `;
}

function renderAttachmentPreview() {
  const attachments = state.chat.stagedAttachments || [];
  if (!attachments.length && !state.chat.imageUrls) return "";
  return `
    <div class="chat-attachment-preview">
      ${attachments.map((attachment, index) => renderAttachmentChip(attachment, index)).join("")}
      ${state.chat.imageUrls ? state.chat.imageUrls.split(/\r?\n|,/).map((url) => url.trim()).filter(Boolean).map((url, index) => renderAttachmentChip({ type: "image", url, name: url }, `legacy-${index}`)).join("") : ""}
    </div>
  `;
}

function renderAttachmentChip(attachment, index) {
  const url = attachment.url || attachment.embedded_url || attachment.file_url || "";
  const isImage = (attachment.type || "").includes("image") || /\.(png|jpg|jpeg|gif|webp)$/i.test(url);
  return `
    <div class="chat-attachment-chip">
      ${isImage && url ? `<img src="${escapeHtml(url)}" alt="" loading="lazy" />` : `<span class="tag">${escapeHtml(attachment.type || "file")}</span>`}
      <span>${escapeHtml(attachment.original_name || attachment.name || attachment.filename || url || attachment.attachment_id || "attachment")}</span>
      ${url ? `<button class="button icon ghost" type="button" data-action="chat-image-preview" data-url="${escapeHtml(url)}" aria-label="Preview">⤢</button>` : ""}
      <button class="button icon ghost" type="button" data-action="chat-attachment-remove" data-index="${escapeHtml(index)}" aria-label="Remove">×</button>
    </div>
  `;
}

function renderOpenApiControls() {
  const subscriptions = state.realtime?.subscriptions || [];
  const elicitations = state.realtime?.elicitations || [];
  const requestId = state.realtime?.last?.request_id || subscriptions[0]?.request_id || state.chat.conversationId;
  return `
    <div class="chat-openapi-panel">
      <div class="panel-title-row">
        <h3>Realtime</h3>
        <button class="button ghost" type="button" data-action="openapi-realtime-refresh">刷新</button>
      </div>
      ${state.realtime?.unavailable ? `<p class="empty">${escapeHtml(state.realtime.unavailable)}</p>` : ""}
      <div class="chat-context-row">
        <label>Request ID <input id="openapi-request-id" value="${escapeHtml(requestId)}" /></label>
        <button class="button secondary" type="button" data-action="openapi-stop-chat">Stop</button>
        <button class="button ghost" type="button" data-action="openapi-subscription-status">Status</button>
      </div>
      <div class="chat-openapi-grid">
        <div>
          ${subscriptions.length ? subscriptions.map((item) => `
            <div class="status-item"><span>${escapeHtml(item.request_id)}</span><strong>${escapeHtml(item.status || "queued")}</strong></div>
          `).join("") : `<p class="empty compact">暂无 subscription。</p>`}
        </div>
        <div>
          <div class="chat-context-row">
            <label>Elicitation ID <input id="elicitation-id" value="${escapeHtml(elicitations[0]?.elicitation_id || "approval-1")}" /></label>
            <label>Message <input id="elicitation-message" value="Approve action?" /></label>
            <label class="check-row"><input id="elicitation-confirmed" type="checkbox" checked /> confirmed</label>
          </div>
          <div class="actions">
            <button class="button secondary" type="button" data-action="openapi-elicitation-create">Create</button>
            <button class="button" type="button" data-action="openapi-elicitation-respond">Respond</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

function renderLiveMode() {
  const live = state.chat.live || {};
  const metrics = live.metrics || {};
  const liveMessages = live.messages || [];
  return `
    <section class="chat-live-mode">
      <div class="chat-live-controls">
        <button class="button icon ghost" type="button" data-action="chat-live-close" aria-label="Close">×</button>
        <button class="button secondary" type="button" data-action="chat-live-connect">Connect</button>
        <button class="button secondary" type="button" data-action="chat-live-speaking-start">Start speaking</button>
        <button class="button secondary" type="button" data-action="chat-live-speaking-end">End speaking</button>
        <button class="button ghost" type="button" data-action="chat-live-disconnect">Disconnect</button>
      </div>
      <div class="chat-live-orb" data-live-status="${escapeHtml(live.status || "idle")}">
        <span>${escapeHtml(live.statusText || "Astr Live")}</span>
      </div>
      <div class="grid cols-3">
        ${metric("WS", live.status || "idle", "/api/live_chat/ws")}
        ${metric("STT", metrics.stt || "-", "provider")}
        ${metric("TTS", metrics.tts || "-", "provider")}
      </div>
      <div class="messages chat-live-messages">
        ${liveMessages.length ? liveMessages.map((message, index) => `
          <article class="message ${message.type === "user" ? "user" : ""}">
            <div class="message-meta">${escapeHtml(message.type || "live")} · #${index + 1}</div>
            <p>${escapeHtml(message.text || "")}</p>
          </article>
        `).join("") : `<p class="empty">LiveMode 等待音频或 WebSocket 事件。</p>`}
      </div>
    </section>
  `;
}

function renderMessage(message, index) {
  const normalized = normalizeMessage(message, index);
  return `
    <article class="message ${normalized.role === "user" ? "user" : ""}" id="chat-message-${escapeHtml(normalized.id)}">
      <div class="message-meta">${escapeHtml(normalized.role)} · #${index + 1}${normalized.createdAt ? ` · ${escapeHtml(normalized.createdAt)}` : ""}</div>
      ${normalized.reasoning ? renderReasoning(normalized.reasoning) : ""}
      <div class="message-parts">
        ${renderMessageParts(normalized.parts)}
      </div>
      <div class="message-actions">
        <button class="button icon ghost" type="button" data-action="chat-reply" data-message-index="${index}" data-message-id="${escapeHtml(normalized.id)}" aria-label="Reply">↩</button>
        ${normalized.refs.length ? `<button class="button secondary" type="button" data-action="chat-refs-open" data-message-index="${index}">Refs ${normalized.refs.length}</button>` : ""}
        ${normalized.agentStats ? `<span class="tag">tokens ${escapeHtml(normalized.agentStats.token_usage?.output || normalized.agentStats.tokens || "-")}</span>` : ""}
      </div>
    </article>
  `;
}

function renderMessageParts(parts) {
  const rendered = [];
  let toolCalls = [];
  const flushToolCalls = () => {
    if (!toolCalls.length) return;
    rendered.push(renderToolCallGroup(toolCalls));
    toolCalls = [];
  };
  for (const part of parts) {
    if (part?.type === "tool_call" && Array.isArray(part.tool_calls)) {
      for (const toolCall of part.tool_calls) {
        if (isIPythonTool(toolCall)) {
          flushToolCalls();
          rendered.push(renderIPythonTool(toolCall));
        } else {
          toolCalls.push(toolCall);
        }
      }
      continue;
    }
    flushToolCalls();
    rendered.push(renderMessagePart(part));
  }
  flushToolCalls();
  return rendered.join("") || `<span class="empty">empty message</span>`;
}

function renderMessagePart(part) {
  if (!part || typeof part !== "object") {
    return `<p>${escapeHtml(part || "")}</p>`;
  }
  if (part.type === "plain") {
    return `<div class="message-markdown">${markdownViewer({ markdown: part.text || "", emptyMessage: "empty text" })}</div>`;
  }
  if (part.type === "image") {
    const url = part.embedded_url || part.url || part.image_url || attachmentUrl(part);
    return `<figure class="message-media"><img src="${escapeHtml(url)}" alt="" loading="lazy" data-action="chat-image-preview" data-url="${escapeHtml(url)}" /><figcaption>${escapeHtml(part.filename || url)}</figcaption></figure>`;
  }
  if (part.type === "reply") {
    return `<blockquote><strong>Reply ${escapeHtml(part.message_id || "")}</strong><br>${escapeHtml(part.selected_text || "")}</blockquote>`;
  }
  if (part.type === "record") {
    const url = part.embedded_url || part.url || part.record_url || attachmentUrl(part);
    return `<div class="message-audio"><audio controls src="${escapeHtml(url)}"></audio></div>`;
  }
  if (part.type === "video") {
    const url = part.embedded_url || part.url || part.video_url || attachmentUrl(part);
    return `<video class="message-video" controls src="${escapeHtml(url)}"></video>`;
  }
  if (part.type === "file") {
    const file = part.embedded_file || {};
    const url = file.url || part.url || part.file_url || attachmentUrl(part);
    const label = file.filename || part.name || part.filename || part.type;
    return `<p><span class="tag">${escapeHtml(part.type)}</span> <a href="${escapeHtml(url)}" target="_blank" rel="noreferrer">${escapeHtml(label)}</a></p>`;
  }
  if (part.type === "elicitation") {
    const payload = part.payload || {};
    return `
      <section class="message-elicitation">
        <strong>${escapeHtml(payload.message || payload.request?.message || "Elicitation")}</strong>
        ${payload.requested_schema || payload.request?.requested_schema ? jsonBlock(payload.requested_schema || payload.request.requested_schema) : ""}
        <div class="actions">
          <button class="button secondary" type="button" data-action="chat-elicitation-respond" data-elicitation="${escapeHtml(payload.elicitation_id || part.elicitation_id || "")}" data-result="decline">Decline</button>
          <button class="button" type="button" data-action="chat-elicitation-respond" data-elicitation="${escapeHtml(payload.elicitation_id || part.elicitation_id || "")}" data-result="accept">Accept</button>
        </div>
      </section>
    `;
  }
  if (part.type === "action_ref" || part.type === "refs") {
    const refs = part.refs || part.items || [];
    return `<button class="button secondary" type="button" data-action="chat-part-refs-open" data-refs="${escapeHtml(JSON.stringify(refs))}">Action refs ${Array.isArray(refs) ? refs.length : ""}</button>`;
  }
  return `<p><span class="tag">${escapeHtml(part.type || "part")}</span> ${escapeHtml(JSON.stringify(part))}</p>`;
}

function renderReasoning(reasoning) {
  return `
    <details class="reasoning-block" open>
      <summary>Reasoning</summary>
      <div class="reasoning-markdown">${markdownViewer({ markdown: reasoning, emptyMessage: "empty reasoning" })}</div>
    </details>
  `;
}

function renderToolCallGroup(toolCalls) {
  return `
    <section class="tool-call-compact">
      ${toolCalls.map((toolCall) => `
        <details class="tool-call-item">
          <summary>${toolIcon(toolCall.name)} ${escapeHtml(toolCall.name || "tool")} ${toolCall.finished_ts ? `<span>${escapeHtml(formatToolDuration(toolCall))}</span>` : ""}</summary>
          <div class="tool-call-detail-row"><span>ID</span><code>${escapeHtml(toolCall.id || "")}</code></div>
          <div class="tool-call-detail-row"><span>Args</span>${jsonBlock(toolCall.args || {})}</div>
          ${toolCall.result ? `<div class="tool-call-detail-row"><span>Result</span>${jsonBlock(formatJsonMaybe(toolCall.result))}</div>` : ""}
        </details>
      `).join("")}
    </section>
  `;
}

function renderIPythonTool(toolCall) {
  const code = toolCall.args?.code || toolCall.args || "";
  return `
    <details class="ipython-tool-block" open>
      <summary>IPython · ${escapeHtml(toolCall.name || "python")}</summary>
      <pre><code>${escapeHtml(typeof code === "string" ? code : JSON.stringify(code, null, 2))}</code></pre>
      ${toolCall.result ? `<pre class="tool-result">${escapeHtml(typeof toolCall.result === "string" ? toolCall.result : JSON.stringify(toolCall.result, null, 2))}</pre>` : ""}
    </details>
  `;
}

function renderRefsSidebar() {
  if (!state.chat.refsSidebarOpen) return "";
  const refs = state.chat.selectedRefs || [];
  return `
    <aside class="chat-refs-sidebar">
      <div class="panel-title-row">
        <h2>References</h2>
        <button class="button icon ghost" type="button" data-action="chat-refs-close" aria-label="Close refs">×</button>
      </div>
      ${refs.length ? refs.map((ref, index) => `
        <article class="ref-card">
          <strong>${escapeHtml(ref.title || ref.name || ref.url || `Ref ${index + 1}`)}</strong>
          <p>${escapeHtml(ref.content || ref.snippet || ref.text || "")}</p>
          ${ref.url ? `<a href="${escapeHtml(ref.url)}" target="_blank" rel="noreferrer">${escapeHtml(ref.url)}</a>` : ""}
        </article>
      `).join("") : `<p class="empty">暂无引用。</p>`}
    </aside>
  `;
}

function renderChatDialogs() {
  return `
    ${dialog({
      id: "chat-rename-dialog",
      title: "重命名会话",
      open: state.chat.dialog === "rename",
      body: `
        <div class="form-row"><label>Session ID</label><input id="chat-rename-session-id" value="${escapeHtml(state.chat.renameSessionId || "")}" /></div>
        <div class="form-row"><label>Title</label><input id="chat-rename-title" value="${escapeHtml(state.chat.renameTitle || "")}" /></div>
      `,
      actions: [
        { label: "取消", variant: "ghost", action: "chat-dialog-close" },
        { label: "保存", action: "chat-rename-save" },
      ],
    })}
    ${dialog({
      id: "chat-project-dialog",
      title: "Chat 项目",
      open: state.chat.dialog === "project",
      maxWidth: "620px",
      body: renderProjectDialogBody(),
      actions: [
        { label: "关闭", variant: "ghost", action: "chat-dialog-close" },
      ],
    })}
    ${dialog({
      id: "chat-image-preview-dialog",
      title: "Image Preview",
      open: state.chat.dialog === "image-preview",
      maxWidth: "860px",
      body: state.chat.previewImageUrl ? `<img class="chat-preview-image" src="${escapeHtml(state.chat.previewImageUrl)}" alt="" />` : "",
      actions: [{ label: "关闭", variant: "ghost", action: "chat-dialog-close" }],
    })}
  `;
}

function renderProjectDialogBody() {
  const targetId = state.chat.projectDialogTargetId || state.activeProjectId || "";
  const active = chatProjects().find((project) => project.project_id === targetId) || {};
  const editing = (state.chat.projectDialogMode || "create") === "edit" && active.project_id;
  return `
    <input id="project-actor" value="user" hidden />
    <input id="project-id" value="${escapeHtml(editing ? active.project_id : "")}" hidden />
    <div class="form-grid cols-2">
      <div class="form-row"><label>Emoji</label><input id="project-emoji" value="${escapeHtml(active.emoji || "📁")}" /></div>
      <div class="form-row"><label>Name</label><input id="project-title" value="${escapeHtml(editing ? active.title : "")}" placeholder="Project name" autofocus /></div>
    </div>
    <div class="form-row"><label>Description</label><textarea id="project-description">${escapeHtml(active.description || "")}</textarea></div>
    <div class="actions">
      <button id="project-dialog-save" class="button" type="button" data-action="project-dialog-save" ${editing ? "" : "disabled"}>${editing ? "保存项目" : "创建项目"}</button>
      ${editing ? `<button class="button danger" type="button" data-action="project-delete" data-project="${escapeHtml(active.project_id)}">删除</button>` : ""}
    </div>
  `;
}

function renderChatProjectItem(project) {
  const active = project.project_id === state.activeProjectId;
  return `
    <article class="chat-project-item ${active ? "active" : ""}">
      <button type="button" class="chat-project-select" data-action="project-select" data-project="${escapeHtml(project.project_id)}">
        <span>${escapeHtml(project.emoji || "📁")}</span>
        <strong>${escapeHtml(project.title || project.project_id)}</strong>
      </button>
      <div class="chat-project-actions">
        <button class="button icon ghost" type="button" data-action="chat-project-dialog-open" data-mode="edit" data-project="${escapeHtml(project.project_id)}" aria-label="Edit project">✎</button>
        <button class="button icon ghost" type="button" data-action="project-delete" data-project="${escapeHtml(project.project_id)}" aria-label="Delete project">×</button>
      </div>
    </article>
  `;
}

function normalizeMessage(message, index) {
  const content = message.content || {};
  const role = content.type || message.type || message.role || message.sender || "assistant";
  const parts = Array.isArray(content.message)
    ? content.message
    : Array.isArray(message.message_parts)
      ? message.message_parts
      : [
        ...(message.text ? [{ type: "plain", text: message.text }] : []),
        ...((message.image_urls || []).map((url) => ({ type: "image", url }))),
      ];
  return {
    id: message.id || message.message_id || index + 1,
    role: role === "bot" ? "assistant" : role,
    parts,
    reasoning: content.reasoning || message.reasoning || "",
    refs: content.refs || message.refs || [],
    agentStats: content.agentStats || content.agent_stats || message.agentStats || null,
    createdAt: message.created_at || message.timestamp || "",
  };
}

function chatSessions() {
  const data = state.chatSessions?.data || state.chatSessions?.sessions || state.chatSessions || [];
  const sessions = Array.isArray(data) ? data : [];
  const normalized = sessions.map((session) => ({
    session_id: session.session_id || session.conversation_id || session.id,
    display_name: session.display_name || session.title || session.name || session.session_id || session.conversation_id,
    platform_id: session.platform_id || "webchat",
    updated_at: session.updated_at || session.last_active_at || "",
  })).filter((session) => session.session_id);
  if (state.chat.conversationId && !normalized.some((session) => session.session_id === state.chat.conversationId)) {
    normalized.unshift({
      session_id: state.chat.conversationId,
      display_name: state.chat.conversationId,
      platform_id: "webchat",
      updated_at: "",
    });
  }
  return normalized;
}

function chatProjects() {
  return state.projects?.projects || state.projects?.data || [];
}

function currentChatProject() {
  return chatProjects().find((project) => project.project_id === state.activeProjectId) || null;
}

function projectSessions() {
  const payload = state.projectSessions?.data || state.projectSessions || {};
  return Array.isArray(payload.sessions)
    ? payload.sessions
    : Array.isArray(payload)
      ? payload
      : [];
}

function configOptions() {
  const list = state.configAbconfs?.data?.info_list || state.configAbconfs?.info_list || [];
  const normalized = list.length ? list : [{ id: "default", name: "default" }];
  return normalized.map((config) => ({
    id: config.id || "default",
    name: config.name || config.id || "default",
  }));
}

function providerOptions() {
  const legacy = state.chatProviderList?.data || state.chatProviderList || [];
  if (Array.isArray(legacy) && legacy.length) return legacy.map(normalizeProviderOption).filter(Boolean);
  const catalog = state.providerCatalog || {};
  const providers = catalog.providers || catalog.chat_providers || catalog.data?.providers || [];
  return providers.map(normalizeProviderOption).filter(Boolean);
}

function normalizeProviderOption(provider) {
  const category = provider.provider_type || provider.category || provider.type || "chat_completion";
  if (category !== "chat_completion" && category !== "openai" && provider.provider_type) return null;
  const id = provider.id || provider.provider_id || provider.name || [provider.provider, provider.model].filter(Boolean).join("/");
  if (!id) return null;
  return {
    ...provider,
    id,
    model: provider.model || provider.model_name || "",
    models: provider.models || provider.supported_models || [],
  };
}

function attachmentUrl(part) {
  return part.attachment_id ? `/api/chat/get_attachment?attachment_id=${encodeURIComponent(part.attachment_id)}` : "";
}

function isIPythonTool(toolCall = {}) {
  return toolCall.name === "astrbot_execute_ipython" || toolCall.name === "astrbot_execute_python";
}

function toolIcon(name = "") {
  if (name.includes("web_search") || name.includes("tavily")) return "⌕";
  if (name === "astrbot_execute_shell") return "⌘";
  return "⚙";
}

function formatToolDuration(toolCall) {
  if (!toolCall.ts || !toolCall.finished_ts) return "";
  const seconds = Number(toolCall.finished_ts) - Number(toolCall.ts);
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  return seconds < 1 ? `${Math.round(seconds * 1000)}ms` : `${seconds.toFixed(1)}s`;
}

function formatJsonMaybe(value) {
  if (typeof value !== "string") return value;
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

export function renderSessions() {
  const payload = sessionPayload();
  const allRules = payload.rules;
  const visibleRules = allRules.filter((rule) => sessionRuleMatches(rule, state.sessionFilter));
  const groups = sessionGroups();
  const selected = new Set(state.sessionSelectedUmos || []);
  const activeUmo = state.activeUmo || visibleRules[0]?.umo || sessionAvailableUmos(visibleRules, groups)[0] || "webchat:GroupMessage:demo";
  const selectedCount = visibleRules.filter((rule) => selected.has(rule.umo)).length;
  const page = payload.page || state.sessionPage || 1;
  const pageSize = payload.pageSize || state.sessionPageSize || 10;
  const total = payload.total ?? visibleRules.length;
  return `
    <div class="grid cols-3">
      ${metric("Rule Sets", total, `${visibleRules.length} visible`)}
      ${metric("Selected", selected.size, "batch target")}
      ${metric("Active UMO", activeUmo, "form target")}
    </div>
    <section class="panel session-management-page" data-page="session-management">
        <div class="panel-title-row">
        <h2>Session Rules</h2>
          <div class="actions">
          ${selected.size ? `<button class="button danger" type="button" data-action="session-batch-delete-open">批量删除 ${selected.size}</button>` : ""}
          <button class="button secondary" type="button" data-action="session-add-rule-open">新增规则</button>
            <button class="button ghost" type="button" data-action="load-sessions">刷新</button>
          </div>
        </div>
      <div class="form-grid cols-4">
          <div class="form-row"><label>Filter</label><input id="session-filter" value="${escapeHtml(state.sessionFilter)}" placeholder="UMO / provider / persona" /></div>
          <div class="form-row"><label>Active UMO</label><input id="session-active-umo" value="${escapeHtml(activeUmo)}" /></div>
        <div class="form-row"><label>Page size</label><select id="session-page-size">${[10, 20, 50, 100].map((size) => `<option value="${size}" ${Number(pageSize) === size ? "selected" : ""}>${size}</option>`).join("")}</select></div>
        <div class="form-row align-end"><button class="button secondary" type="button" data-action="session-filter">筛选</button></div>
        </div>
      ${payload.unavailable ? uiState({ state: "error", message: payload.unavailable, compact: true }) : ""}
        ${visibleRules.length ? `
        <div class="table-scroll">
          <table class="table ui-data-table session-rules-table">
            <thead>
              <tr>
                <th><input type="checkbox" data-action="session-select-all" ${visibleRules.length && selectedCount === visibleRules.length ? "checked" : ""} /></th>
                <th>UMO 信息</th>
                <th>规则概览</th>
                <th>Service</th>
                <th>Provider</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              ${visibleRules.map((rule) => renderSessionRuleRow(rule, selected, activeUmo)).join("")}
            </tbody>
          </table>
        </div>
        <footer class="ui-table-footer">
          <span>Page ${escapeHtml(page)}</span>
          <span>${escapeHtml(pageSize)} per page</span>
          <strong>${escapeHtml(total)} total</strong>
          <button class="button ghost" type="button" data-action="session-page-prev" ${Number(page) <= 1 ? "disabled" : ""}>上一页</button>
          <button class="button ghost" type="button" data-action="session-page-next" ${Number(page) * Number(pageSize) >= Number(total) ? "disabled" : ""}>下一页</button>
        </footer>
      ` : uiState({ state: "empty", title: "暂无规则", message: "没有匹配的 Session rule；可从 active UMO 新增规则。", action: { label: "新增规则", action: "session-add-rule-open", variant: "secondary" } })}
      </section>
    <div class="grid cols-2">
      <section class="panel">
        <div class="panel-title-row">
          <h2>批量操作</h2>
          <span class="tag">selected / all / group / private / custom_group</span>
        </div>
        <div class="form-grid cols-4">
          <div class="form-row"><label>Scope</label><select id="session-batch-scope">${sessionBatchScopeOptions(groups).map((option) => `<option value="${escapeHtml(option.value)}" ${option.value === (state.sessionBatchScope || "selected") ? "selected" : ""}>${escapeHtml(option.label)}</option>`).join("")}</select></div>
          <div class="form-row"><label>LLM</label><select id="session-batch-llm"><option value="">不修改</option><option value="true" ${state.sessionBatchLlm === "true" ? "selected" : ""}>启用</option><option value="false" ${state.sessionBatchLlm === "false" ? "selected" : ""}>禁用</option></select></div>
          <div class="form-row"><label>TTS</label><select id="session-batch-tts"><option value="">不修改</option><option value="true" ${state.sessionBatchTts === "true" ? "selected" : ""}>启用</option><option value="false" ${state.sessionBatchTts === "false" ? "selected" : ""}>禁用</option></select></div>
          <div class="form-row"><label>Chat Provider</label>${renderSessionProviderSelect("session-batch-chat-provider", state.sessionBatchChatProvider || "", payload.availableChatProviders, true)}</div>
          <div class="form-row"><label>TTS Provider</label>${renderSessionProviderSelect("session-batch-tts-provider", state.sessionBatchTtsProvider || "", payload.availableTtsProviders, true)}</div>
        </div>
        <div class="actions mt-16">
          <button class="button" type="button" data-action="session-batch-apply">应用批量修改</button>
        </div>
      </section>
      <section class="panel">
        <div class="panel-title-row">
          <h2>分组管理</h2>
          <div class="actions">
            ${selected.size && groups.length ? groups.map((group) => `<button class="button ghost" type="button" data-action="session-group-add-selected" data-group="${escapeHtml(group.id)}">加入 ${escapeHtml(group.name)}</button>`).join("") : ""}
            <button class="button secondary" type="button" data-action="session-group-create-open">新建分组</button>
          </div>
        </div>
        ${groups.length ? `
          <table class="table">
            <thead><tr><th>ID</th><th>名称</th><th>UMO</th><th>操作</th></tr></thead>
            <tbody>
              ${groups.map((group) => `
                <tr>
                  <td>${escapeHtml(group.id)}</td>
                  <td><strong>${escapeHtml(group.name)}</strong><br><span class="metric-label">${escapeHtml(group.umo_count ?? group.umos?.length ?? 0)} 个会话</span></td>
                  <td>${(group.umos || []).slice(0, 5).map((umo) => `<span class="tag">${escapeHtml(umo)}</span>`).join(" ") || "-"}</td>
                  <td class="button-cell">
                    <button class="button secondary" type="button" data-action="session-group-edit-open" data-group="${escapeHtml(group.id)}">编辑</button>
                    <button class="button ghost" type="button" data-action="session-group-delete-open" data-group="${escapeHtml(group.id)}">删除</button>
                  </td>
                </tr>
              `).join("")}
            </tbody>
          </table>
        ` : `<p class="empty">暂无分组。</p>`}
      </section>
    </div>
    ${renderSessionDialogs({ payload, visibleRules, groups, activeUmo, selected })}
    ${state.operation ? `<section class="panel"><h2>最近 Session 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

function sessionRuleMatches(rule, filter) {
  const value = (filter || "").trim().toLowerCase();
  if (!value) return true;
  const rules = sessionRuleRules(rule);
  return [
    rule.umo,
    rules.session_service_config?.custom_name,
    rules.session_service_config?.persona_id,
    rules.provider_perf_chat_completion,
    rules.provider_perf_speech_to_text,
    rules.provider_perf_text_to_speech,
    JSON.stringify(rules),
  ].some((item) => String(item || "").toLowerCase().includes(value));
}

function renderSessionRuleRow(rule, selected, activeUmo) {
  const rules = sessionRuleRules(rule);
  const service = rules.session_service_config || {};
  return `
    <tr class="${rule.umo === activeUmo ? "selected-row" : ""}">
      <td><input type="checkbox" data-action="session-select" data-umo="${escapeHtml(rule.umo)}" ${selected.has(rule.umo) ? "checked" : ""} /></td>
      <td>
        <strong>${escapeHtml(rule.umo)}</strong>
        ${rule.umo === activeUmo ? `<br>${pill("active", "ok")}` : ""}
        ${service.custom_name ? `<br><span class="metric-label">${escapeHtml(service.custom_name)}</span>` : ""}
        <div class="ui-chip-row compact">
          ${chip(rule.platform || "unknown", "label")}
          ${chip(rule.message_type || "unknown", "label")}
        </div>
      </td>
      <td>${sessionRuleOverview(rules)}</td>
      <td>
        <span class="tag">session ${formatSessionFlag(service.session_enabled, true)}</span>
        <span class="tag">llm ${formatSessionFlag(service.llm_enabled, true)}</span>
        <span class="tag">tts ${formatSessionFlag(service.tts_enabled, true)}</span>
        ${service.persona_id ? `<span class="tag">${escapeHtml(service.persona_id)}</span>` : ""}
      </td>
      <td>${sessionProviderEntries(rules).map(([label, value]) => `<span class="tag">${escapeHtml(label)} · ${escapeHtml(value)}</span>`).join(" ") || "-"}</td>
      <td class="button-cell">
        <button class="button secondary" type="button" data-action="session-rule-edit-open" data-umo="${escapeHtml(rule.umo)}">编辑</button>
        <button class="button ghost" type="button" data-action="session-quick-name-open" data-umo="${escapeHtml(rule.umo)}">名称</button>
        <button class="button ghost" type="button" data-action="session-rule-select" data-umo="${escapeHtml(rule.umo)}">设为当前</button>
        <button class="button danger" type="button" data-action="session-rule-delete-open" data-umo="${escapeHtml(rule.umo)}">删除</button>
      </td>
    </tr>
  `;
}

function renderSessionDialogs({ payload, visibleRules, groups, activeUmo, selected }) {
  const editUmo = state.sessionEditUmo || activeUmo;
  const editRule = visibleRules.find((rule) => rule.umo === editUmo) || normalizeSessionRule({ umo: editUmo, rules: {} });
  const editRules = sessionRuleRules(editRule);
  const service = {
    session_enabled: editRules.session_service_config?.session_enabled !== false,
    llm_enabled: editRules.session_service_config?.llm_enabled !== false,
    tts_enabled: editRules.session_service_config?.tts_enabled !== false,
    custom_name: editRules.session_service_config?.custom_name || "",
    persona_id: editRules.session_service_config?.persona_id || "",
  };
  const quickRule = visibleRules.find((rule) => rule.umo === state.sessionQuickNameTarget) || {};
  const deleteRule = visibleRules.find((rule) => rule.umo === state.sessionDeleteTarget) || {};
  const group = groups.find((item) => item.id === state.sessionGroupTargetId) || { id: "", name: "", umos: [] };
  const groupDraftUmos = state.sessionGroupDraftUmos?.length ? state.sessionGroupDraftUmos : (group.umos || []);
  return `
    ${dialog({
      id: "session-add-rule-dialog",
      title: "新增 Session Rule",
      open: state.sessionDialog === "add-rule",
      closeAction: "session-dialog-close",
      body: `
        <p class="ui-dialog-message">选择一个 active UMO 后进入规则编辑。</p>
        <div class="form-row"><label>UMO</label><input id="session-new-umo" value="${escapeHtml(activeUmo)}" list="session-available-umos" /></div>
        <datalist id="session-available-umos">${sessionAvailableUmos(visibleRules, groups).map((umo) => `<option value="${escapeHtml(umo)}"></option>`).join("")}</datalist>
      `,
      actions: [
        { label: "取消", variant: "ghost", action: "session-dialog-close" },
        { label: "下一步", action: "session-add-rule-next" },
      ],
    })}
    ${dialog({
      id: "session-rule-editor-dialog",
      title: "规则编辑",
      open: state.sessionDialog === "editor",
      maxWidth: "760px",
      closeAction: "session-dialog-close",
      kind: "session-rule-editor-modal",
      body: `
        <div class="form-row"><label>UMO</label><input id="session-editor-umo" value="${escapeHtml(editRule.umo || activeUmo)}" readonly /></div>
        <div class="grid cols-2 session-editor-grid">
          <section class="session-editor-section">
            <h3>Service Config</h3>
            <label class="check-row"><input id="session-editor-session-enabled" type="checkbox" ${service.session_enabled ? "checked" : ""} /> Session</label>
            <label class="check-row"><input id="session-editor-llm-enabled" type="checkbox" ${service.llm_enabled ? "checked" : ""} /> LLM</label>
            <label class="check-row"><input id="session-editor-tts-enabled" type="checkbox" ${service.tts_enabled ? "checked" : ""} /> TTS</label>
            <div class="form-row"><label>Custom name</label><input id="session-editor-custom-name" value="${escapeHtml(service.custom_name)}" /></div>
            <div class="form-row"><label>Persona</label>${renderSessionProviderSelect("session-editor-persona", service.persona_id, payload.availablePersonas.map((persona) => ({ id: persona.name, name: persona.name, model: persona.prompt })), true)}</div>
            <button class="button" type="button" data-action="session-rule-save-service">保存 Service</button>
          </section>
          <section class="session-editor-section">
            <h3>Provider Config</h3>
            <div class="form-row"><label>Chat</label>${renderSessionProviderSelect("session-editor-chat-provider", editRules.provider_perf_chat_completion || "", payload.availableChatProviders, true)}</div>
            <div class="form-row"><label>STT</label>${renderSessionProviderSelect("session-editor-stt-provider", editRules.provider_perf_speech_to_text || "", payload.availableSttProviders, true)}</div>
            <div class="form-row"><label>TTS</label>${renderSessionProviderSelect("session-editor-tts-provider", editRules.provider_perf_text_to_speech || "", payload.availableTtsProviders, true)}</div>
            <button class="button" type="button" data-action="session-rule-save-provider">保存 Provider</button>
          </section>
        </div>
        <div class="grid cols-2 session-editor-grid mt-16">
          <section class="session-editor-section">
            <h3>Plugin Config</h3>
            <div class="form-row"><label>Enabled plugins</label><textarea id="session-editor-enabled-plugins">${escapeHtml((editRules.session_plugin_config?.enabled_plugins || []).join("\n"))}</textarea></div>
            <div class="form-row"><label>Disabled plugins</label><textarea id="session-editor-disabled-plugins">${escapeHtml((editRules.session_plugin_config?.disabled_plugins || []).join("\n"))}</textarea></div>
            <div class="ui-chip-row compact">${payload.availablePlugins.map((plugin) => chip(plugin.display_name || plugin.name, "label")).join("")}</div>
            <button class="button secondary" type="button" data-action="session-rule-save-plugin">保存 Plugin</button>
          </section>
          <section class="session-editor-section">
            <h3>Knowledge Base Config</h3>
            <div class="form-row"><label>KB IDs</label><textarea id="session-editor-kb-ids">${escapeHtml((editRules.kb_config?.kb_ids || []).join("\n"))}</textarea></div>
            <div class="form-row"><label>Top K</label><input id="session-editor-kb-top-k" type="number" min="1" value="${escapeHtml(editRules.kb_config?.top_k ?? 5)}" /></div>
            <label class="check-row"><input id="session-editor-kb-rerank" type="checkbox" ${editRules.kb_config?.enable_rerank !== false ? "checked" : ""} /> Enable rerank</label>
            <div class="ui-chip-row compact">${payload.availableKbs.map((kb) => chip(`${kb.emoji || "KB"} ${kb.kb_name || kb.name || kb.kb_id}`, "label")).join("")}</div>
            <button class="button secondary" type="button" data-action="session-rule-save-kb">保存 KB</button>
          </section>
        </div>
      `,
      actions: [
        { label: "关闭", variant: "ghost", action: "session-dialog-close" },
      ],
    })}
    ${dialog({
      id: "session-quick-name-dialog",
      title: "快速编辑名称",
      open: state.sessionDialog === "quick-name",
      closeAction: "session-dialog-close",
      body: `
        <div class="form-row"><label>UMO</label><input id="session-quick-umo" value="${escapeHtml(state.sessionQuickNameTarget || "")}" readonly /></div>
        <div class="form-row"><label>Custom name</label><input id="session-quick-name" value="${escapeHtml(sessionRuleRules(quickRule).session_service_config?.custom_name || "")}" /></div>
        <div class="actions mt-16"><button class="button" type="button" data-action="session-quick-name-save">保存名称</button></div>
      `,
      actions: [
        { label: "取消", variant: "ghost", action: "session-dialog-close" },
      ],
    })}
    ${dialog({
      id: "session-delete-dialog",
      title: "删除规则",
      open: state.sessionDialog === "delete-rule",
      kind: "danger",
      closeAction: "session-dialog-close",
      body: `<p class="ui-dialog-message">确认删除 ${escapeHtml(deleteRule.umo || state.sessionDeleteTarget || "")} 的全部规则？</p>`,
      actions: [
        { label: "取消", variant: "ghost", action: "session-dialog-close" },
        { label: "删除", variant: "danger", action: "session-rule-delete-confirm" },
      ],
    })}
    ${dialog({
      id: "session-batch-delete-dialog",
      title: "批量删除规则",
      open: state.sessionDialog === "batch-delete",
      kind: "danger",
      closeAction: "session-dialog-close",
      body: `<p class="ui-dialog-message">确认删除 ${escapeHtml(String(selected.size))} 个 selected rule sets？</p>`,
      actions: [
        { label: "取消", variant: "ghost", action: "session-dialog-close" },
        { label: "批量删除", variant: "danger", action: "session-batch-delete-confirm" },
      ],
    })}
    ${dialog({
      id: "session-group-dialog",
      title: state.sessionGroupDialogMode === "edit" ? "编辑分组" : "新建分组",
      open: state.sessionDialog === "group",
      maxWidth: "760px",
      closeAction: "session-dialog-close",
      body: renderSessionGroupDialogBody(group, groupDraftUmos, sessionAvailableUmos(visibleRules, groups)),
      actions: [
        { label: "取消", variant: "ghost", action: "session-dialog-close" },
      ],
    })}
    ${dialog({
      id: "session-group-delete-dialog",
      title: "删除分组",
      open: state.sessionDialog === "group-delete",
      kind: "danger",
      closeAction: "session-dialog-close",
      body: `<p class="ui-dialog-message">确认删除分组 ${escapeHtml(group.name || state.sessionGroupTargetId || "")}？</p>`,
      actions: [
        { label: "取消", variant: "ghost", action: "session-dialog-close" },
        { label: "删除", variant: "danger", action: "session-group-delete-confirm" },
      ],
    })}
  `;
}

function renderSessionGroupDialogBody(group, draftUmos, availableUmos) {
  const selected = new Set(draftUmos || []);
  const unselected = availableUmos.filter((umo) => !selected.has(umo));
  const draftName = state.sessionGroupDraftName || group.name || "";
  return `
    <input id="session-group-id" value="${escapeHtml(group.id || "")}" hidden />
    <div class="form-row"><label>分组名称</label><input id="session-group-name" value="${escapeHtml(draftName)}" /></div>
    <div class="actions mt-16"><button class="button" type="button" data-action="session-group-save">保存分组</button></div>
    <div class="grid cols-2">
      <section>
        <h3>可选会话 (${unselected.length})</h3>
        <div class="transfer-list">
          ${unselected.length ? unselected.map((umo) => `<button class="button ghost transfer-item" type="button" data-action="session-group-add-umo" data-umo="${escapeHtml(umo)}">+ ${escapeHtml(formatUmoShort(umo))}</button>`).join("") : `<p class="empty compact">无可选会话</p>`}
        </div>
      </section>
      <section>
        <h3>已选会话 (${selected.size})</h3>
        <div class="transfer-list">
          ${draftUmos.length ? draftUmos.map((umo) => `<button class="button ghost transfer-item" type="button" data-action="session-group-remove-umo" data-umo="${escapeHtml(umo)}">- ${escapeHtml(formatUmoShort(umo))}</button>`).join("") : `<p class="empty compact">暂无成员</p>`}
        </div>
      </section>
    </div>
    <div class="form-row mt-16"><label>UMOs</label><textarea id="session-group-umos">${escapeHtml((draftUmos || []).join("\n"))}</textarea></div>
  `;
}

function sessionPayload() {
  const payload = state.sessions || {};
  const data = payload.data || payload;
  const rules = Array.isArray(data.rules) ? data.rules.map(normalizeSessionRule) : [];
  return {
    rules,
    total: data.total ?? rules.length,
    page: data.page ?? state.sessionPage ?? 1,
    pageSize: data.page_size ?? state.sessionPageSize ?? 10,
    availablePersonas: data.available_personas || [],
    availableChatProviders: normalizeSessionProviderOptions(data.available_chat_providers || providerOptions()),
    availableSttProviders: normalizeSessionProviderOptions(data.available_stt_providers || []),
    availableTtsProviders: normalizeSessionProviderOptions(data.available_tts_providers || []),
    availablePlugins: data.available_plugins || [],
    availableKbs: data.available_kbs || [],
    unavailable: payload.unavailable,
  };
}

function normalizeSessionRule(rule = {}) {
  const rules = { ...(rule.rules || {}) };
  if (rule.service) rules.session_service_config = rule.service;
  if (rule.plugin) rules.session_plugin_config = rule.plugin;
  if (rule.knowledge_base) rules.kb_config = rule.knowledge_base;
  for (const provider of rule.provider_preferences || []) {
    const key = sessionProviderPreferenceKey(provider.capability);
    if (key) rules[key] = provider.provider_id;
  }
  const parsed = parseSessionUmo(rule.umo || "");
  return {
    ...rule,
    umo: rule.umo || "",
    platform: rule.platform || parsed.platform,
    message_type: rule.message_type || parsed.messageType,
    session_id: rule.session_id || parsed.sessionId,
    rules,
  };
}

function sessionRuleRules(rule = {}) {
  return normalizeSessionRule(rule).rules || {};
}

function sessionProviderPreferenceKey(capability = "") {
  const normalized = String(capability).toLowerCase();
  if (normalized.includes("chat")) return "provider_perf_chat_completion";
  if (normalized.includes("speech_to_text")) return "provider_perf_speech_to_text";
  if (normalized.includes("text_to_speech")) return "provider_perf_text_to_speech";
  return "";
}

function sessionProviderEntries(rules = {}) {
  return [
    ["chat", rules.provider_perf_chat_completion],
    ["stt", rules.provider_perf_speech_to_text],
    ["tts", rules.provider_perf_text_to_speech],
  ].filter(([, value]) => value);
}

function sessionRuleOverview(rules = {}) {
  const chips = [];
  if (rules.session_service_config) chips.push(chip("Service", "ok"));
  if (rules.session_plugin_config) chips.push(chip("Plugin", "label"));
  if (rules.kb_config) chips.push(chip("KB", "label"));
  if (sessionProviderEntries(rules).length) chips.push(chip("Provider", "warn"));
  return chips.length ? `<div class="ui-chip-row compact">${chips.join("")}</div>` : "-";
}

function sessionGroups() {
  const payload = state.sessionGroups || {};
  const data = payload.data || payload;
  return Array.isArray(data.groups) ? data.groups : [];
}

function sessionAvailableUmos(rules = [], groups = []) {
  return Array.from(new Set([
    ...(state.sessionAvailableUmos || []),
    ...rules.map((rule) => rule.umo),
    ...groups.flatMap((group) => group.umos || []),
  ].filter(Boolean))).sort();
}

function sessionBatchScopeOptions(groups = []) {
  return [
    { value: "selected", label: "已选择" },
    { value: "all", label: "全部会话" },
    { value: "group", label: "群聊" },
    { value: "private", label: "私聊" },
    ...groups.map((group) => ({ value: `custom_group:${group.id}`, label: `分组：${group.name} (${group.umo_count ?? group.umos?.length ?? 0})` })),
  ];
}

function renderSessionProviderSelect(id, value, options = [], allowEmpty = false) {
  const normalized = normalizeSessionProviderOptions(options);
  return `<select id="${escapeHtml(id)}">${allowEmpty ? `<option value="" ${value ? "" : "selected"}>跟随配置文件</option>` : ""}${normalized.map((option) => `<option value="${escapeHtml(option.id)}" ${String(value || "") === option.id ? "selected" : ""}>${escapeHtml(option.name || option.id)}${option.model ? ` · ${escapeHtml(option.model)}` : ""}</option>`).join("")}</select>`;
}

function normalizeSessionProviderOptions(options = []) {
  return options.map((option) => {
    if (typeof option === "string") return { id: option, name: option, model: "" };
    const id = option.id || option.provider_id || option.value || option.name || "";
    return {
      id,
      name: option.name || option.label || id,
      model: option.model || option.model_name || "",
    };
  }).filter((option) => option.id);
}

function formatSessionFlag(value, fallback = true) {
  const enabled = value ?? fallback;
  return enabled ? "on" : "off";
}

function parseSessionUmo(umo = "") {
  const parts = String(umo).split(":");
  return {
    platform: parts[0] || "unknown",
    messageType: parts[1] || "unknown",
    sessionId: parts.slice(2).join(":") || parts[2] || umo,
  };
}

function formatUmoShort(umo = "") {
  const parsed = parseSessionUmo(umo);
  return `${parsed.platform}:${parsed.sessionId || umo}`;
}

export function renderProjects() {
  const projects = state.projects;
  const actor = "user";
  const allProjects = projects?.projects || [];
  const visibleProjects = allProjects.filter((project) => projectMatches(project, state.projectFilter));
  const activeProject = allProjects.find((project) => project.project_id === state.activeProjectId) || allProjects[0] || null;
  return `
    <div class="chat-projects-page" data-page="chat-projects">
      <div class="grid cols-3">
        ${metric("Projects", allProjects.length, `${visibleProjects.length} visible`)}
        ${metric("Loaded Sessions", state.projectSessions?.sessions?.length ?? 0, "selected project")}
        ${metric("Active Project", activeProject?.project_id || "-", actor)}
      </div>
      <div class="grid cols-2">
        <section class="panel">
          <div class="panel-title-row">
            <h2>项目</h2>
            <div class="actions">
              <button class="button secondary" type="button" data-action="project-filter">筛选</button>
              <button class="button ghost" type="button" data-action="load-projects">刷新</button>
            </div>
          </div>
          <div class="form-grid cols-2">
            <div class="form-row"><label>Filter</label><input id="project-filter" value="${escapeHtml(state.projectFilter)}" placeholder="title / id / description" /></div>
            <div class="form-row"><label>Active Project</label><input id="project-id" value="${escapeHtml(activeProject?.project_id || "")}" /></div>
          </div>
          ${projects?.unavailable ? `<p class="empty">${escapeHtml(projects.unavailable)}</p>` : ""}
          ${visibleProjects.length ? `
            <table class="table">
              <thead><tr><th>项目</th><th>描述</th><th>更新时间</th><th>操作</th></tr></thead>
              <tbody>
                ${visibleProjects.map((project) => `
                  <tr>
                    <td><strong>${escapeHtml(project.emoji || "P")} ${escapeHtml(project.title)}</strong><br><span class="metric-label">${escapeHtml(project.project_id)}</span>${project.project_id === activeProject?.project_id ? `<br>${pill("active", "ok")}` : ""}</td>
                    <td>${escapeHtml(project.description || "-")}</td>
                    <td>${escapeHtml(project.updated_at)}</td>
                    <td class="button-cell">
                      <button class="button secondary" type="button" data-action="project-select" data-project="${escapeHtml(project.project_id)}">选择</button>
                      <button class="button secondary" type="button" data-action="project-sessions-load" data-project="${escapeHtml(project.project_id)}">会话</button>
                      <button class="button secondary" type="button" data-action="project-update" data-project="${escapeHtml(project.project_id)}">更新</button>
                      <button class="button ghost" type="button" data-action="project-delete" data-project="${escapeHtml(project.project_id)}">删除</button>
                    </td>
                  </tr>
                `).join("")}
              </tbody>
            </table>
          ` : `<p class="empty">暂无项目，或后端未配置 chat project state。</p>`}
        </section>
        <section class="panel">
          <h2>创建 / 更新项目</h2>
          <div class="form-row"><label>Creator</label><input id="project-actor" value="${actor}" /></div>
          <div class="form-row"><label>Title</label><input id="project-title" value="${escapeHtml(activeProject?.title || "Research")}" /></div>
          <div class="form-row"><label>Emoji</label><input id="project-emoji" value="${escapeHtml(activeProject?.emoji || "folder")}" /></div>
          <div class="form-row"><label>Description</label><textarea id="project-description">${escapeHtml(activeProject?.description || "Project notes")}</textarea></div>
          <div class="actions">
            <button class="button" type="button" data-action="project-create">创建项目</button>
            <button class="button secondary" type="button" data-action="project-update">更新当前</button>
          </div>
        </section>
      </div>
      <div class="grid cols-2">
        <section class="panel">
          <h2>注册并绑定会话</h2>
          <div class="form-row"><label>Project ID</label><input id="project-session-project-id" value="${escapeHtml(activeProject?.project_id || "")}" /></div>
          <div class="form-row"><label>Session ID</label><input id="project-session-id" value="webchat-demo" /></div>
          <div class="form-row"><label>Display name</label><input id="project-session-name" value="WebChat demo" /></div>
          <label class="check-row"><input id="project-session-group" type="checkbox" /> Group session</label>
          <div class="actions mt-16">
            <button class="button secondary" type="button" data-action="project-session-upsert">注册会话</button>
            <button class="button" type="button" data-action="project-session-add">加入项目</button>
            <button class="button ghost" type="button" data-action="project-session-remove">移出项目</button>
          </div>
        </section>
        <section class="panel">
          <h2>项目会话</h2>
          ${state.projectSessions?.sessions?.length ? `
            <table class="table">
              <thead><tr><th>Session</th><th>平台</th><th>Creator</th><th>类型</th></tr></thead>
              <tbody>
                ${state.projectSessions.sessions.map((session) => `
                  <tr>
                    <td><strong>${escapeHtml(session.display_name || session.session_id)}</strong><br><span class="metric-label">${escapeHtml(session.session_id)}</span></td>
                    <td>${escapeHtml(session.platform_id)}</td>
                    <td>${escapeHtml(session.creator)}</td>
                    <td>${pill(session.is_group ? "group" : "direct")}</td>
                  </tr>
                `).join("")}
              </tbody>
            </table>
          ` : `<p class="empty">选择项目后读取会话，或先注册一个 session 并加入项目。</p>`}
        </section>
      </div>
      ${state.operation ? `<section class="panel"><h2>最近 Project 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
    </div>
  `;
}

function projectMatches(project, filter) {
  const value = (filter || "").trim().toLowerCase();
  if (!value) return true;
  return [project.project_id, project.title, project.description, project.emoji]
    .some((item) => String(item || "").toLowerCase().includes(value));
}
