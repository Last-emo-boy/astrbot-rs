import { api } from "../api.js";
import { $, showToast } from "../dom.js";
import { loadChatControls, loadMessages, loadProjectSessions, loadProjects } from "../loaders.js";
import { state } from "../state.js";
import { optionalText, projectActor, projectIdFrom } from "./forms.js";

export async function handleProjectActions({ action, target }) {
    if (action === "load-projects") {
      await loadProjects(projectActor());
      showToast("Chat 项目已刷新");
    }
    if (action === "project-filter") {
      state.projectFilter = $("#project-filter")?.value.trim() || "";
      state.activeProjectId = $("#project-id")?.value.trim() || state.activeProjectId;
      showToast("Chat 项目筛选已更新");
    }
    if (action === "project-select") {
      state.activeProjectId = target.dataset.project;
      state.chat.conversationId = "";
      state.chat.currentSessionProject = null;
      state.messages = [];
      await loadProjectSessions(projectActor(), target.dataset.project);
      showToast(`当前项目：${state.activeProjectId}`);
    }
    if (action === "project-dialog-save") {
      const projectId = $("#project-id")?.value.trim();
      const title = $("#project-title")?.value.trim() || "";
      if (!title) throw new Error("请填写项目名称");
      if (projectId) {
        state.operation = await api("/api/management/chat-projects/update", {
          method: "POST",
          body: JSON.stringify({
            actor: projectActor(),
            project_id: projectId,
            title,
            emoji: optionalText("#project-emoji"),
            description: optionalText("#project-description"),
            now: new Date().toISOString(),
          }),
        });
        state.activeProjectId = projectId;
        showToast("Chat 项目已更新");
      } else {
        state.operation = await api("/api/management/chat-projects/create", {
          method: "POST",
          body: JSON.stringify({
            creator: projectActor(),
            title,
            emoji: optionalText("#project-emoji") || "📁",
            description: optionalText("#project-description"),
            now: new Date().toISOString(),
          }),
        });
        state.activeProjectId = state.operation.project?.project_id || state.activeProjectId;
        showToast("Chat 项目已创建");
      }
      state.chat.dialog = "";
      await loadProjects(projectActor());
      if (state.activeProjectId) await loadProjectSessions(projectActor(), state.activeProjectId);
    }
    if (action === "project-create") {
      state.operation = await api("/api/management/chat-projects/create", {
        method: "POST",
        body: JSON.stringify({
          creator: projectActor(),
          title: $("#project-title").value.trim(),
          emoji: optionalText("#project-emoji"),
          description: optionalText("#project-description"),
          now: new Date().toISOString(),
        }),
      });
      state.activeProjectId = state.operation.project?.project_id || state.activeProjectId;
      await loadProjects(projectActor());
      showToast("Chat 项目已创建");
    }
    if (action === "project-update") {
      const projectId = projectIdFrom(target);
      state.operation = await api("/api/management/chat-projects/update", {
        method: "POST",
        body: JSON.stringify({
          actor: projectActor(),
          project_id: projectId,
          title: optionalText("#project-title"),
          emoji: optionalText("#project-emoji"),
          description: optionalText("#project-description"),
          now: new Date().toISOString(),
        }),
      });
      state.activeProjectId = projectId;
      await loadProjects(projectActor());
      showToast("Chat 项目已更新");
    }
    if (action === "project-delete") {
      state.operation = await api("/api/management/chat-projects/delete", {
        method: "POST",
        body: JSON.stringify({ actor: projectActor(), project_id: projectIdFrom(target) }),
      });
      state.projectSessions = null;
      if (state.activeProjectId === projectIdFrom(target)) state.activeProjectId = "";
      state.chat.dialog = "";
      await loadProjects(projectActor());
      showToast("Chat 项目已删除");
    }
    if (action === "project-session-upsert") {
      state.operation = await api("/api/management/chat-projects/sessions/upsert", {
        method: "POST",
        body: JSON.stringify({
          session_id: $("#project-session-id").value.trim(),
          platform_id: "webchat",
          creator: projectActor(),
          display_name: optionalText("#project-session-name"),
          is_group: $("#project-session-group").checked,
          now: new Date().toISOString(),
        }),
      });
      showToast("项目会话已注册");
    }
    if (action === "project-session-add") {
      const actor = projectActor();
      const projectId = projectIdFrom(target);
      state.operation = await api("/api/management/chat-projects/add-session", {
        method: "POST",
        body: JSON.stringify({
          actor,
          project_id: projectId,
          session_id: $("#project-session-id").value.trim(),
        }),
      });
      await loadProjectSessions(actor, projectId);
      showToast("会话已加入项目");
    }
    if (action === "project-session-remove") {
      const actor = projectActor();
      const projectId = projectIdFrom(target);
      state.operation = await api("/api/management/chat-projects/remove-session", {
        method: "POST",
        body: JSON.stringify({
          actor,
          project_id: projectId || null,
          session_id: target.dataset.session || $("#project-session-id")?.value.trim(),
        }),
      });
      if (projectId) await loadProjectSessions(actor, projectId);
      showToast("会话已移出项目");
    }
    if (action === "project-session-select") {
      const project = (state.projects?.projects || state.projects?.data || [])
        .find((item) => item.project_id === target.dataset.project);
      state.chat.currentSessionProject = project || state.chat.currentSessionProject;
      state.chat.conversationId = target.dataset.session || state.chat.conversationId;
      state.activeProjectId = "";
      await loadMessages();
      await loadChatControls();
      showToast(`已打开项目会话 ${state.chat.conversationId}`);
    }
    if (action === "project-sessions-load") {
      state.activeProjectId = target.dataset.project;
      await loadProjectSessions(projectActor(), target.dataset.project);
      showToast("项目会话已刷新");
    }
}
