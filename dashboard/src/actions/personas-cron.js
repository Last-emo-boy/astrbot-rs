import { api } from "../api.js";
import { $, showToast } from "../dom.js";
import { loadCron, loadPersonas, loadSubagents } from "../loaders.js";
import { state } from "../state.js";
import { optionalText } from "./forms.js";

export async function handlePersonaCronActions({ action, target }) {
    if (action === "load-personas") {
      await loadPersonas(state.personaFolderId);
      showToast("Persona 已刷新");
    }
    if (action === "persona-search") {
      state.personaSearch = personaSearchValue();
      state.personaFolderId = selectedPersonaFolder();
      await loadPersonas(state.personaFolderId);
      showToast("Persona 筛选已更新");
    }
    if (action === "persona-dialog-close") {
      closePersonaDialog();
    }
    if (action === "persona-create-open") {
      state.personaDialog = "form";
      state.personaEditId = "";
      state.personaDialogPairCount = 1;
    }
    if (action === "persona-edit-open") {
      state.personaDialog = "form";
      state.personaEditId = target.dataset.persona;
      state.personaPreviewId = "";
      state.personaDialogPairCount = 1;
    }
    if (action === "persona-preview") {
      state.personaDialog = "preview";
      state.personaPreviewId = target.dataset.persona;
      showToast(`Persona 预览：${target.dataset.persona}`);
    }
    if (action === "persona-resolve") {
      state.operation = await api("/api/management/personas/resolve", {
        method: "POST",
        body: JSON.stringify({
          session_id: state.chat.conversationId,
          platform_name: "webchat",
          forced_persona_id: target.dataset.persona,
        }),
      });
      showToast(`Persona 解析完成：${state.operation.persona_id || "disabled"}`);
    }
    if (action === "persona-form-save") {
      const payload = personaFormPayload();
      const editing = $("#persona-form-mode")?.value === "edit";
      state.operation = await api(editing ? "/api/persona/update" : "/api/persona/create", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      closePersonaDialog();
      await loadPersonas(state.personaFolderId);
      showToast(state.operation.message || "Persona 已保存");
    }
    if (action === "persona-dialog-add-pair") {
      state.personaDialogPairCount = (state.personaDialogPairCount || 1) + 1;
    }
    if (action === "persona-folder-create-open") {
      state.personaDialog = "create-folder";
    }
    if (action === "persona-folder-create-submit" || action === "persona-folder-upsert") {
      const name = optionalText("#persona-folder-name");
      if (!name) throw new Error("文件夹名称不能为空");
      state.operation = await api("/api/persona/folder/create", {
        method: "POST",
        body: JSON.stringify({
          name,
          description: optionalText("#persona-folder-description"),
          parent_id: optionalText("#persona-folder-parent"),
        }),
      });
      const folderId = state.operation?.data?.folder?.folder_id || state.personaFolderId;
      closePersonaDialog();
      await loadPersonas(folderId);
      state.personaFolderId = folderId || state.personaFolderId;
      showToast(state.operation.message || "Persona 文件夹已创建");
    }
    if (action === "persona-folder-rename-open") {
      state.personaDialog = "rename-folder";
      state.personaRenameFolderId = target.dataset.folder;
    }
    if (action === "persona-folder-rename-submit") {
      const folderId = state.personaRenameFolderId;
      const name = optionalText("#persona-folder-rename-name");
      if (!folderId || !name) throw new Error("文件夹名称不能为空");
      state.operation = await api("/api/persona/folder/update", {
        method: "POST",
        body: JSON.stringify({ folder_id: folderId, name }),
      });
      closePersonaDialog();
      await loadPersonas(state.personaFolderId);
      showToast(state.operation.message || "Persona 文件夹已重命名");
    }
    if (action === "persona-clone-open" || action === "persona-clone") {
      state.personaDialog = "clone";
      state.personaCloneSourceId = target.dataset.persona;
    }
    if (action === "persona-clone-submit") {
      const sourceId = state.personaCloneSourceId;
      const newId = optionalText("#persona-clone-new-id");
      if (!sourceId || !newId) throw new Error("New Persona ID 不能为空");
      state.operation = await api("/api/persona/clone", {
        method: "POST",
        body: JSON.stringify({
          source_persona_id: sourceId,
          new_persona_id: newId,
        }),
      });
      closePersonaDialog();
      await loadPersonas(state.personaFolderId);
      showToast(state.operation.message || "Persona 已克隆");
    }
    if (action === "persona-move-open" || action === "persona-move") {
      state.personaDialog = "move";
      state.personaMoveType = "persona";
      state.personaMoveId = target.dataset.persona;
    }
    if (action === "persona-folder-move-open") {
      state.personaDialog = "move";
      state.personaMoveType = "folder";
      state.personaMoveId = target.dataset.folder;
    }
    if (action === "persona-move-submit") {
      const targetFolderId = optionalText("#persona-move-target");
      if (state.personaMoveType === "folder") {
        state.operation = await api("/api/persona/folder/update", {
          method: "POST",
          body: JSON.stringify({
            folder_id: state.personaMoveId,
            parent_id: targetFolderId,
          }),
        });
      } else {
        state.operation = await api("/api/persona/move", {
          method: "POST",
          body: JSON.stringify({
            persona_id: state.personaMoveId,
            folder_id: targetFolderId,
          }),
        });
      }
      closePersonaDialog();
      await loadPersonas(state.personaFolderId);
      showToast("Persona 已移动");
    }
    if (action === "persona-delete-open" || action === "persona-delete") {
      state.personaDialog = "delete";
      state.personaDeleteType = "persona";
      state.personaDeleteId = target.dataset.persona;
    }
    if (action === "persona-folder-delete-open" || action === "persona-folder-delete") {
      state.personaDialog = "delete";
      state.personaDeleteType = "folder";
      state.personaDeleteId = target.dataset.folder;
    }
    if (action === "persona-delete-confirm") {
      if (state.personaDeleteType === "folder") {
        state.operation = await api("/api/persona/folder/delete", {
          method: "POST",
          body: JSON.stringify({ folder_id: state.personaDeleteId }),
        });
      } else {
        state.operation = await api("/api/persona/delete", {
          method: "POST",
          body: JSON.stringify({ persona_id: state.personaDeleteId }),
        });
      }
      closePersonaDialog();
      await loadPersonas(state.personaFolderId);
      showToast(state.operation.message || "Persona 已删除");
    }
    if (action === "persona-rank-up" || action === "persona-rank-down") {
      state.operation = await api("/api/persona/reorder", {
        method: "POST",
        body: JSON.stringify({
          items: reorderedItems(
            currentPersonasInFolder(),
            target.dataset.persona,
            action === "persona-rank-up" ? -1 : 1,
            "persona",
          ),
        }),
      });
      await loadPersonas(state.personaFolderId);
      showToast("Persona 顺序已更新");
    }
    if (action === "persona-open-folder") {
      state.personaFolderId = target.dataset.folder || null;
      await loadPersonas(state.personaFolderId);
      showToast(`已打开文件夹：${target.dataset.folder || "根目录"}`);
    }
    if (action === "persona-folder-rank-up" || action === "persona-folder-rank-down") {
      state.operation = await api("/api/persona/reorder", {
        method: "POST",
        body: JSON.stringify({
          items: reorderedItems(
            currentFoldersInFolder(),
            target.dataset.folder,
            action === "persona-folder-rank-up" ? -1 : 1,
            "folder",
          ),
        }),
      });
      await loadPersonas(state.personaFolderId);
      showToast("Persona 文件夹顺序已更新");
    }
    if (action === "load-cron") {
      await loadCron();
      showToast("Cron 已刷新");
    }
    if (action === "cron-create-open") {
      state.cronDialog = "form";
      state.cronFormMode = "create";
      state.cronEditId = "";
    }
    if (action === "cron-edit-open") {
      state.cronDialog = "form";
      state.cronFormMode = "edit";
      state.cronEditId = target.dataset.job;
    }
    if (action === "cron-dialog-close") {
      closeCronDialog();
    }
    if (action === "cron-start") {
      state.operation = await api("/api/management/cron/start", { method: "POST" });
      await loadCron();
      showToast("Scheduler 已启动");
    }
    if (action === "cron-shutdown") {
      state.operation = await api("/api/management/cron/shutdown", { method: "POST" });
      await loadCron();
      showToast("Scheduler 已停止");
    }
    if (action === "cron-toggle") {
      const nextEnabled = target.matches("input")
        ? target.checked
        : target.dataset.enabled !== "true";
      state.operation = await api(`/api/cron/jobs/${encodeURIComponent(target.dataset.job)}`, {
        method: "PATCH",
        body: JSON.stringify({ enabled: nextEnabled }),
      });
      await loadCron();
      showToast(nextEnabled ? "Cron job 已启用" : "Cron job 已停用");
    }
    if (action === "cron-run") {
      state.operation = await api(`/api/cron/jobs/${encodeURIComponent(target.dataset.job)}/run`, {
        method: "POST",
        body: JSON.stringify({}),
      });
      await loadCron();
      showToast("Cron job 已执行");
    }
    if (action === "cron-tick") {
      state.operation = await api("/api/management/cron/tick", {
        method: "POST",
        body: JSON.stringify({ now_unix: Math.floor(Date.now() / 1000) }),
      });
      await loadCron();
      showToast(`Tick 完成：运行 ${state.operation.report.ran_count} 个任务`);
    }
    if (action === "cron-delete-open") {
      state.cronDialog = "delete";
      state.cronDeleteId = target.dataset.job;
    }
    if (action === "cron-delete-confirm") {
      state.operation = await api(`/api/cron/jobs/${encodeURIComponent(state.cronDeleteId)}`, {
        method: "DELETE",
      });
      closeCronDialog();
      await loadCron();
      showToast(state.operation.message || "Cron job 已删除");
    }
    if (action === "cron-form-save" || action === "cron-upsert") {
      const payload = cronFormPayload();
      const editing = $("#cron-form-mode")?.value === "edit";
      const jobId = $("#cron-form-job-id")?.value.trim();
      state.operation = await api(editing ? `/api/cron/jobs/${encodeURIComponent(jobId)}` : "/api/cron/jobs", {
        method: editing ? "PATCH" : "POST",
        body: JSON.stringify(payload),
      });
      closeCronDialog();
      await loadCron();
      showToast(editing ? "Cron job 已更新" : "Cron job 已创建");
    }
    if (action === "subagent-refresh") {
      await loadSubagents();
      showToast("SubAgent 已刷新");
    }
    if (action === "subagent-add") {
      const payload = subagentFormPayload({ validate: false });
      payload.agents.push(defaultSubagentDraft());
      state.subagents = {
        ...(state.subagents || {}),
        ...payload,
      };
      showToast("SubAgent 草稿已新增");
    }
    if (action === "subagent-remove") {
      const payload = subagentFormPayload({ validate: false });
      const index = Number.parseInt(target.dataset.index || "-1", 10);
      if (index >= 0) {
        payload.agents.splice(index, 1);
      }
      state.subagents = {
        ...(state.subagents || {}),
        ...payload,
      };
      showToast("SubAgent 草稿已删除");
    }
    if (action === "subagent-save") {
      const payload = subagentFormPayload();
      state.operation = await api("/api/subagent/config", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      await loadSubagents();
      showToast(state.operation.message || "SubAgent 配置已保存");
    }
    if (action === "subagent-execute") {
      state.operation = await api("/api/management/subagents/execute", {
        method: "POST",
        body: JSON.stringify({
          agent_name: $("#subagent-execute-name").value.trim(),
          input: $("#subagent-execute-input").value,
          background: $("#subagent-execute-background").checked,
          context: { route: state.route },
        }),
      });
      state.subagents = {
        ...(state.subagents || {}),
        ...(state.operation.catalog || {}),
      };
      showToast("SubAgent execution bridge 已调用");
    }
}

export function handlePersonaDragStart(event) {
  const draggable = event.target?.closest?.("[data-drag-type='persona']");
  if (!draggable) return false;
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("application/json", JSON.stringify({
    type: "persona",
    persona_id: draggable.dataset.persona,
  }));
  return true;
}

export function handlePersonaDragOver(event) {
  const dropTarget = event.target?.closest?.("[data-drop-folder]");
  if (!dropTarget || state.route !== "personas") return false;
  event.preventDefault();
  event.dataTransfer.dropEffect = "move";
  return true;
}

export async function handlePersonaDrop(event) {
  const dropTarget = event.target?.closest?.("[data-drop-folder]");
  if (!dropTarget || state.route !== "personas") return false;
  event.preventDefault();
  const raw = event.dataTransfer?.getData("application/json") || "";
  if (!raw) return false;
  const payload = JSON.parse(raw);
  if (payload.type !== "persona" || !payload.persona_id) return false;
  const folderId = dropTarget.dataset.dropFolder || null;
  state.operation = await api("/api/persona/move", {
    method: "POST",
    body: JSON.stringify({
      persona_id: payload.persona_id,
      folder_id: folderId,
    }),
  });
  state.personaFolderId = folderId;
  await loadPersonas(folderId);
  showToast("Persona 已移动");
  return true;
}

function selectedPersonaFolder() {
  return state.personaFolderId || null;
}

function personaSearchValue() {
  const inputs = [...document.querySelectorAll(".persona-search-field")];
  const visible = inputs.find((input) => input.offsetParent !== null);
  return (visible || inputs[0])?.value.trim() || "";
}

function integerValue(selector) {
  const value = Number.parseInt($(selector)?.value || "0", 10);
  return Number.isFinite(value) ? value : 0;
}

function closePersonaDialog() {
  state.personaDialog = "";
  state.personaEditId = "";
  state.personaPreviewId = "";
  state.personaMoveType = "";
  state.personaMoveId = "";
  state.personaDeleteType = "";
  state.personaDeleteId = "";
  state.personaCloneSourceId = "";
  state.personaRenameFolderId = "";
  state.personaDialogPairCount = 1;
}

function closeCronDialog() {
  state.cronDialog = "";
  state.cronEditId = "";
  state.cronDeleteId = "";
  state.cronRunId = "";
  state.cronFormMode = "create";
}

function cronFormPayload() {
  const runOnce = Boolean($("#cron-form-run-once")?.checked);
  const name = optionalText("#cron-form-name") || "active_agent_task";
  const note = optionalText("#cron-form-note");
  const session = optionalText("#cron-form-session");
  const cronExpression = optionalText("#cron-form-cron");
  const runAt = optionalText("#cron-form-run-at");
  if (!session) throw new Error("session is required");
  if (!note) throw new Error("note is required");
  if (runOnce && !runAt) throw new Error("run_at is required when run_once=true");
  if (!runOnce && !cronExpression) throw new Error("cron_expression is required when run_once=false");
  return {
    run_once: runOnce,
    name,
    note,
    description: note,
    session,
    timezone: optionalText("#cron-form-timezone") || undefined,
    enabled: $("#cron-form-enabled")?.checked !== false,
    cron_expression: runOnce ? undefined : cronExpression,
    run_at: runOnce ? normalizeRunAt(runAt) : undefined,
  };
}

function normalizeRunAt(value) {
  const runAt = String(value || "").trim();
  return runAt.length === 16 ? `${runAt}:00Z` : runAt;
}

function subagentFormPayload({ validate = true } = {}) {
  const agents = [...document.querySelectorAll("[data-subagent-card]")]
    .map((card, index) => subagentAgentPayload(card, index));
  if (validate) {
    validateSubagentAgents(agents);
  }
  return {
    main_enable: Boolean($("#subagent-main-enable")?.checked),
    remove_main_duplicate_tools: Boolean($("#subagent-dedupe")?.checked),
    router_system_prompt: optionalText("#subagent-router-prompt"),
    agents,
  };
}

function subagentAgentPayload(card, index) {
  const toolsMode = card.querySelector(`input[name="subagent-tools-mode-${index}"]:checked`)?.value || "custom";
  const tools = toolsMode === "all"
    ? null
    : [...card.querySelectorAll('input[name="subagent-tool"]:checked')]
      .map((input) => input.value)
      .filter(Boolean);
  return {
    name: card.querySelector(".subagent-agent-name")?.value.trim() || "",
    enabled: card.querySelector(".subagent-agent-enabled")?.checked !== false,
    persona_id: card.querySelector(".subagent-agent-persona")?.value.trim() || "",
    provider_id: card.querySelector(".subagent-agent-provider")?.value.trim() || undefined,
    system_prompt: card.querySelector(".subagent-agent-prompt")?.value.trim() || "",
    public_description: card.querySelector(".subagent-agent-description")?.value.trim() || "",
    tools,
  };
}

function validateSubagentAgents(agents) {
  const nameRe = /^[a-z][a-z0-9_]{0,63}$/;
  const seen = new Set();
  for (const agent of agents) {
    if (!agent.name) throw new Error("存在未填写名称的 SubAgent");
    if (!nameRe.test(agent.name)) throw new Error("SubAgent 名称不合法：仅允许英文小写字母/数字/下划线，且需以字母开头");
    if (seen.has(agent.name)) throw new Error(`SubAgent 名称重复：${agent.name}`);
    seen.add(agent.name);
    if (!agent.persona_id) throw new Error(`SubAgent ${agent.name} 未选择 Persona`);
  }
}

function defaultSubagentDraft() {
  return {
    name: "",
    enabled: true,
    persona_id: "",
    provider_id: "",
    system_prompt: "",
    public_description: "",
    tools: null,
  };
}

function personaFormPayload() {
  const personaId = optionalText("#persona-form-id");
  const prompt = optionalText("#persona-form-prompt");
  if (!personaId || !prompt) {
    throw new Error("Persona ID 和 System prompt 不能为空");
  }
  return {
    persona_id: personaId,
    system_prompt: prompt,
    custom_error_message: optionalText("#persona-form-error"),
    begin_dialogs: collectBeginDialogs(),
    tools: collectSelectedAccess("tools"),
    skills: collectSelectedAccess("skills"),
    folder_id: optionalText("#persona-form-folder"),
    sort_order: integerValue("#persona-form-sort-order"),
  };
}

function collectBeginDialogs() {
  const dialogs = [];
  for (const row of document.querySelectorAll("[data-persona-dialog-pair]")) {
    const user = row.querySelector(".persona-dialog-user")?.value.trim() || "";
    const assistant = row.querySelector(".persona-dialog-assistant")?.value.trim() || "";
    if (!user && !assistant) continue;
    if (!user || !assistant) {
      throw new Error("预设对话需要成对填写 User 和 Assistant 消息");
    }
    dialogs.push(user, assistant);
  }
  return dialogs;
}

function collectSelectedAccess(kind) {
  const mode = document.querySelector(`input[name="persona-${kind}-mode"]:checked`)?.value || "all";
  if (mode === "all") return null;
  return [...document.querySelectorAll(`input[name="persona-${kind}"]:checked`)]
    .map((input) => input.value)
    .filter(Boolean);
}

function currentPersonasInFolder() {
  const folderId = state.personaFolderId || null;
  return (state.personas?.personas || [])
    .map(normalizePersona)
    .filter((persona) => persona.folder_id === folderId)
    .sort(compareBySortAndName);
}

function currentFoldersInFolder() {
  const folderId = state.personaFolderId || null;
  return (state.personas?.folders || [])
    .map(normalizeFolder)
    .filter((folder) => folder.parent_id === folderId)
    .sort(compareBySortAndName);
}

function normalizePersona(persona) {
  return {
    ...persona,
    id: persona.persona_id || persona.id,
    folder_id: normalizeFolderId(persona.folder_id),
    sort_order: Number(persona.sort_order || 0),
  };
}

function normalizeFolder(folder) {
  return {
    ...folder,
    id: folder.folder_id || folder.id,
    parent_id: normalizeFolderId(folder.parent_id),
    sort_order: Number(folder.sort_order || 0),
  };
}

function normalizeFolderId(value) {
  const id = String(value || "").trim();
  return id ? id : null;
}

function compareBySortAndName(left, right) {
  return (left.sort_order || 0) - (right.sort_order || 0)
    || String(left.name || left.id).localeCompare(String(right.name || right.id));
}

function reorderedItems(items, id, direction, type) {
  const ids = items.map((item) => item.id);
  const index = ids.indexOf(id);
  const nextIndex = index + direction;
  if (index < 0 || nextIndex < 0 || nextIndex >= ids.length) {
    return items.map((item, sortOrder) => ({ id: item.id, type, sort_order: sortOrder }));
  }
  const next = [...ids];
  [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
  return next.map((itemId, sortOrder) => ({ id: itemId, type, sort_order: sortOrder }));
}
