import { escapeHtml, jsonBlock } from "../dom.js";
import { state } from "../state.js";
import { closurePill, metric, pill, statusItem } from "./shared.js";

export function renderSubAgent() {
  const service = state.capabilities?.services?.find((item) => item.id === "subagent");
  const subagents = normalizedState();
  const closure = service?.closure_level || "unavailable";
  return `
    <div class="subagent-page" data-page="subagent">
      <section class="panel subagent-hero-panel">
        <div class="subagent-title-row">
          <div>
            <div class="subagent-heading-line">
              <h2>SubAgent 编排</h2>
              <span class="tag warn">实验性</span>
            </div>
            <p class="empty">主 LLM 可直接使用自身工具，也可通过 handoff 分派给各个 SubAgent。</p>
          </div>
          <div class="actions">
            <button class="button secondary" type="button" data-action="subagent-refresh">刷新</button>
            <button class="button" type="button" data-action="subagent-save">保存</button>
          </div>
        </div>
        ${subagents.unavailable ? `<p class="empty error">${escapeHtml(subagents.unavailable)}</p>` : ""}
      </section>

      <div class="grid cols-4">
        ${metric("Orchestrator", subagents.main_enable ? "enabled" : "disabled", "main LLM routing")}
        ${metric("Agents", subagents.agents.length, "configured")}
        ${metric("Handoffs", subagents.handoffs.length, closure)}
        ${metric("Tools", subagents.available_tools.length, "assignable")}
      </div>

      <div class="grid cols-2 subagent-main-grid">
        <section class="panel">
          <h2>全局设置</h2>
          <div class="subagent-switch-list">
            <label class="subagent-switch-row">
              <span>
                <strong>启用 SubAgent 编排</strong>
                <small>启用后主 LLM 会获得 transfer_to_* 委派工具。</small>
              </span>
              <span class="ui-switch">
                <input id="subagent-main-enable" type="checkbox" ${subagents.main_enable ? "checked" : ""} />
                <span class="ui-switch-track"><span class="ui-switch-thumb"></span></span>
              </span>
            </label>
            <label class="subagent-switch-row">
              <span>
                <strong>主 LLM 去重重复工具</strong>
                <small>与 SubAgent 重叠的工具将从主 LLM 工具集中隐藏。</small>
              </span>
              <span class="ui-switch">
                <input id="subagent-dedupe" type="checkbox" ${subagents.remove_main_duplicate_tools ? "checked" : ""} />
                <span class="ui-switch-track"><span class="ui-switch-thumb"></span></span>
              </span>
            </label>
          </div>
          <div class="form-row">
            <label>Router system prompt</label>
            <textarea id="subagent-router-prompt" rows="5">${escapeHtml(subagents.router_system_prompt || "")}</textarea>
          </div>
        </section>
        <section class="panel">
          <h2>Runtime</h2>
          <div class="status-list">
            ${statusItem("API Base", service?.api_base || "/api/subagent/config")}
            ${statusItem("Closure", closure)}
            ${statusItem("Configured", service?.configured ? "yes" : "no")}
            ${statusItem("Execution Bridge", subagents.execution_available ? "configured" : "on demand")}
          </div>
          <div class="mt-16">${closurePill(closure)}</div>
          ${service?.notes?.length ? service.notes.map((note) => `<p class="empty">${escapeHtml(note)}</p>`).join("") : ""}
        </section>
      </div>

      <section class="panel subagent-agents-panel">
        <div class="panel-title-row">
          <div>
            <h2>SubAgents 配置</h2>
            <p class="empty">每个 SubAgent 会生成一个 transfer_to_* handoff tool。</p>
          </div>
          <button class="button" type="button" data-action="subagent-add">新增 SubAgent</button>
        </div>
        ${subagents.agents.length ? `
          <div class="subagent-agent-list">
            ${subagents.agents.map((agent, index) => renderAgentCard(agent, index, subagents)).join("")}
          </div>
        ` : renderEmptyAgents()}
      </section>

      <div class="grid cols-2">
        ${renderHandoffPreview(subagents)}
        ${renderExecutionPanel(subagents)}
      </div>

      <section class="panel">
        <h2>Service Descriptor</h2>
        ${jsonBlock({ service: service || { id: "subagent", configured: false, closure_level: "unavailable" }, catalog: subagents })}
      </section>
    </div>
  `;
}

function normalizedState() {
  const raw = state.subagents || {};
  return {
    main_enable: Boolean(raw.main_enable),
    remove_main_duplicate_tools: Boolean(raw.remove_main_duplicate_tools),
    router_system_prompt: raw.router_system_prompt || "",
    agents: Array.isArray(raw.agents) ? raw.agents : [],
    handoffs: Array.isArray(raw.handoffs) ? raw.handoffs : [],
    executions: Array.isArray(raw.executions) ? raw.executions : [],
    available_tools: Array.isArray(raw.available_tools) ? raw.available_tools.filter((tool) => tool.name) : [],
    providers: Array.isArray(raw.providers) ? raw.providers : [],
    personas: Array.isArray(raw.personas) ? raw.personas : [],
    unavailable: raw.unavailable || "",
    execution_available: raw.execution_available !== false,
  };
}

function renderAgentCard(agent, index, subagents) {
  const enabled = agent.enabled !== false;
  return `
    <details class="subagent-card" data-subagent-card data-index="${index}" open>
      <summary>
        <span class="subagent-card-status ${enabled ? "enabled" : ""}"></span>
        <span>
          <strong>${escapeHtml(agent.name || "未命名 SubAgent")}</strong>
          <small>${escapeHtml(agent.public_description || "暂无描述")}</small>
        </span>
        <span class="subagent-card-actions">
          ${pill(enabled ? "启用" : "停用", enabled ? "ok" : "")}
          <button class="button ghost danger" type="button" data-action="subagent-remove" data-index="${index}">删除</button>
        </span>
      </summary>
      <div class="subagent-card-body">
        <div class="form-grid cols-2 subagent-form-grid">
          <label class="check-row wide-field">
            <input class="subagent-agent-enabled" type="checkbox" ${enabled ? "checked" : ""} />
            启用此 SubAgent
          </label>
          <div class="form-row">
            <label>Agent 名称</label>
            <input class="subagent-agent-name" value="${escapeHtml(agent.name || "")}" placeholder="researcher" />
          </div>
          <div class="form-row">
            <label>Chat Provider</label>
            <select class="subagent-agent-provider">
              <option value="">跟随全局默认 provider</option>
              ${providerOptions(subagents.providers, agent.provider_id)}
            </select>
          </div>
          <div class="form-row">
            <label>Persona</label>
            <select class="subagent-agent-persona">
              <option value="">选择人格设定</option>
              ${personaOptions(subagents.personas, agent.persona_id)}
            </select>
          </div>
          <div class="form-row">
            <label>Handoff tool</label>
            <input value="${escapeHtml(transferToolName(agent.name))}" readonly />
          </div>
          <div class="form-row wide-field">
            <label>System prompt</label>
            <textarea class="subagent-agent-prompt" rows="4" placeholder="SubAgent 的专用系统提示词">${escapeHtml(agent.system_prompt || "")}</textarea>
          </div>
          <div class="form-row wide-field">
            <label>对主 LLM 的描述</label>
            <textarea class="subagent-agent-description" rows="3" placeholder="用于帮助主 LLM 判断何时 handoff">${escapeHtml(agent.public_description || "")}</textarea>
          </div>
          <div class="subagent-tool-picker wide-field">
            <div class="subagent-tool-header">
              <strong>工具分配</strong>
              <span>${agent.tools === null ? "全部工具" : `${(agent.tools || []).length} selected`}</span>
            </div>
            <div class="subagent-tool-mode">
              <label class="check-row"><input type="radio" name="subagent-tools-mode-${index}" value="all" ${agent.tools === null ? "checked" : ""} /> 全部工具</label>
              <label class="check-row"><input type="radio" name="subagent-tools-mode-${index}" value="custom" ${agent.tools === null ? "" : "checked"} /> 自定义工具</label>
            </div>
            <div class="subagent-tool-grid">
              ${subagents.available_tools.length ? subagents.available_tools.map((tool) => renderToolChoice(tool, agent.tools || [])).join("") : `<p class="empty">暂无可分配工具。</p>`}
            </div>
          </div>
        </div>
      </div>
    </details>
  `;
}

function providerOptions(providers, selectedId = "") {
  return providers
    .filter((provider) => provider.provider_type === "chat_completion" || provider.type === "chat_completion" || provider.id)
    .map((provider) => {
      const id = provider.id || provider.provider_id || "";
      const label = provider.name || provider.model || provider.provider_source_id || id;
      return `<option value="${escapeHtml(id)}" ${id === selectedId ? "selected" : ""}>${escapeHtml(label || id)}</option>`;
    })
    .join("");
}

function personaOptions(personas, selectedId = "") {
  return personas
    .map((persona) => {
      const id = persona.persona_id || persona.id || "";
      const label = persona.name || persona.persona_id || persona.id || "";
      return `<option value="${escapeHtml(id)}" ${id === selectedId ? "selected" : ""}>${escapeHtml(label || id)}</option>`;
    })
    .join("");
}

function renderToolChoice(tool, selectedTools) {
  const checked = selectedTools.includes(tool.name);
  return `
    <label class="subagent-tool-choice">
      <input type="checkbox" name="subagent-tool" value="${escapeHtml(tool.name)}" ${checked ? "checked" : ""} />
      <span>
        <strong>${escapeHtml(tool.name)}</strong>
        <small>${escapeHtml(tool.description || tool.handler_module_path || "tool")}</small>
      </span>
    </label>
  `;
}

function renderEmptyAgents() {
  return `
    <div class="ui-state empty compact">
      <div class="ui-state-icon" aria-hidden="true">S</div>
      <div class="ui-state-copy">
        <strong>未配置 SubAgent</strong>
        <span>添加一个新的子代理以开始。</span>
      </div>
      <div class="ui-state-action">
        <button class="button secondary" type="button" data-action="subagent-add">创建第一个 Agent</button>
      </div>
    </div>
  `;
}

function renderHandoffPreview(subagents) {
  return `
    <section class="panel">
      <h2>Handoff Preview</h2>
      ${subagents.handoffs.length ? `
        <table class="table subagent-handoff-table">
          <thead><tr><th>Tool</th><th>Agent</th><th>Description</th><th>Tools</th></tr></thead>
          <tbody>
            ${subagents.handoffs.map((handoff) => `
              <tr>
                <td><strong>${escapeHtml(handoff.tool_name)}</strong></td>
                <td>${escapeHtml(handoff.agent_name)}</td>
                <td>${escapeHtml(handoff.description || "-")}</td>
                <td>${handoff.all_tools ? `<span class="tag">all tools</span>` : (handoff.tools || []).map((tool) => `<span class="tag">${escapeHtml(tool)}</span>`).join(" ") || "-"}</td>
              </tr>
            `).join("")}
          </tbody>
        </table>
      ` : `<p class="empty">没有可注册的 handoff tool。</p>`}
    </section>
  `;
}

function renderExecutionPanel(subagents) {
  return `
    <section class="panel">
      <h2>Execute</h2>
      <div class="form-grid cols-2">
        <div class="form-row">
          <label>Agent</label>
          <select id="subagent-execute-name">
            ${subagents.agents.map((agent) => `<option value="${escapeHtml(agent.name)}">${escapeHtml(agent.name || "unnamed")}</option>`).join("")}
          </select>
        </div>
        <label class="check-row"><input id="subagent-execute-background" type="checkbox" /> Background</label>
      </div>
      <div class="form-row"><label>Input</label><textarea id="subagent-execute-input" rows="4">Summarize current dashboard task state.</textarea></div>
      <div class="actions">
        <button class="button" type="button" data-action="subagent-execute" ${subagents.agents.length ? "" : "disabled"}>执行 SubAgent</button>
      </div>
      ${subagents.executions.length ? `
        <table class="table mt-16 subagent-execution-table">
          <thead><tr><th>Run</th><th>Agent</th><th>Status</th><th>Output</th></tr></thead>
          <tbody>
            ${subagents.executions.map((execution) => `
              <tr>
                <td>${escapeHtml(execution.run_id)}</td>
                <td>${escapeHtml(execution.agent_name)}<br><span class="metric-label">${escapeHtml(execution.handoff_tool || "")}</span></td>
                <td>${pill(execution.status, execution.status === "completed" ? "ok" : "warn")}</td>
                <td>${escapeHtml(execution.output || "-")}</td>
              </tr>
            `).join("")}
          </tbody>
        </table>
      ` : `<p class="empty">暂无执行记录。</p>`}
    </section>
  `;
}

function transferToolName(name = "") {
  const normalized = String(name || "").trim().replace(/[^a-zA-Z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
  return normalized ? `transfer_to_${normalized}` : "transfer_to_<agent>";
}
