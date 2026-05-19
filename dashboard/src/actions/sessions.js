import { api } from "../api.js";
import { $, showToast } from "../dom.js";
import { loadSessions } from "../loaders.js";
import { state } from "../state.js";
import { optionalText, splitLines } from "./forms.js";

export async function handleSessionActions({ action, target }) {
  if (action === "load-sessions") {
    await loadSessions();
    showToast("会话规则已刷新");
  }
  if (action === "session-filter") {
    state.sessionFilter = $("#session-filter")?.value.trim() || "";
    state.activeUmo = $("#session-active-umo")?.value.trim() || state.activeUmo;
    state.sessionPageSize = Number($("#session-page-size")?.value || state.sessionPageSize || 10);
    state.sessionPage = 1;
    await loadSessions();
    showToast("会话规则筛选已更新");
  }
  if (action === "session-page-prev" || action === "session-page-next") {
    state.sessionPage = Math.max(1, Number(state.sessionPage || 1) + (action === "session-page-next" ? 1 : -1));
    await loadSessions();
  }
  if (action === "session-select") {
    toggleSelectedUmo(target.dataset.umo);
  }
  if (action === "session-select-all") {
    const visible = sessionRules().map((rule) => rule.umo);
    const selected = new Set(state.sessionSelectedUmos || []);
    const allSelected = visible.length > 0 && visible.every((umo) => selected.has(umo));
    if (allSelected) {
      visible.forEach((umo) => selected.delete(umo));
    } else {
      visible.forEach((umo) => selected.add(umo));
    }
    state.sessionSelectedUmos = Array.from(selected);
  }
  if (action === "session-rule-select") {
    state.activeUmo = target.dataset.umo;
    state.operation = sessionRules().find((rule) => rule.umo === target.dataset.umo) || null;
    showToast(`当前 UMO：${state.activeUmo}`);
  }
  if (action === "session-add-rule-open") {
    state.sessionDialog = "add-rule";
  }
  if (action === "session-add-rule-next") {
    const umo = $("#session-new-umo")?.value.trim();
    if (!umo) {
      showToast("请选择 UMO", "error");
      return;
    }
    state.activeUmo = umo;
    state.sessionEditUmo = umo;
    state.sessionDialog = "editor";
  }
  if (action === "session-rule-edit-open") {
    state.sessionEditUmo = target.dataset.umo || state.activeUmo;
    state.activeUmo = state.sessionEditUmo;
    state.sessionDialog = "editor";
  }
  if (action === "session-quick-name-open") {
    state.sessionQuickNameTarget = target.dataset.umo || state.activeUmo;
    state.sessionDialog = "quick-name";
  }
  if (action === "session-quick-name-save") {
    const umo = $("#session-quick-umo")?.value.trim() || state.sessionQuickNameTarget;
    const existing = sessionRuleByUmo(umo).rules?.session_service_config || {};
    const config = {
      session_enabled: existing.session_enabled !== false,
      llm_enabled: existing.llm_enabled !== false,
      tts_enabled: existing.tts_enabled !== false,
      ...existing,
    };
    const name = $("#session-quick-name")?.value.trim() || "";
    if (name) config.custom_name = name;
    if (!name) delete config.custom_name;
    await updateSourceRule(umo, "session_service_config", config);
    state.sessionDialog = "";
    await loadSessions();
    showToast("Custom name 已保存");
  }
  if (action === "session-dialog-close") {
    closeSessionDialog();
  }
  if (action === "session-rule-save-service") {
    const umo = editorUmo();
    await updateSourceRule(umo, "session_service_config", {
      session_enabled: $("#session-editor-session-enabled")?.checked ?? true,
      llm_enabled: $("#session-editor-llm-enabled")?.checked ?? true,
      tts_enabled: $("#session-editor-tts-enabled")?.checked ?? true,
      custom_name: optionalText("#session-editor-custom-name"),
      persona_id: optionalText("#session-editor-persona"),
    });
    await loadSessions();
    showToast("Service rule 已保存");
  }
  if (action === "session-rule-save-provider") {
    const umo = editorUmo();
    await saveProviderRule(umo, "provider_perf_chat_completion", $("#session-editor-chat-provider")?.value.trim() || "");
    await saveProviderRule(umo, "provider_perf_speech_to_text", $("#session-editor-stt-provider")?.value.trim() || "");
    await saveProviderRule(umo, "provider_perf_text_to_speech", $("#session-editor-tts-provider")?.value.trim() || "");
    await loadSessions();
    showToast("Provider rule 已保存");
  }
  if (action === "session-rule-save-plugin") {
    const umo = editorUmo();
    const config = {
      enabled_plugins: splitLines($("#session-editor-enabled-plugins")?.value || ""),
      disabled_plugins: splitLines($("#session-editor-disabled-plugins")?.value || ""),
    };
    if (!config.enabled_plugins.length && !config.disabled_plugins.length) {
      await deleteSourceRule(umo, "session_plugin_config");
    } else {
      await updateSourceRule(umo, "session_plugin_config", config);
    }
    await loadSessions();
    showToast("Plugin rule 已保存");
  }
  if (action === "session-rule-save-kb") {
    const umo = editorUmo();
    const kbIds = splitLines($("#session-editor-kb-ids")?.value || "");
    if (!kbIds.length) {
      await deleteSourceRule(umo, "kb_config");
    } else {
      await updateSourceRule(umo, "kb_config", {
        kb_ids: kbIds,
        top_k: Number($("#session-editor-kb-top-k")?.value || 5),
        enable_rerank: $("#session-editor-kb-rerank")?.checked ?? true,
      });
    }
    await loadSessions();
    showToast("KB rule 已保存");
  }
  if (action === "session-rule-delete-open") {
    state.sessionDeleteTarget = target.dataset.umo;
    state.sessionDialog = "delete-rule";
  }
  if (action === "session-rule-delete-confirm") {
    if (!state.sessionDeleteTarget) return;
    state.operation = await api("/api/session/delete-rule", {
      method: "POST",
      body: JSON.stringify({ umo: state.sessionDeleteTarget }),
    });
    state.sessionSelectedUmos = (state.sessionSelectedUmos || []).filter((umo) => umo !== state.sessionDeleteTarget);
    closeSessionDialog();
    await loadSessions();
    showToast("规则集已删除");
  }
  if (action === "session-batch-delete-open") {
    state.sessionDialog = "batch-delete";
  }
  if (action === "session-batch-delete-confirm") {
    const umos = state.sessionSelectedUmos || [];
    if (!umos.length) return;
    state.operation = await api("/api/session/batch-delete-rule", {
      method: "POST",
      body: JSON.stringify({ umos }),
    });
    state.sessionSelectedUmos = [];
    closeSessionDialog();
    await loadSessions();
    showToast("批量删除完成");
  }
  if (action === "session-batch-apply") {
    await applySessionBatch();
  }
  if (action === "session-group-create-open") {
    state.sessionGroupDialogMode = "create";
    state.sessionGroupTargetId = "";
    state.sessionGroupDraftName = "";
    state.sessionGroupDraftUmos = [];
    state.sessionDialog = "group";
  }
  if (action === "session-group-edit-open") {
    const group = sessionGroupById(target.dataset.group);
    state.sessionGroupDialogMode = "edit";
    state.sessionGroupTargetId = group.id || "";
    state.sessionGroupDraftName = group.name || "";
    state.sessionGroupDraftUmos = [...(group.umos || [])];
    state.sessionDialog = "group";
  }
  if (action === "session-group-add-umo") {
    captureGroupDraft();
    addGroupDraftUmo(target.dataset.umo);
  }
  if (action === "session-group-remove-umo") {
    captureGroupDraft();
    state.sessionGroupDraftUmos = (state.sessionGroupDraftUmos || []).filter((umo) => umo !== target.dataset.umo);
  }
  if (action === "session-group-save") {
    const id = $("#session-group-id")?.value.trim() || state.sessionGroupTargetId;
    const name = $("#session-group-name")?.value.trim() || state.sessionGroupDraftName;
    const umos = splitLines($("#session-group-umos")?.value || "").length
      ? splitLines($("#session-group-umos")?.value || "")
      : state.sessionGroupDraftUmos || [];
    if (!name) {
      showToast("分组名称不能为空", "error");
      return;
    }
    const path = state.sessionGroupDialogMode === "edit" ? "/api/session/group/update" : "/api/session/group/create";
    state.operation = await api(path, {
      method: "POST",
      body: JSON.stringify(state.sessionGroupDialogMode === "edit" ? { id, name, umos } : { name, umos }),
    });
    closeSessionDialog();
    await loadSessions();
    showToast("会话分组已保存");
  }
  if (action === "session-group-delete-open") {
    state.sessionGroupTargetId = target.dataset.group;
    state.sessionDialog = "group-delete";
  }
  if (action === "session-group-delete-confirm") {
    state.operation = await api("/api/session/group/delete", {
      method: "POST",
      body: JSON.stringify({ id: state.sessionGroupTargetId }),
    });
    closeSessionDialog();
    await loadSessions();
    showToast("会话分组已删除");
  }
  if (action === "session-group-add-selected") {
    const umos = state.sessionSelectedUmos || [];
    if (!umos.length) {
      showToast("请先选择会话", "error");
      return;
    }
    state.operation = await api("/api/session/group/update", {
      method: "POST",
      body: JSON.stringify({ id: target.dataset.group, add_umos: umos }),
    });
    await loadSessions();
    showToast("已添加到分组");
  }
}

function editorUmo() {
  return $("#session-editor-umo")?.value.trim() || state.sessionEditUmo || state.activeUmo;
}

async function updateSourceRule(umo, ruleKey, ruleValue) {
  state.operation = await api("/api/session/update-rule", {
    method: "POST",
    body: JSON.stringify({ umo, rule_key: ruleKey, rule_value: ruleValue }),
  });
}

async function deleteSourceRule(umo, ruleKey) {
  state.operation = await api("/api/session/delete-rule", {
    method: "POST",
    body: JSON.stringify({ umo, rule_key: ruleKey }),
  });
}

async function saveProviderRule(umo, ruleKey, providerId) {
  if (providerId) {
    await updateSourceRule(umo, ruleKey, providerId);
  } else if (sessionRuleByUmo(umo).rules?.[ruleKey]) {
    await deleteSourceRule(umo, ruleKey);
  }
}

async function applySessionBatch() {
  const scopeValue = $("#session-batch-scope")?.value || "selected";
  const { scope, group_id } = sourceBatchScope(scopeValue);
  const umos = scope === "selected" ? (state.sessionSelectedUmos || []) : [];
  if (scope === "selected" && !umos.length) {
    showToast("请先选择要操作的会话", "error");
    return;
  }
  const servicePatch = {
    scope,
    group_id,
    umos,
    llm_enabled: boolSelect("#session-batch-llm"),
    tts_enabled: boolSelect("#session-batch-tts"),
  };
  const tasks = [];
  if (servicePatch.llm_enabled !== null || servicePatch.tts_enabled !== null) {
    tasks.push(api("/api/session/batch-update-service", {
      method: "POST",
      body: JSON.stringify(servicePatch),
    }));
  }
  const chatProvider = $("#session-batch-chat-provider")?.value.trim() || "";
  const ttsProvider = $("#session-batch-tts-provider")?.value.trim() || "";
  if (chatProvider) {
    tasks.push(api("/api/session/batch-update-provider", {
      method: "POST",
      body: JSON.stringify({ scope, group_id, umos, provider_type: "chat_completion", provider_id: chatProvider }),
    }));
  }
  if (ttsProvider) {
    tasks.push(api("/api/session/batch-update-provider", {
      method: "POST",
      body: JSON.stringify({ scope, group_id, umos, provider_type: "text_to_speech", provider_id: ttsProvider }),
    }));
  }
  if (!tasks.length) {
    showToast("请至少选择一项要修改的配置", "error");
    return;
  }
  const results = await Promise.all(tasks);
  state.operation = results.at(-1);
  state.sessionBatchScope = scopeValue;
  state.sessionBatchLlm = $("#session-batch-llm")?.value || "";
  state.sessionBatchTts = $("#session-batch-tts")?.value || "";
  state.sessionBatchChatProvider = "";
  state.sessionBatchTtsProvider = "";
  await loadSessions();
  showToast("批量更新成功");
}

function sourceBatchScope(scopeValue) {
  if (scopeValue.startsWith("custom_group:")) {
    return { scope: "custom_group", group_id: scopeValue.slice("custom_group:".length) };
  }
  return { scope: scopeValue, group_id: null };
}

function boolSelect(selector) {
  const value = $(selector)?.value || "";
  if (value === "true") return true;
  if (value === "false") return false;
  return null;
}

function toggleSelectedUmo(umo) {
  if (!umo) return;
  const selected = new Set(state.sessionSelectedUmos || []);
  if (selected.has(umo)) {
    selected.delete(umo);
  } else {
    selected.add(umo);
  }
  state.sessionSelectedUmos = Array.from(selected);
}

function sessionRules() {
  const payload = state.sessions || {};
  const data = payload.data || payload;
  return Array.isArray(data.rules) ? data.rules.map(normalizeRule) : [];
}

function sessionRuleByUmo(umo) {
  return sessionRules().find((rule) => rule.umo === umo) || { umo, rules: {} };
}

function normalizeRule(rule = {}) {
  const rules = { ...(rule.rules || {}) };
  if (rule.service) rules.session_service_config = rule.service;
  if (rule.plugin) rules.session_plugin_config = rule.plugin;
  if (rule.knowledge_base) rules.kb_config = rule.knowledge_base;
  for (const provider of rule.provider_preferences || []) {
    const key = providerKey(provider.capability);
    if (key) rules[key] = provider.provider_id;
  }
  return { ...rule, rules };
}

function providerKey(capability = "") {
  const normalized = String(capability).toLowerCase();
  if (normalized.includes("chat")) return "provider_perf_chat_completion";
  if (normalized.includes("speech_to_text")) return "provider_perf_speech_to_text";
  if (normalized.includes("text_to_speech")) return "provider_perf_text_to_speech";
  return "";
}

function sessionGroupById(id) {
  const payload = state.sessionGroups || {};
  const data = payload.data || payload;
  const groups = Array.isArray(data.groups) ? data.groups : [];
  return groups.find((group) => group.id === id) || { id: "", name: "", umos: [] };
}

function addGroupDraftUmo(umo) {
  if (!umo) return;
  const selected = new Set(state.sessionGroupDraftUmos || []);
  selected.add(umo);
  state.sessionGroupDraftUmos = Array.from(selected);
}

function closeSessionDialog() {
  state.sessionDialog = "";
  state.sessionDeleteTarget = "";
  state.sessionQuickNameTarget = "";
  state.sessionGroupTargetId = "";
  state.sessionGroupDraftName = "";
}

function captureGroupDraft() {
  state.sessionGroupDraftName = $("#session-group-name")?.value.trim() || state.sessionGroupDraftName || "";
}
