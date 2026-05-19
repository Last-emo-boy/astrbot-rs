import { escapeHtml, jsonBlock } from "../dom.js";
import { state } from "../state.js";
import { button, chip, closurePill, dataTable, dialog, formField, markdownViewer, metric, pill, uiState } from "./shared.js";
export function renderProviders() {
  const summary = state.status?.providers || state.providerCatalog?.summary;
  if (!summary && !state.providerCatalog) return `<div class="panel"><p class="empty">Provider 状态不可用。</p></div>`;
  const catalog = sourceProviderCatalog();
  const allProviders = catalog.providers || catalog.chat_providers || [];
  const templates = providerTemplates(catalog);
  const activeTab = state.providerTab || "chat_completion";
  const tabProviders = allProviders.filter((provider) => providerCategory(provider) === activeTab);
  const sources = providerSources(catalog, allProviders);
  const selectedSource = selectedProviderSource(sources);
  const sourceProviders = selectedSource
    ? allProviders.filter((provider) => providerCategory(provider) === "chat_completion" && providerSourceId(provider) === selectedSource.id)
    : [];
  return `
    <section class="provider-page" data-page="providers">
      <div class="dashboard-banner compact">
        <div>
          <div class="eyebrow">Provider</div>
          <h2>模型提供商</h2>
          <p>配置对话模型、Agent 执行器、STT、TTS、Embedding 和 Rerank provider。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新", action: "load-providers", variant: "secondary", icon: "↻" })}
          ${activeTab !== "chat_completion" ? button({ label: "新增模型提供商", action: "provider-dialog-open", variant: "primary", icon: "+" }) : ""}
        </div>
      </div>
      ${renderProviderMetrics(summary, catalog)}
      ${renderProviderTypeTabs(activeTab)}
      ${catalog.unavailable ? uiState({ state: "error", title: "Provider API unavailable", message: catalog.unavailable }) : ""}
      ${activeTab === "chat_completion"
        ? renderChatProviderWorkspace({ catalog, sources, selectedSource, sourceProviders, templates, summary })
        : renderTypedProviderCards(activeTab, tabProviders, templates, defaultProviderId(activeTab, summary, catalog))}
      ${renderProviderDialogs({ catalog, templates, selectedSource, sourceProviders, allProviders })}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近 Provider 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

const PROVIDER_TABS = [
  ["chat_completion", "对话", "💬"],
  ["agent_runner", "Agent 执行器", "⚙"],
  ["speech_to_text", "语音转文字", "🎙"],
  ["text_to_speech", "文字转语音", "🔊"],
  ["embedding", "嵌入(Embedding)", "{}"],
  ["rerank", "重排序(Rerank)", "⇅"],
];

function sourceProviderCatalog() {
  const catalog = state.providerCatalog || {};
  return catalog.data || catalog;
}

function providerTemplates(catalog) {
  const templateMap = catalog.config_schema?.provider?.config_template;
  if (templateMap && typeof templateMap === "object") return templateMap;
  const templates = {};
  for (const template of catalog.templates || []) {
    templates[template.label || template.provider_type] = {
      id: template.provider_type,
      type: template.provider_type,
      provider_type: "chat_completion",
      provider: providerKind(template.provider_type),
      model: template.default_model,
      api_base: template.default_api_base,
      key: null,
      enable: true,
    };
  }
  return templates;
}

function renderProviderMetrics(summary = {}, catalog = {}) {
  const items = [
    ["Chat", summary.chat_provider_count ?? providerCount(catalog, "chat_completion"), summary.default_chat_provider_id || catalog.default_chat_provider_id],
    ["STT", summary.speech_to_text_provider_count ?? providerCount(catalog, "speech_to_text"), summary.default_speech_to_text_provider_id],
    ["TTS", summary.text_to_speech_provider_count ?? providerCount(catalog, "text_to_speech"), summary.default_text_to_speech_provider_id],
    ["Embedding", summary.embedding_provider_count ?? providerCount(catalog, "embedding"), summary.default_embedding_provider_id],
    ["Rerank", summary.rerank_provider_count ?? providerCount(catalog, "rerank"), summary.default_rerank_provider_id],
  ];
  return `
    <div class="grid cols-3 provider-metrics">
      ${items.map(([label, count, defaultId]) => metric(label, count ?? 0, defaultId ? `默认 ${defaultId}` : "未配置默认 provider")).join("")}
    </div>
  `;
}

function renderProviderTypeTabs(activeTab) {
  return `
    <div class="provider-type-tabs" role="tablist">
      ${PROVIDER_TABS.map(([id, label, icon]) => `
        <button type="button" class="${id === activeTab ? "active" : ""}" role="tab" aria-selected="${id === activeTab ? "true" : "false"}" data-action="provider-tab" data-tab="${escapeHtml(id)}">
          <span>${escapeHtml(icon)}</span>${escapeHtml(label)}
        </button>
      `).join("")}
    </div>
  `;
}

function renderChatProviderWorkspace({ catalog, sources, selectedSource, sourceProviders, templates, summary }) {
  return `
    <div class="provider-workspace">
      <aside class="panel provider-source-panel">
        <div class="panel-title-row">
          <h2>提供商源</h2>
          <div class="ui-menu">
            <button class="button ghost" type="button">新增</button>
            <div class="ui-menu-list provider-source-add-list">
              ${Object.entries(templates)
                .filter(([, template]) => providerCategory(template) === "chat_completion")
                .map(([name, template]) => `<button type="button" data-action="provider-source-add" data-template="${escapeHtml(name)}">${providerLogo(template)} ${escapeHtml(name)}</button>`)
                .join("") || `<button type="button" disabled>暂无模板</button>`}
            </div>
          </div>
        </div>
        ${sources.length ? `
          <div class="provider-source-list">
            ${sources.map((source) => `
              <button type="button" class="provider-source-item ${selectedSource?.id === source.id ? "active" : ""}" data-action="provider-source-select" data-source="${escapeHtml(source.id)}">
                <span class="provider-logo">${providerLogo(source)}</span>
                <strong>${escapeHtml(source.id)}</strong>
                <small>${escapeHtml(source.api_base || "N/A")}</small>
              </button>
            `).join("")}
          </div>
        ` : uiState({ state: "empty", title: "暂无提供商源", message: "从新增菜单创建 OpenAI、Mock 或兼容源。", compact: true })}
      </aside>
      <section class="panel provider-config-card">
        ${selectedSource ? renderProviderSourceEditor(selectedSource, sourceProviders, defaultProviderId("chat_completion", summary, catalog)) : uiState({ state: "empty", title: "请选择一个提供商源", message: "源配置和模型列表会显示在这里。" })}
      </section>
    </div>
  `;
}

function renderProviderSourceEditor(source, sourceProviders, defaultId) {
  return `
    <div class="provider-config-header">
      <div>
        <h2>${escapeHtml(source.id)}</h2>
        <p>${escapeHtml(source.api_base || "N/A")}</p>
      </div>
      <div class="actions compact">
        ${button({ label: "保存配置", action: "provider-source-save", variant: "secondary", icon: "✓" })}
        ${button({ label: "删除源", action: "provider-source-delete", variant: "ghost", icon: "×", attrs: { "data-source": source.id } })}
      </div>
    </div>
    <div class="provider-source-form">
      ${formField({ id: "provider-source-id", label: "ID", value: source.id, required: true, hint: "提供商源唯一 ID（不是模型 provider ID）。" })}
      ${formField({ id: "provider-source-type", label: "Type", value: source.type || "openai_chat_completion", required: true })}
      ${formField({ id: "provider-source-provider", label: "Provider", value: source.provider || providerKind(source.type) })}
      ${formField({ id: "provider-source-api-base", label: "API Base", value: source.api_base || "", placeholder: "https://api.openai.com/v1" })}
      ${formField({ id: "provider-source-key", label: "API Key", type: "password", value: source.key || "", placeholder: "sk-..." })}
      ${formField({ id: "provider-source-proxy", label: "Proxy", value: source.proxy || "", placeholder: "http://127.0.0.1:7890" })}
      ${formField({ id: "provider-source-enabled", label: "Enabled", type: "switch", value: source.enable !== false })}
    </div>
    <details class="provider-advanced" open>
      <summary>高级配置...</summary>
      <div class="provider-source-form compact">
        ${formField({ id: "provider-source-timeout", label: "Timeout seconds", type: "number", value: source.timeout_secs || 120 })}
        ${formField({ id: "provider-source-extra", label: "Extra body JSON", type: "json", value: source.custom_extra_body || {}, rows: 4 })}
      </div>
    </details>
    ${renderProviderModelsPanel(source, sourceProviders, defaultId)}
  `;
}

function renderProviderModelsPanel(source, sourceProviders, defaultId) {
  const availableModels = state.providerModels || [];
  const configuredModels = new Set(sourceProviders.map((provider) => provider.model).filter(Boolean));
  const entries = [
    ...sourceProviders.map((provider) => ({ type: "configured", provider })),
    ...availableModels
      .filter((model) => !configuredModels.has(modelName(model)))
      .map((model) => ({ type: "available", model: modelName(model), metadata: model.metadata || state.providerModelMetadata?.[modelName(model)] })),
  ].filter((entry) => {
    const term = (state.providerModelSearch || "").trim().toLowerCase();
    if (!term) return true;
    if (entry.type === "configured") return `${entry.provider.id} ${entry.provider.model || ""}`.toLowerCase().includes(term);
    return entry.model.toLowerCase().includes(term);
  });
  return `
    <section class="provider-models-panel">
      <div class="panel-title-row">
        <h2>已配置的模型 <small>${availableModels.length ? `可用模型 ${availableModels.length}` : ""}</small></h2>
        <label class="provider-model-search">
          <span>搜索</span>
          <input id="provider-model-search" value="${escapeHtml(state.providerModelSearch || "")}" placeholder="搜索模型或 Provider ID" />
          <button class="button ghost" type="button" data-action="provider-model-search">筛选</button>
        </label>
        <div class="actions compact">
          ${button({ label: "获取模型列表", action: "provider-source-models", variant: "secondary", icon: "↓" })}
          ${button({ label: "自定义模型", action: "provider-manual-open", variant: "ghost", icon: "+" })}
        </div>
      </div>
      ${entries.length ? `
        <div class="provider-model-list">
          ${entries.map((entry) => entry.type === "configured" ? renderConfiguredModel(entry.provider, defaultId) : renderAvailableModel(source, entry.model, entry.metadata)).join("")}
        </div>
      ` : uiState({ state: "empty", title: "暂无已配置的模型", message: "点击获取模型列表，或手动添加模型 ID。", compact: true })}
    </section>
  `;
}

function renderConfiguredModel(provider, defaultId) {
  const metadata = state.providerModelMetadata?.[provider.model] || {};
  return `
    <article class="provider-model-row configured">
      <div>
        <strong>${escapeHtml(provider.id)}</strong>
        <small>${escapeHtml(provider.model || "-")} ${renderModelCapabilities(metadata, provider)}</small>
      </div>
      <div class="actions compact">
        ${provider.id === defaultId ? pill("默认", "ok") : ""}
        ${pill(provider.enable !== false && provider.enabled !== false ? "启用" : "停用", provider.enable !== false && provider.enabled !== false ? "ok" : "warn")}
        ${renderProviderStatus(provider.id)}
        <button class="button ghost" type="button" data-action="provider-toggle" data-provider="${escapeHtml(provider.id)}">${provider.enable !== false && provider.enabled !== false ? "停用" : "启用"}</button>
        <button class="button secondary" type="button" data-action="provider-check" data-provider="${escapeHtml(provider.id)}">测试</button>
        <button class="button ghost" type="button" data-action="provider-edit-open" data-provider="${escapeHtml(provider.id)}">配置</button>
        <button class="button ghost" type="button" data-action="provider-copy" data-provider="${escapeHtml(provider.id)}">复制</button>
        <button class="button ghost" type="button" data-action="provider-delete" data-provider="${escapeHtml(provider.id)}">删除</button>
      </div>
    </article>
  `;
}

function renderAvailableModel(source, model, metadata) {
  return `
    <article class="provider-model-row available" data-action="provider-model-add" data-source="${escapeHtml(source.id)}" data-model="${escapeHtml(model)}">
      <div>
        <strong>${escapeHtml(model)}</strong>
        <small>${renderModelCapabilities(metadata, { model })}</small>
      </div>
      <button class="button ghost" type="button" tabindex="-1">添加</button>
    </article>
  `;
}

function renderTypedProviderCards(activeTab, providers, templates, defaultId) {
  const tabLabel = PROVIDER_TABS.find(([id]) => id === activeTab)?.[1] || activeTab;
  return `
    <section class="panel">
      <div class="panel-title-row">
        <h2>${escapeHtml(tabLabel)}</h2>
        ${button({ label: "新增模型提供商", action: "provider-dialog-open", variant: "secondary", icon: "+" })}
      </div>
      ${providers.length ? `
        <div class="ui-card-grid cols-3">
          ${providers.map((provider) => `
            <article class="ui-card provider-card">
              <header>
                <h3>${providerLogo(provider)} ${escapeHtml(provider.id)}</h3>
                <p>${escapeHtml(provider.type || provider.provider || activeTab)}</p>
              </header>
              <div class="ui-chip-row">
                ${provider.id === defaultId ? chip("默认", "ok") : ""}
                ${chip(provider.enable !== false && provider.enabled !== false ? "已启用" : "已停用", provider.enable !== false && provider.enabled !== false ? "ok" : "warn")}
                ${provider.model ? chip(provider.model) : ""}
                ${provider.dimensions || provider.embedding_dimensions ? chip(`dim ${provider.dimensions || provider.embedding_dimensions}`) : ""}
                ${renderProviderStatus(provider.id)}
              </div>
              <footer class="actions">
                <button class="button secondary" type="button" data-action="provider-check" data-provider="${escapeHtml(provider.id)}">测试</button>
                <button class="button ghost" type="button" data-action="provider-toggle" data-provider="${escapeHtml(provider.id)}">${provider.enable !== false && provider.enabled !== false ? "停用" : "启用"}</button>
                <button class="button ghost" type="button" data-action="provider-edit-open" data-provider="${escapeHtml(provider.id)}">编辑</button>
                <button class="button ghost" type="button" data-action="provider-copy" data-provider="${escapeHtml(provider.id)}">复制</button>
                <button class="button ghost" type="button" data-action="provider-delete" data-provider="${escapeHtml(provider.id)}">删除</button>
              </footer>
            </article>
          `).join("")}
        </div>
      ` : uiState({ state: "empty", title: `暂无${tabLabel}类型的模型提供商`, message: "点击新增模型提供商添加。", compact: true })}
      <div class="template-hints">${Object.entries(templates).filter(([, template]) => providerCategory(template) === activeTab).map(([name, template]) => chip(`${name} ${template.type || ""}`)).join("")}</div>
    </section>
  `;
}

function renderProviderDialogs({ templates, selectedSource, sourceProviders, allProviders }) {
  const selectedProvider = allProviders.find((provider) => provider.id === state.providerEditId) || null;
  return `
    ${dialog({
      id: "provider-agent-runner-dialog",
      title: "请前往「配置文件」页测试 Agent 执行器",
      open: Boolean(state.providerAgentRunnerDialog),
      maxWidth: "520px",
      body: `
        <p class="ui-dialog-message">Agent 执行器的测试请在「配置文件」页进行。</p>
        <ol class="provider-dialog-list">
          <li>找到对应的配置文件并打开。</li>
          <li>找到 Agent 执行方式部分，修改执行器后点击保存。</li>
          <li>点击右下角的聊天按钮进行测试。</li>
        </ol>
        <p class="ui-dialog-message">要让机器人应用这个 Agent 执行器，也需要前往修改 Agent 执行器。</p>
      `,
      actions: [
        { label: "好的", variant: "ghost", value: "close", action: "provider-agent-runner-close" },
        { label: "点击前往", action: "provider-agent-runner-go" },
      ],
    })}
    ${dialog({
      id: "provider-add-dialog",
      title: "模型提供商",
      open: state.providerDialog === "add-provider",
      maxWidth: "1100px",
      body: renderAddProviderDialogBody(templates),
      actions: [{ label: "关闭", variant: "ghost", value: "close", action: "provider-dialog-close" }],
    })}
    ${dialog({
      id: "provider-manual-model-dialog",
      title: "添加自定义模型",
      open: state.providerDialog === "manual-model",
      maxWidth: "420px",
      body: `
        ${formField({ id: "provider-manual-model", label: "模型 ID", value: "", placeholder: "gpt-4.1-mini", required: true })}
        ${formField({ id: "provider-manual-preview", label: "显示 ID", value: selectedSource ? `${selectedSource.id}/<model>` : "", readonly: true, hint: "生成规则：源ID/模型ID" })}
        <div class="actions"><button class="button" type="button" data-action="provider-manual-add">添加</button></div>
      `,
      actions: [
        { label: "取消", variant: "ghost", value: "close", action: "provider-dialog-close" },
      ],
    })}
    ${dialog({
      id: "provider-edit-dialog",
      title: selectedProvider?.id || "编辑模型提供商",
      open: Boolean(state.providerEditId),
      maxWidth: "820px",
      body: selectedProvider ? renderProviderEditForm(selectedProvider) : "",
      actions: [
        { label: "取消", variant: "ghost", value: "close", action: "provider-dialog-close" },
        { label: "保存", action: "provider-edit-save" },
      ],
    })}
    ${dialog({
      id: "provider-config-dialog",
      title: state.providerTemplateDraft?.id || "新增模型提供商",
      open: state.providerDialog === "provider-config",
      maxWidth: "820px",
      body: state.providerTemplateDraft ? renderProviderEditForm(state.providerTemplateDraft, { prefix: "provider-new" }) : "",
      actions: [
        { label: "取消", variant: "ghost", value: "close", action: "provider-dialog-close" },
        { label: "保存", action: "provider-template-save" },
      ],
    })}
  `;
}

function renderAddProviderDialogBody(templates) {
  const addProviderTabs = PROVIDER_TABS.filter(([tab]) => tab !== "chat_completion");
  return `
    <div class="provider-template-grid">
      ${addProviderTabs.map(([tab, label]) => `
        <section>
          <h3>${escapeHtml(label)}</h3>
          <div class="ui-card-grid cols-2">
            ${Object.entries(templates)
              .filter(([, template]) => providerCategory(template) === tab)
              .map(([name, template]) => `
                <button type="button" class="provider-template-card" data-action="provider-template-select" data-template="${escapeHtml(name)}">
                  <strong>${providerLogo(template)} ${escapeHtml(name)}</strong>
                  <span>${escapeHtml(template.type || "")}</span>
                </button>
              `).join("") || `<p class="empty">暂无该类型的提供商模板</p>`}
          </div>
        </section>
      `).join("")}
    </div>
  `;
}

function renderProviderEditForm(provider, { prefix = "provider-edit" } = {}) {
  const category = providerCategory(provider);
  return `
    <div class="provider-source-form">
      ${formField({ id: `${prefix}-id`, label: "ID", value: provider.id || "", required: true, hint: "不建议修改 ID，可能导致默认模型或插件配置失效。" })}
      ${formField({ id: `${prefix}-type`, label: "Type", value: provider.type || "", required: true })}
      ${formField({ id: `${prefix}-provider-type`, label: "Provider type", value: category, readonly: true })}
      ${formField({ id: `${prefix}-model`, label: "Model", value: provider.model || "" })}
      ${formField({ id: `${prefix}-api-base`, label: "API Base", value: provider.api_base || "" })}
      ${formField({ id: `${prefix}-key`, label: "API Key", type: "password", value: provider.key || "" })}
      ${formField({ id: `${prefix}-timeout`, label: "Timeout seconds", type: "number", value: provider.timeout_secs || 120 })}
      <div class="provider-inline-field">
        ${formField({ id: `${prefix}-dimensions`, label: "Embedding dim", type: "number", value: provider.dimensions || provider.embedding_dimensions || "" })}
        ${category === "embedding" ? button({ label: "检测维度", action: "provider-embedding-dim", variant: "secondary", icon: "↓", attrs: { "data-prefix": prefix } }) : ""}
      </div>
      ${formField({ id: `${prefix}-enabled`, label: "Enabled", type: "switch", value: provider.enable !== false && provider.enabled !== false })}
      ${formField({ id: `${prefix}-extra-json`, label: "Adapter-specific JSON", type: "json", value: adapterSpecificConfig(provider), rows: 5, hint: "保留模板中的专属字段，例如 voice、app_id、headers 或 provider_options。" })}
    </div>
    <input type="hidden" id="${escapeHtml(prefix)}-provider-source-id" value="${escapeHtml(provider.provider_source_id || "")}" />
  `;
}

function providerSources(catalog, allProviders) {
  const explicit = catalog.provider_sources || [];
  if (explicit.length) return explicit;
  const byId = new Map();
  for (const provider of allProviders.filter((item) => providerCategory(item) === "chat_completion")) {
    const sourceId = providerSourceId(provider);
    if (!byId.has(sourceId)) {
      byId.set(sourceId, {
        id: sourceId,
        type: provider.type,
        provider_type: "chat_completion",
        provider: provider.provider || providerKind(provider.type),
        api_base: provider.api_base,
        key: provider.key,
        enable: provider.enable !== false && provider.enabled !== false,
      });
    }
  }
  return [...byId.values()];
}

function selectedProviderSource(sources) {
  if (!sources.length) return null;
  const selected = sources.find((source) => source.id === state.providerSelectedSourceId);
  return selected || sources[0];
}

function providerCount(catalog, category) {
  const providers = catalog.providers || catalog.chat_providers || [];
  return providers.filter((provider) => providerCategory(provider) === category).length;
}

function providerCategory(provider = {}) {
  const type = provider.provider_type || provider.category || provider.type || "";
  if (PROVIDER_TABS.some(([id]) => id === type)) return type;
  if (type.includes("speech_to_text") || type.includes("stt")) return "speech_to_text";
  if (type.includes("text_to_speech") || type.includes("tts")) return "text_to_speech";
  if (type.includes("embedding")) return "embedding";
  if (type.includes("rerank")) return "rerank";
  if (["dify", "coze", "dashscope", "deerflow", "fastgpt"].includes(type)) return "agent_runner";
  return "chat_completion";
}

function providerSourceId(provider = {}) {
  return provider.provider_source_id || provider.source_id || provider.id || "";
}

function providerKind(type = "") {
  return String(type || "openai").split("_")[0] || "openai";
}

function providerLogo(provider = {}) {
  const kind = provider.provider || providerKind(provider.type);
  return escapeHtml(String(kind || "?").slice(0, 2).toUpperCase());
}

function modelName(model) {
  return typeof model === "string" ? model : model?.name || "";
}

function renderModelCapabilities(metadata = {}, provider = {}) {
  const chips = [];
  const inputs = metadata?.modalities?.input || provider.modalities || [];
  if (inputs.includes("image")) chips.push("vision");
  if (metadata?.tool_call || provider.modalities?.includes("tool_use")) chips.push("tool");
  if (metadata?.reasoning) chips.push("reasoning");
  const ctx = metadata?.limit?.context || provider.max_context_tokens;
  if (ctx) chips.push(ctx >= 1000000 ? `${Math.round(ctx / 1000000)}M` : `${Math.round(ctx / 1000)}K`);
  return chips.map((item) => `<span class="tag">${escapeHtml(item)}</span>`).join(" ");
}

function renderProviderStatus(providerId) {
  const status = state.providerStatuses?.[providerId];
  if (!status?.status) return "";
  const kind = status.status === "available" ? "ok" : status.status === "unavailable" ? "error" : "warn";
  const text = status.status === "available" ? "可用" : status.status === "unavailable" ? "不可用" : "检查中";
  const title = status.error ? ` title="${escapeHtml(status.error)}"` : "";
  return `<span class="status-pill ${escapeHtml(kind)}"${title}>${escapeHtml(text)}</span>`;
}

function defaultProviderId(category, summary = {}, catalog = {}) {
  if (category === "speech_to_text") return summary.default_speech_to_text_provider_id || catalog.default_speech_to_text_provider_id || "";
  if (category === "text_to_speech") return summary.default_text_to_speech_provider_id || catalog.default_text_to_speech_provider_id || "";
  if (category === "embedding") return summary.default_embedding_provider_id || catalog.default_embedding_provider_id || "";
  if (category === "rerank") return summary.default_rerank_provider_id || catalog.default_rerank_provider_id || "";
  return summary.default_chat_provider_id || catalog.default_chat_provider_id || "";
}

function adapterSpecificConfig(provider = {}) {
  const excluded = new Set([
    "id",
    "type",
    "provider_type",
    "category",
    "model",
    "api_base",
    "key",
    "api_key",
    "timeout_secs",
    "dimensions",
    "embedding_dimensions",
    "enable",
    "enabled",
    "provider_source_id",
  ]);
  return Object.fromEntries(Object.entries(provider).filter(([key]) => !excluded.has(key)));
}

export function renderPlatforms() {
  const catalog = sourcePlatformCatalog();
  const platforms = state.status?.platforms || catalog.summary || {};
  if (!platforms && !catalog) return `<div class="panel"><p class="empty">平台状态不可用。</p></div>`;
  const configured = configuredPlatforms();
  const templates = platformTemplates(catalog);
  return `
    <section class="platform-page" data-page="platforms">
      <div class="dashboard-banner compact">
        <div>
          <div class="eyebrow">Platform</div>
          <h2>平台适配器</h2>
          <p>配置消息平台、ABConf 路由和运行状态，保持与源端平台页一致的管理流程。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新", action: "load-platforms", variant: "secondary", icon: "↻" })}
          ${button({ label: "新增平台适配器", action: "platform-dialog-open", variant: "primary", icon: "+" })}
        </div>
      </div>
      ${renderPlatformMetrics(platforms, configured)}
      ${catalog.unavailable ? uiState({ state: "error", title: "Platform API unavailable", message: catalog.unavailable }) : ""}
      ${configured.length ? renderPlatformCards(configured) : uiState({ state: "empty", title: "暂无平台适配器", message: "点击新增平台适配器，从模板创建 Console、WebChat 或 OneBot 配置。" })}
      ${renderPlatformConsole()}
      ${renderPlatformDialogs({ templates, platforms: configured })}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近 Platform 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

function sourcePlatformCatalog() {
  const catalog = state.platformCatalog || {};
  return catalog.data || catalog;
}

function configuredPlatforms() {
  const config = state.platformConfig || {};
  const catalog = sourcePlatformCatalog();
  const fullConfigs = config.platforms || config.platform || [];
  if (Array.isArray(fullConfigs) && fullConfigs.length) {
    return fullConfigs.map(normalizePlatform);
  }
  return (catalog.platforms || []).map(normalizePlatform);
}

function normalizePlatform(platform = {}) {
  const type = platform.type || platform.platform_type || platform.platform || "";
  return {
    ...platform,
    id: platform.id || "",
    type,
    platform_type: type,
    enabled: platform.enabled ?? platform.enable ?? true,
    enable: platform.enable ?? platform.enabled ?? true,
    name: platform.name || "",
    options: platform.options || {},
    secrets: platform.secrets || {},
  };
}

function platformTemplates(catalog = sourcePlatformCatalog()) {
  const metadataTemplates = state.platformMetadata?.platform_group?.metadata?.platform?.config_template;
  if (metadataTemplates && typeof metadataTemplates === "object") {
    return Object.entries(metadataTemplates).map(([name, template]) => platformTemplateFromObject(name, template));
  }
  const entries = (catalog.templates || []).map((template) => platformTemplateFromObject(template.label || template.platform_type, template));
  return entries.length ? entries : [
    platformTemplateFromObject("WebChat", { platform_type: "webchat", label: "WebChat", runtime_supported: true }),
    platformTemplateFromObject("Console", { platform_type: "console", label: "Console", runtime_supported: true }),
    platformTemplateFromObject("OneBot", { platform_type: "onebot", label: "OneBot", runtime_supported: true }),
    platformTemplateFromObject("Mock", { platform_type: "mock", label: "Mock", runtime_supported: true }),
  ];
}

function platformTemplateFromObject(name, template = {}) {
  const type = template.type || template.platform_type || template.id || name;
  return {
    name,
    label: template.label || name,
    type,
    runtime_supported: template.runtime_supported !== false,
    config: normalizePlatform({
      id: template.id || type,
      type,
      name: template.name || template.label || name,
      enabled: template.enabled ?? template.enable ?? true,
      options: template.options || defaultPlatformOptions(type),
      secrets: template.secrets || defaultPlatformSecrets(type),
    }),
  };
}

function defaultPlatformOptions(type) {
  if (type === "onebot" || type === "aiocqhttp") {
    return { ws_reverse_host: "0.0.0.0", ws_reverse_port: 6199 };
  }
  return {};
}

function defaultPlatformSecrets(type) {
  if (type === "telegram") return { telegram_token: "" };
  if (type === "onebot" || type === "aiocqhttp") return { ws_reverse_token: "" };
  return {};
}

function renderPlatformMetrics(summary = {}, platforms = configuredPlatforms()) {
  return `
    <div class="grid cols-4 platform-metrics">
      ${metric("Online Platforms", summary.platform_count ?? platforms.length, (summary.platform_ids || platforms.map((item) => item.id)).join(", ") || "未配置")}
      ${metric("WebChat", summary.webchat_platform_count ?? platformCountByType(platforms, "webchat"), "前端 Chat 依赖此能力")}
      ${metric("OneBot", summary.onebot_platform_count ?? platformCountByType(platforms, "onebot"), "真实 IM 适配入口")}
      ${metric("Recording Sink", summary.recording_sink_count ?? 0, "用于历史与测试")}
    </div>
  `;
}

function platformCountByType(platforms, type) {
  return platforms.filter((platform) => platform.type === type).length;
}

function renderPlatformCards(platforms) {
  return `
    <section class="platform-card-grid">
      ${platforms.map((platform) => renderPlatformCard(platform)).join("")}
    </section>
  `;
}

function renderPlatformCard(platform) {
  const stat = platformRuntimeStat(platform.id);
  const routes = platformRoutesFor(platform.id);
  const check = state.platformChecks?.[platform.id];
  return `
    <article class="ui-card platform-card">
      <header>
        <span class="platform-logo">${platformLogo(platform.type)}</span>
        <div>
          <h3>${escapeHtml(platform.id)}</h3>
          <p>${escapeHtml(platform.name || platform.type || "-")}</p>
        </div>
      </header>
      <div class="ui-chip-row">
        ${chip(platform.type || "unknown")}
        ${chip(platform.enabled ? "已启用" : "已停用", platform.enabled ? "ok" : "warn")}
        ${routes.length ? chip(`${routes.length} 条路由`, "info") : chip("未绑定 ABConf", "warn")}
        ${renderPlatformRuntimeChips(stat, platform)}
        ${renderPlatformCheckStatus(check)}
      </div>
      <dl class="platform-card-meta">
        <div><dt>Options</dt><dd>${Object.keys(platform.options || {}).length}</dd></div>
        <div><dt>Secrets</dt><dd>${Object.keys(platform.secrets || {}).length}</dd></div>
        <div><dt>Messages</dt><dd>${escapeHtml(platformMessageCount(platform.id, platform.type))}</dd></div>
      </dl>
      <footer class="actions">
        <button class="button secondary" type="button" data-action="platform-check" data-platform="${escapeHtml(platform.id)}">检查</button>
        <button class="button ghost" type="button" data-action="platform-toggle" data-platform="${escapeHtml(platform.id)}">${platform.enabled ? "停用" : "启用"}</button>
        <button class="button ghost" type="button" data-action="platform-edit-open" data-platform="${escapeHtml(platform.id)}">编辑</button>
        <button class="button ghost" type="button" data-action="platform-delete" data-platform="${escapeHtml(platform.id)}">删除</button>
      </footer>
    </article>
  `;
}

function renderPlatformRuntimeChips(stat, platform) {
  const chips = [];
  if (stat?.status && stat.status !== "running") {
    chips.push(chip(runtimeStatusLabel(stat.status), statusKind(stat.status)));
  }
  if ((stat?.error_count || 0) > 0) {
    chips.push(`<button class="ui-chip error" type="button" data-action="platform-error-open" data-platform="${escapeHtml(platform.id)}">${escapeHtml(stat.error_count)} 个错误</button>`);
  }
  if (stat?.unified_webhook && (platform.options?.webhook_uuid || platform.webhook_uuid)) {
    chips.push(`<button class="ui-chip info" type="button" data-action="platform-webhook-open" data-webhook="${escapeHtml(platform.options?.webhook_uuid || platform.webhook_uuid)}">Webhook</button>`);
  }
  return chips.join("");
}

function renderPlatformCheckStatus(check) {
  if (!check?.status) return "";
  const kind = check.status === "available" ? "ok" : check.status === "pending" ? "warn" : "error";
  const label = check.status === "available" ? "配置可构建" : check.status === "pending" ? "检查中" : "检查失败";
  return `<span class="status-pill ${escapeHtml(kind)}" title="${escapeHtml(check.message || "")}">${escapeHtml(label)}</span>`;
}

function statusKind(status) {
  if (status === "running" || status === "available") return "ok";
  if (status === "pending") return "warn";
  if (status === "error" || status === "stopped") return "error";
  return "";
}

function runtimeStatusLabel(status) {
  return {
    running: "运行中",
    error: "错误",
    pending: "启动中",
    stopped: "已停止",
  }[status] || "未知";
}

function platformRuntimeStat(platformId) {
  const stats = state.platformRuntimeStats?.platforms || [];
  return stats.find((item) => item.id === platformId || item.platform_id === platformId) || null;
}

function platformMessageCount(platformId, platformType) {
  const counts = state.stats?.platform_counts || [];
  const item = counts.find((entry) => entry.platform_id === platformId || entry.platform_type === platformType);
  return item?.count ?? 0;
}

function platformRoutesFor(platformId) {
  const routes = normalizedPlatformRoutes();
  return routes.filter((route) => isUmoMatchPlatform(route.pattern || route.umo, platformId));
}

function normalizedPlatformRoutes() {
  const routes = state.platformRoutes || {};
  if (Array.isArray(routes.routes)) return routes.routes;
  const routing = routes.routing || {};
  return Object.entries(routing).map(([pattern, config_id]) => ({ pattern, config_id }));
}

function isUmoMatchPlatform(umo, platformId) {
  const parts = String(umo || "").split(":");
  if (parts.length !== 3) return false;
  return parts[0] === platformId || parts[0] === "*" || parts[0] === "";
}

function renderPlatformConsole() {
  const visible = platformConsoleVisible();
  const entries = state.logs?.snapshot?.entries || [];
  return `
    <section class="panel platform-console-panel">
      <div class="panel-title-row">
        <h2>日志</h2>
        <div class="actions compact">
          ${button({ label: visible ? "收起" : "展开", action: "platform-console-toggle", variant: "ghost", icon: visible ? "⌃" : "⌄" })}
        </div>
      </div>
      ${visible ? `
        <div class="platform-console">
          ${entries.length ? entries.slice(-80).map((entry) => `<div><span>${escapeHtml(entry.timestamp || entry.time || "")}</span> ${escapeHtml(entry.level || "INFO")} ${escapeHtml(entry.message || entry.target || JSON.stringify(entry))}</div>`).join("") : `<p>暂无日志快照，展开后可点击刷新加载最新日志。</p>`}
        </div>
        <div class="actions compact">${button({ label: "刷新日志", action: "load-logs", variant: "secondary", icon: "↻" })}</div>
      ` : ""}
    </section>
  `;
}

function platformConsoleVisible() {
  if (state.platformShowConsole !== null) return Boolean(state.platformShowConsole);
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem("platformPage_showConsole") === "true";
}

function renderPlatformDialogs({ templates, platforms }) {
  const editPlatform = platforms.find((platform) => platform.id === state.platformEditId) || null;
  const errorStat = state.platformErrorId ? platformRuntimeStat(state.platformErrorId) : null;
  return `
    ${dialog({
      id: "platform-add-dialog",
      title: "新增平台适配器",
      open: state.platformDialog === "add-platform",
      maxWidth: "1060px",
      body: renderAddPlatformDialogBody(templates),
      actions: [
        { label: "取消", variant: "ghost", value: "close", action: "platform-dialog-close" },
        { label: "保存", action: "platform-save-new" },
      ],
    })}
    ${dialog({
      id: "platform-edit-dialog",
      title: editPlatform ? `编辑 ${editPlatform.id}` : "编辑平台适配器",
      open: Boolean(state.platformEditId),
      maxWidth: "1100px",
      body: editPlatform ? renderEditPlatformDialogBody(editPlatform) : "",
      actions: [
        { label: "取消", variant: "ghost", value: "close", action: "platform-dialog-close" },
        { label: "保存", action: "platform-save-edit" },
      ],
    })}
    ${dialog({
      id: "platform-error-dialog",
      title: errorStat ? `平台错误：${errorStat.id || errorStat.platform_id}` : "平台错误",
      open: Boolean(state.platformErrorId),
      maxWidth: "720px",
      body: errorStat ? renderPlatformErrorBody(errorStat) : "",
      actions: [{ label: "关闭", variant: "ghost", value: "close", action: "platform-error-close" }],
    })}
    ${dialog({
      id: "platform-webhook-dialog",
      title: "Webhook URL",
      open: Boolean(state.platformWebhookUuid),
      maxWidth: "640px",
      body: renderWebhookBody(state.platformWebhookUuid),
      actions: [{ label: "关闭", variant: "ghost", value: "close", action: "platform-webhook-close" }],
    })}
  `;
}

function renderAddPlatformDialogBody(templates) {
  const selectedName = state.platformSelectedTemplate || templates[0]?.name || "";
  const selected = templates.find((template) => template.name === selectedName) || templates[0] || null;
  const draft = state.platformDraft || selected?.config || {};
  return `
    <div class="platform-dialog-layout">
      <section class="platform-step">
        <div class="timeline-dot">1</div>
        <div>
          <h3>选择平台适配器</h3>
          <p>从运行时支持的模板创建平台配置。</p>
          <div class="platform-template-list">
            ${templates.map((template) => `
              <button type="button" class="platform-template-card ${template.name === selectedName ? "active" : ""}" data-action="platform-template-select" data-template="${escapeHtml(template.name)}">
                <span class="platform-logo">${platformLogo(template.type)}</span>
                <strong>${escapeHtml(template.label)}</strong>
                <small>${escapeHtml(template.type)}${template.runtime_supported ? "" : " · runtime unsupported"}</small>
              </button>
            `).join("") || `<p class="empty">暂无平台模板。</p>`}
          </div>
          ${renderPlatformConfigFields(draft, "platform-new")}
        </div>
      </section>
      <section class="platform-step">
        <div class="timeline-dot">2</div>
        <div>
          <div class="panel-title-row">
            <div>
              <h3>绑定配置文件</h3>
              <p>保存平台后自动写入默认 UMO 路由：platform:*:*。</p>
            </div>
            ${button({ label: state.platformShowConfigSection ? "收起" : "展开", action: "platform-config-section-toggle", variant: "ghost", icon: state.platformShowConfigSection ? "⌃" : "⌄" })}
          </div>
          ${state.platformShowConfigSection ? renderPlatformConfigBinding() : ""}
        </div>
      </section>
    </div>
  `;
}

function renderEditPlatformDialogBody(platform) {
  const draft = state.platformDraft || platform;
  return `
    <div class="platform-dialog-layout">
      <section class="platform-step">
        <div class="timeline-dot">1</div>
        <div>
          <h3>平台配置</h3>
          <p>Type 固定，其他字段会保存回 runtime config。</p>
          ${renderPlatformConfigFields(draft, "platform-edit", { idReadonly: true, typeReadonly: true })}
        </div>
      </section>
      <section class="platform-step">
        <div class="timeline-dot">2</div>
        <div>
          <div class="panel-title-row">
            <div>
              <h3>UMO 路由</h3>
              <p>按顺序匹配消息来源并绑定 ABConf。</p>
            </div>
            <div class="actions compact">
              ${state.platformRouteEdit ? button({ label: "添加路由", action: "platform-route-add", variant: "secondary", icon: "+" }) : ""}
              ${button({ label: state.platformRouteEdit ? "查看模式" : "编辑模式", action: "platform-route-edit-toggle", variant: "ghost", icon: state.platformRouteEdit ? "◌" : "✎" })}
            </div>
          </div>
          ${renderPlatformRouteTable()}
        </div>
      </section>
    </div>
  `;
}

function renderPlatformConfigFields(platform = {}, prefix, { idReadonly = false, typeReadonly = false } = {}) {
  return `
    <div class="provider-source-form platform-config-form">
      ${formField({ id: `${prefix}-id`, label: "ID", value: platform.id || "", required: true, readonly: idReadonly, hint: "不能包含 ! 或 :，否则无法组成 UMO。" })}
      ${formField({ id: `${prefix}-type`, label: "Type", value: platform.type || platform.platform_type || "", required: true, readonly: typeReadonly })}
      ${formField({ id: `${prefix}-name`, label: "Name", value: platform.name || "" })}
      ${formField({ id: `${prefix}-enabled`, label: "Enabled", type: "switch", value: platform.enabled !== false && platform.enable !== false })}
      ${formField({ id: `${prefix}-options-json`, label: "Options JSON", type: "json", value: platform.options || {}, rows: 6, hint: "适配器专属非密钥配置。" })}
      ${formField({ id: `${prefix}-secrets-json`, label: "Secrets JSON", type: "json", value: platform.secrets || {}, rows: 6, hint: "密钥字段会随 runtime config 保存。" })}
    </div>
  `;
}

function renderPlatformConfigBinding() {
  const configMode = state.platformConfigMode || "existing";
  const abconfs = platformAbconfs();
  return `
    <div class="platform-config-binding">
      <div class="platform-mode-switch">
        <button class="button ${configMode === "existing" ? "" : "ghost"}" type="button" data-action="platform-config-existing-mode">使用现有配置</button>
        <button class="button ${configMode === "new" ? "" : "ghost"}" type="button" data-action="platform-config-new-mode">创建新配置</button>
      </div>
      ${configMode === "new" ? `
        ${formField({ id: "platform-new-config-name", label: "新配置名称", value: state.platformNewConfigName || "platform-config", required: true })}
        ${formField({ id: "platform-new-config-json", label: "新配置 JSON", type: "json", value: state.platformNewConfigDraft || state.config || {}, rows: 10 })}
      ` : `
        ${formField({ id: "platform-config-select", label: "配置文件", type: "select", value: state.platformSelectedConfigId || "default", options: abconfs.map((item) => ({ value: item.id, label: item.name || item.id })) })}
      `}
    </div>
  `;
}

function platformAbconfs() {
  return state.platformAbconfs?.info_list || [{ id: "default", name: "default" }];
}

function renderPlatformRouteTable() {
  const drafts = state.platformRouteDrafts?.length ? state.platformRouteDrafts : [{ messageType: "*", sessionId: "*", configId: "default" }];
  return `
    <div class="table-scroll">
      <table class="table platform-route-table">
        <thead><tr><th>来源</th><th>配置文件</th><th>操作</th></tr></thead>
        <tbody>
          ${drafts.map((route, index) => renderPlatformRouteRow(route, index, drafts.length)).join("")}
        </tbody>
      </table>
    </div>
    <p class="empty compact">UMO 格式：platform:messageType:sessionId。* 表示全部消息类型或会话。</p>
  `;
}

function renderPlatformRouteRow(route, index, length) {
  const abconfs = platformAbconfs();
  if (!state.platformRouteEdit) {
    return `
      <tr>
        <td>${escapeHtml(messageTypeLabel(route.messageType))}: ${escapeHtml(route.sessionId === "*" ? "全部会话" : route.sessionId)}</td>
        <td>${escapeHtml(configName(route.configId))}</td>
        <td>-</td>
      </tr>
    `;
  }
  return `
    <tr>
      <td>
        <div class="platform-route-source">
          <select id="platform-route-${index}-message-type">
            ${["*", "GroupMessage", "FriendMessage"].map((type) => `<option value="${escapeHtml(type)}" ${route.messageType === type ? "selected" : ""}>${escapeHtml(messageTypeLabel(type))}</option>`).join("")}
          </select>
          <input id="platform-route-${index}-session-id" value="${escapeHtml(route.sessionId || "*")}" placeholder="*" />
        </div>
      </td>
      <td>
        <select id="platform-route-${index}-config-id">
          ${abconfs.map((item) => `<option value="${escapeHtml(item.id)}" ${item.id === route.configId ? "selected" : ""}>${escapeHtml(item.name || item.id)}</option>`).join("")}
        </select>
      </td>
      <td class="button-cell">
        <button class="button ghost" type="button" data-action="platform-route-up" data-index="${index}" ${index === 0 ? "disabled" : ""}>↑</button>
        <button class="button ghost" type="button" data-action="platform-route-down" data-index="${index}" ${index === length - 1 ? "disabled" : ""}>↓</button>
        <button class="button ghost" type="button" data-action="platform-route-delete" data-index="${index}">删除</button>
      </td>
    </tr>
  `;
}

function messageTypeLabel(type) {
  return {
    "*": "全部",
    "": "全部",
    GroupMessage: "群聊",
    FriendMessage: "私聊",
  }[type] || type;
}

function configName(configId) {
  return platformAbconfs().find((item) => item.id === configId)?.name || configId || "-";
}

function renderPlatformErrorBody(stat) {
  const last = stat.last_error || {};
  return `
    <div class="platform-error-body">
      <p><strong>Platform ID：</strong>${escapeHtml(stat.id || stat.platform_id || "")}</p>
      <p><strong>Error Count：</strong>${escapeHtml(stat.error_count || 0)}</p>
      ${last.message ? `<pre>${escapeHtml(last.message)}</pre>` : ""}
      ${last.timestamp ? `<p class="empty">Occurred at: ${escapeHtml(last.timestamp)}</p>` : ""}
      ${last.traceback ? `<pre>${escapeHtml(last.traceback)}</pre>` : ""}
    </div>
  `;
}

function renderWebhookBody(webhookUuid) {
  const url = platformWebhookUrl(webhookUuid);
  return `
    <p class="ui-dialog-message">复制该地址到平台侧回调配置。</p>
    <div class="platform-webhook-copy">
      <input id="platform-webhook-url" readonly value="${escapeHtml(url)}" />
      ${button({ label: "复制", action: "platform-webhook-copy", variant: "secondary", icon: "⧉" })}
    </div>
  `;
}

function platformWebhookUrl(webhookUuid) {
  if (!webhookUuid) return "";
  const base = state.platformConfig?.callback_api_base || "http(s)://<your-domain-or-ip>";
  return `${String(base).replace(/\/$/, "")}/api/platform/webhook/${webhookUuid}`;
}

function platformLogo(type = "") {
  const labels = {
    webchat: "WC",
    console: "CL",
    onebot: "OB",
    aiocqhttp: "OB",
    mock: "MK",
  };
  return labels[type] || String(type || "?").slice(0, 2).toUpperCase();
}

function providerTypeOptions(templates = []) {
  const options = templates.length ? templates : [
    { provider_type: "mock", label: "Mock" },
    { provider_type: "openai", label: "OpenAI Compatible" },
  ];
  return options.map((template) => `<option value="${escapeHtml(template.provider_type)}">${escapeHtml(template.label || template.provider_type)}</option>`).join("");
}

function platformTypeOptions(templates = []) {
  const options = templates.length ? templates : [
    { platform_type: "webchat", label: "WebChat" },
    { platform_type: "console", label: "Console" },
  ];
  return options.map((template) => `<option value="${escapeHtml(template.platform_type)}">${escapeHtml(template.label || template.platform_type)}</option>`).join("");
}

export function renderPlugins() {
  const lifecycle = state.pluginLifecycle || { handlers: state.status?.plugins || { handlers: [] }, plugins: [], operations: [] };
  const handlers = lifecycle.handlers || state.status?.plugins || { handlers: [], handler_count: 0 };
  const plugins = filteredInstalledPlugins(lifecycle.plugins || []);
  const failed = (lifecycle.plugins || []).filter((plugin) => plugin.state === "failed" || plugin.active === false && plugin.state === "failed");
  return `
    <section class="extension-page" data-page="extension-installed">
      ${renderExtensionTabs("plugins")}
      <div class="dashboard-banner compact">
        <div>
          <div class="eyebrow">Extension</div>
          <h2>已安装插件</h2>
          <p>插件卡片、handler snapshot、配置文件、README、source 与 lifecycle 操作。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新", action: "load-plugin-lifecycle", variant: "secondary", icon: "↻" })}
          ${button({ label: "Upload Plan", action: "plugin-upload-plan", variant: "secondary", icon: "↑" })}
        </div>
      </div>
      ${renderExtensionCapabilityStrip([
        { label: "Plugin lifecycle", level: lifecycle.unavailable ? "unavailable" : "runtime", note: "/api/management/plugins/lifecycle" },
        { label: "Config files", level: lifecycle.unavailable ? "unavailable" : "runtime", note: "read/write/delete JSON" },
        { label: "Upload zip", level: "plan_only", note: "archive validation only" },
        { label: "Hot reload probe", level: "in_memory", note: "lifecycle state transition" },
      ])}
      ${lifecycle.unavailable ? uiState({ state: "error", title: "Plugin lifecycle unavailable", message: lifecycle.unavailable }) : ""}
      ${renderPluginToolbar(lifecycle.plugins || [])}
      ${failed.length ? renderFailedPlugins(failed) : ""}
      ${state.extensionPluginView === "list" ? renderPluginList(plugins) : renderPluginGrid(plugins)}
      <div class="grid cols-3 mt-16">
        ${renderPluginUploadPanel()}
        ${renderPluginSourcePanel()}
        ${renderPluginConfigPanel(lifecycle.plugins || [])}
      </div>
      <section class="panel mt-16">
        <div class="panel-title-row"><h2>已注册 Handler</h2><span class="tag">${escapeHtml(handlers.handler_count ?? handlers.handlers?.length ?? 0)} handlers</span></div>
        ${renderHandlersTable(handlers.handlers || [])}
      </section>
      ${lifecycle.operations?.length ? `<section class="panel mt-16"><h2>Lifecycle 操作</h2>${jsonBlock(lifecycle.operations)}</section>` : ""}
      ${renderExtensionDialogs()}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近插件管理结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

export function renderMarket() {
  const market = state.market || { plugins: [] };
  const plugins = sortedMarketPlugins(filteredMarketPlugins(market.plugins || []));
  const randomPlugins = plugins.slice(0, Math.min(3, plugins.length));
  return `
    <section class="extension-page" data-page="extension-marketplace">
      ${renderExtensionTabs("market")}
      <div class="dashboard-banner compact">
        <div>
          <div class="eyebrow">Marketplace</div>
          <h2>插件市场</h2>
          <p>市场搜索、排序、插件源、安全提示、安装/更新/卸载 plan 与 README 预览。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新", action: "load-market", variant: "secondary", icon: "↻" })}
          ${button({ label: "Update All Plan", action: "plugin-update-all-plan", variant: "secondary", icon: "⇧" })}
          ${button({ label: "Update All", action: "plugin-update-all", variant: "primary", icon: "↑" })}
        </div>
      </div>
      ${renderExtensionCapabilityStrip([
        { label: "Market catalog", level: market.unavailable ? "unavailable" : "runtime", note: "/api/management/plugin-market" },
        { label: "Install/update/uninstall", level: market.unavailable ? "unavailable" : "in_memory", note: "management operation state" },
        { label: "External download/unpack", level: "plan_only", note: "not probed by frontend" },
        { label: "Custom source", level: "plan_only", note: "source-compatible UI placeholder" },
      ])}
      ${market.unavailable ? uiState({ state: "error", title: "Plugin market unavailable", message: market.unavailable }) : ""}
      ${renderMarketToolbar(plugins.length)}
      ${state.marketShowRandom && randomPlugins.length ? `
        <section class="panel mt-16">
          <div class="panel-title-row">
            <h2>随机推荐</h2>
            <button class="button ghost" type="button" data-action="market-random-toggle">隐藏</button>
          </div>
          <div class="extension-card-grid">${randomPlugins.map(renderMarketPluginCard).join("")}</div>
        </section>
      ` : ""}
      <section class="panel mt-16">
        <div class="panel-title-row"><h2>全部插件 (${plugins.length})</h2><button class="button ghost" type="button" data-action="market-random-toggle">${state.marketShowRandom ? "隐藏推荐" : "显示推荐"}</button></div>
        ${plugins.length ? `<div class="extension-card-grid">${plugins.map(renderMarketPluginCard).join("")}</div>` : uiState({ state: "empty", title: "暂无市场条目", message: "后端未配置 plugin market state，或当前筛选没有结果。" })}
      </section>
      ${market.installed_plugins?.length ? `<section class="panel mt-16"><h2>已安装插件</h2>${jsonBlock(market.installed_plugins)}</section>` : ""}
      ${market.operations?.length ? `<section class="panel mt-16"><h2>插件操作记录</h2>${jsonBlock(market.operations)}</section>` : ""}
      ${renderExtensionDialogs()}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近插件结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

export function renderSkills() {
  const skills = state.skills || { skills: [] };
  const mode = state.skillsMode || "local";
  return `
    <section class="extension-page" data-page="extension-skills">
      ${renderExtensionTabs("skills")}
      <div class="dashboard-banner compact">
        <div>
          <div class="eyebrow">Skills</div>
          <h2>Skills</h2>
          <p>本地 skill catalog、sandbox cache、批量上传计划与 Neo lifecycle 入口。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新", action: mode === "neo" ? "skill-neo-refresh" : "load-skills", variant: "secondary", icon: "↻" })}
        </div>
      </div>
      ${renderExtensionCapabilityStrip([
        { label: "Local skills", level: skills.unavailable ? "unavailable" : "runtime", note: "/api/management/skills" },
        { label: "Batch upload", level: "in_memory", note: "zip entries install plan" },
        { label: "Download zip", level: "unavailable", note: "/api/skills/download not wired" },
        { label: "Neo lifecycle", level: neoUnavailable() ? "unavailable" : "runtime", note: "source-compatible endpoints" },
      ])}
      <div class="provider-type-tabs" role="tablist" aria-label="Skills mode">
        <button type="button" class="${mode === "local" ? "active" : ""}" role="tab" aria-selected="${mode === "local" ? "true" : "false"}" data-action="skills-mode" data-mode="local">Local</button>
        <button type="button" class="${mode === "neo" ? "active" : ""}" role="tab" aria-selected="${mode === "neo" ? "true" : "false"}" data-action="skills-mode" data-mode="neo">Neo</button>
      </div>
      ${skills.unavailable ? uiState({ state: "error", title: "Skills unavailable", message: skills.unavailable }) : ""}
      ${mode === "neo" ? renderNeoSkills() : renderLocalSkills(skills)}
      ${renderExtensionDialogs()}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近 Skills 结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

export function renderTools() {
  const tools = state.tools || { tools: [] };
  const commands = state.commands || { commands: [], conflicts: [] };
  const mcp = state.mcp || { servers: [], active_count: 0 };
  const filteredCommands = filterCommands(commands.commands || []);
  const filteredTools = filterTools(tools.tools || []);
  return `
    <section class="extension-page" data-page="extension-tools">
      ${renderExtensionTabs("tools")}
      <div class="dashboard-banner compact">
        <div>
          <div class="eyebrow">Tools</div>
          <h2>Commands、Tools 与 MCP</h2>
          <p>源端 component panel：Command filter/rename/permission/details、Tool details/toggle、MCP JSON/test/sync-provider。</p>
        </div>
        <div class="banner-actions">
          ${button({ label: "刷新全部", action: "load-tools", variant: "secondary", icon: "↻" })}
        </div>
      </div>
      ${renderExtensionCapabilityStrip([
        { label: "Commands", level: commands.unavailable ? "unavailable" : "runtime", note: "/api/management/commands" },
        { label: "Function tools", level: tools.unavailable ? "unavailable" : "runtime", note: "/api/management/tools" },
        { label: "MCP config", level: mcp.unavailable ? "unavailable" : "runtime", note: "/api/management/mcp/servers" },
        { label: "MCP process probe", level: "plan_only", note: "config validation only" },
      ])}
      ${renderComponentFilters(commands.commands || [])}
      <div class="grid cols-2">
        <section class="panel">
          <div class="panel-title-row"><h2>Commands</h2><span class="tag">${filteredCommands.length} / ${(commands.commands || []).length}</span></div>
          ${commands.unavailable ? uiState({ state: "error", message: commands.unavailable, compact: true }) : ""}
          ${renderCommandConflicts(commands.conflicts || [])}
          ${renderCommandTable(filteredCommands)}
        </section>
        <section class="panel">
          <div class="panel-title-row"><h2>Function Tools</h2><span class="tag">${filteredTools.length} / ${(tools.tools || []).length}</span></div>
          ${tools.unavailable ? uiState({ state: "error", message: tools.unavailable, compact: true }) : ""}
          ${renderToolTable(filteredTools)}
        </section>
      </div>
      ${renderCommandForm()}
      ${renderMcpPanel(mcp)}
      ${renderExtensionDialogs()}
    </section>
    ${state.operation ? `<section class="panel"><h2>最近工具结果</h2>${jsonBlock(state.operation)}</section>` : ""}
  `;
}

function renderExtensionTabs(active) {
  const tabs = [
    { id: "plugins", label: "Installed", route: "plugins" },
    { id: "market", label: "Marketplace", route: "market" },
    { id: "tools", label: "Commands & Tools", route: "tools" },
    { id: "skills", label: "Skills", route: "skills" },
  ];
  return `
    <nav class="provider-type-tabs extension-tabs" aria-label="Extension tabs">
      ${tabs.map((tab) => `<button type="button" class="${tab.id === active ? "active" : ""}" aria-pressed="${tab.id === active ? "true" : "false"}" data-route="${escapeHtml(tab.route)}">${escapeHtml(tab.label)}</button>`).join("")}
    </nav>
  `;
}

function renderExtensionCapabilityStrip(items) {
  return `<div class="capability-strip">${items.map((item) => `<div class="capability-item"><strong>${escapeHtml(item.label)}</strong>${closurePill(item.level)}<small>${escapeHtml(item.note)}</small></div>`).join("")}</div>`;
}

function renderPluginToolbar(allPlugins) {
  const status = state.extensionPluginStatusFilter || "all";
  const view = state.extensionPluginView || "grid";
  const reservedCount = allPlugins.filter((plugin) => plugin.source?.reserved).length;
  return `
    <section class="panel extension-toolbar">
      <div class="form-grid cols-3">
        ${formField({ id: "plugin-search", label: "Search", value: state.extensionPluginSearch, placeholder: "name, id, description" })}
        ${formField({ id: "plugin-status-filter", label: "Status", type: "select", value: status, options: [
          { value: "all", label: "All" },
          { value: "enabled", label: "Enabled" },
          { value: "disabled", label: "Disabled" },
          { value: "failed", label: "Failed" },
          { value: "system", label: `System (${reservedCount})` },
        ] })}
        <div class="ui-form-field">
          <label>View</label>
          <div class="actions compact">
            <button class="button ${view === "grid" ? "" : "ghost"}" type="button" data-action="plugin-view-mode" data-view="grid">Grid</button>
            <button class="button ${view === "list" ? "" : "ghost"}" type="button" data-action="plugin-view-mode" data-view="list">List</button>
          </div>
        </div>
      </div>
      <div class="actions">
        <button class="button" type="button" data-action="extension-filter">应用筛选</button>
        <button class="button ghost" type="button" data-action="plugin-toggle-reserved">${state.extensionPluginShowReserved ? "隐藏系统插件" : "显示系统插件"}</button>
      </div>
    </section>
  `;
}

function filteredInstalledPlugins(plugins) {
  const query = normalizeText(state.extensionPluginSearch);
  const status = state.extensionPluginStatusFilter || "all";
  return plugins
    .filter((plugin) => state.extensionPluginShowReserved || !plugin.source?.reserved)
    .filter((plugin) => {
      if (status === "enabled") return plugin.active;
      if (status === "disabled") return !plugin.active;
      if (status === "failed") return plugin.state === "failed";
      if (status === "system") return plugin.source?.reserved;
      return true;
    })
    .filter((plugin) => !query || normalizeText([
      plugin.name,
      plugin.plugin_id,
      plugin.description,
      plugin.version,
      plugin.source?.kind,
      ...(plugin.capabilities || []),
    ].join(" ")).includes(query))
    .sort((left, right) => pluginDisplayName(left).localeCompare(pluginDisplayName(right)));
}

function renderPluginGrid(plugins) {
  if (!plugins.length) return uiState({ state: "empty", title: "暂无插件", message: "当前筛选没有 installed plugin。" });
  return `<div class="extension-card-grid">${plugins.map(renderInstalledPluginCard).join("")}</div>`;
}

function renderPluginList(plugins) {
  if (!plugins.length) return uiState({ state: "empty", title: "暂无插件", message: "当前筛选没有 installed plugin。" });
  return `
    <section class="panel">
      <table class="table">
        <thead><tr><th>插件</th><th>来源</th><th>能力</th><th>状态</th><th>操作</th></tr></thead>
        <tbody>${plugins.map((plugin) => `
          <tr>
            <td><strong>${escapeHtml(pluginDisplayName(plugin))}</strong><br><span class="metric-label">${escapeHtml(plugin.plugin_id)} ${escapeHtml(plugin.version || "")}</span></td>
            <td>${renderPluginSource(plugin)}</td>
            <td>${renderChipList(plugin.capabilities || [])}</td>
            <td>${pluginStatePill(plugin)}</td>
            <td class="button-cell">${renderPluginActions(plugin)}</td>
          </tr>
        `).join("")}</tbody>
      </table>
    </section>
  `;
}

function renderInstalledPluginCard(plugin) {
  return `
    <article class="extension-card">
      <header>
        <div class="extension-icon" aria-hidden="true">${escapeHtml(pluginDisplayName(plugin).slice(0, 2).toUpperCase())}</div>
        <div>
          <h3>${escapeHtml(pluginDisplayName(plugin))}</h3>
          <p>${escapeHtml(plugin.plugin_id)} · ${escapeHtml(plugin.version || "unknown")}</p>
        </div>
        ${pluginStatePill(plugin)}
      </header>
      <p class="extension-description">${escapeHtml(plugin.description || "No description.")}</p>
      <div class="ui-chip-row">${renderChipList([plugin.source?.kind || "unknown", ...(plugin.capabilities || []).slice(0, 3)])}</div>
      <div class="extension-meta">${renderPluginSource(plugin)}</div>
      <footer class="actions">${renderPluginActions(plugin)}</footer>
    </article>
  `;
}

function renderPluginActions(plugin) {
  const activeAction = plugin.active ? "disable" : "activate";
  return `
    <button class="button secondary" type="button" data-action="plugin-lifecycle" data-lifecycle="${activeAction}" data-plugin="${escapeHtml(plugin.plugin_id)}">${plugin.active ? "停用" : "启用"}</button>
    <button class="button secondary" type="button" data-action="plugin-lifecycle" data-lifecycle="reload" data-plugin="${escapeHtml(plugin.plugin_id)}">Reload</button>
    <button class="button ghost" type="button" data-action="plugin-config-open" data-plugin="${escapeHtml(plugin.plugin_id)}">Config</button>
    <button class="button ghost" type="button" data-action="plugin-doc-open" data-doc="readme" data-plugin="${escapeHtml(plugin.plugin_id)}">README</button>
    <button class="button ghost" type="button" data-action="plugin-doc-open" data-doc="changelog" data-plugin="${escapeHtml(plugin.plugin_id)}">Changelog</button>
    <button class="button ghost" type="button" data-action="plugin-source-open" data-plugin="${escapeHtml(plugin.plugin_id)}">Source</button>
  `;
}

function renderPluginSource(plugin) {
  const source = plugin.source || {};
  return `${pill(source.kind || "unknown", source.reserved ? "warn" : "")}<br><span class="metric-label">${escapeHtml(source.root_dir || source.module_path || "-")}</span>${plugin.config_files?.length ? `<br>${plugin.config_files.map((file) => `<span class="tag">${escapeHtml(file.filename)}</span>`).join(" ")}` : ""}`;
}

function renderFailedPlugins(failed) {
  return `
    <section class="panel warning-panel">
      <div class="panel-title-row"><h2>失败插件 (${failed.length})</h2><span class="tag">reload failed source-compatible workflow</span></div>
      <table class="table">
        <thead><tr><th>插件</th><th>状态</th><th>操作</th></tr></thead>
        <tbody>${failed.map((plugin) => `
          <tr>
            <td><strong>${escapeHtml(pluginDisplayName(plugin))}</strong><br><span class="metric-label">${escapeHtml(plugin.plugin_id)}</span></td>
            <td>${pluginStatePill(plugin)}</td>
            <td class="button-cell">
              <button class="button secondary" type="button" data-action="plugin-lifecycle" data-lifecycle="reload" data-plugin="${escapeHtml(plugin.plugin_id)}">重载失败插件</button>
              <button class="button ghost" type="button" data-action="plugin-lifecycle" data-lifecycle="unload" data-plugin="${escapeHtml(plugin.plugin_id)}">卸载失败记录</button>
            </td>
          </tr>
        `).join("")}</tbody>
      </table>
    </section>
  `;
}

function renderHandlersTable(handlers) {
  if (!handlers.length) return uiState({ state: "empty", title: "当前没有注册 handler", compact: true });
  return `
    <table class="table">
      <thead><tr><th>插件</th><th>Handler</th><th>事件</th><th>优先级</th><th>状态</th></tr></thead>
      <tbody>${handlers.map((handler) => `
        <tr>
          <td>${escapeHtml(handler.plugin_name)}</td>
          <td>${escapeHtml(handler.handler_name)}</td>
          <td>${escapeHtml(handler.event_type)}</td>
          <td>${escapeHtml(handler.priority)}</td>
          <td>${pill(handler.enabled ? "启用" : "停用", handler.enabled ? "ok" : "warn")}</td>
        </tr>
      `).join("")}</tbody>
    </table>
  `;
}

function renderPluginUploadPanel() {
  return `
    <section class="panel">
      <h2>Install Upload</h2>
      <p class="empty">目标后端当前支持 archive shape plan，真实 multipart 解包/热加载仍显示为 plan-only。</p>
      <div class="form-row"><label>ZIP entries</label><textarea id="plugin-upload-entries">weather/main.py
weather/metadata.yaml</textarea></div>
      <label class="check-row"><input id="plugin-upload-overwrite" type="checkbox" checked /> Overwrite</label>
      <div class="actions"><button class="button" type="button" data-action="plugin-upload-plan">生成计划</button></div>
    </section>
  `;
}

function renderPluginSourcePanel() {
  return `
    <section class="panel">
      <h2>Source Plan</h2>
      <div class="form-row"><label>Plugin ID</label><input id="plugin-source-id" value="weather" /></div>
      <div class="form-row"><label>Kind</label><select id="plugin-source-kind"><option value="python_compat">python_compat</option><option value="native_rust">native_rust</option><option value="wasm">wasm</option><option value="external_process">external_process</option></select></div>
      <div class="form-row"><label>Root</label><input id="plugin-source-root" value="plugins/weather" /></div>
      <div class="form-row"><label>Module</label><input id="plugin-source-module" value="main.py" /></div>
      <label class="check-row"><input id="plugin-source-reserved" type="checkbox" /> Reserved</label>
      <div class="actions"><button class="button" type="button" data-action="plugin-source-plan">生成来源计划</button></div>
    </section>
  `;
}

function renderPluginConfigPanel(plugins) {
  return `
    <section class="panel">
      <h2>Config File</h2>
      <div class="form-row"><label>Plugin ID</label><input id="plugin-config-id" value="${escapeHtml(plugins[0]?.plugin_id || "weather")}" /></div>
      <div class="form-row"><label>Filename</label><input id="plugin-config-filename" value="config.json" /></div>
      <div class="form-row"><label>JSON</label><textarea id="plugin-config-json">{ "enabled": true }</textarea></div>
      <div class="actions">
        <button class="button" type="button" data-action="plugin-config-save">保存状态配置</button>
        <button class="button secondary" type="button" data-action="plugin-config-file-read">读取文件</button>
        <button class="button secondary" type="button" data-action="plugin-config-file-write">写入文件</button>
        <button class="button ghost" type="button" data-action="plugin-config-file-delete">删除文件</button>
      </div>
    </section>
  `;
}

function renderMarketToolbar(total) {
  return `
    <section class="panel extension-toolbar">
      <div class="form-grid cols-3">
        ${formField({ id: "market-search", label: "Search market", value: state.marketSearch, placeholder: "name, author, repo" })}
        ${formField({ id: "market-sort-by", label: "Sort by", type: "select", value: state.marketSortBy || "default", options: [
          { value: "default", label: "Default" },
          { value: "name", label: "Name" },
          { value: "version", label: "Version" },
          { value: "status", label: "Installed status" },
        ] })}
        ${formField({ id: "market-sort-order", label: "Order", type: "select", value: state.marketSortOrder || "desc", options: [
          { value: "desc", label: "Descending" },
          { value: "asc", label: "Ascending" },
        ] })}
      </div>
      <div class="actions">
        <button class="button" type="button" data-action="extension-filter">应用筛选</button>
        <span class="tag">${escapeHtml(total)} results</span>
      </div>
      <div class="source-warning">插件源可包含第三方代码；安装/更新前请核对 repo、兼容性与 capability 层级。</div>
    </section>
  `;
}

function filteredMarketPlugins(plugins) {
  const query = normalizeText(state.marketSearch);
  if (!query) return plugins;
  return plugins.filter((plugin) => normalizeText([
    plugin.name,
    plugin.plugin_id,
    plugin.repo_url,
    plugin.version,
    plugin.package?.source?.kind,
  ].join(" ")).includes(query));
}

function sortedMarketPlugins(plugins) {
  const sortBy = state.marketSortBy || "default";
  const order = state.marketSortOrder === "asc" ? 1 : -1;
  return [...plugins].sort((left, right) => {
    if (sortBy === "name") return order * pluginDisplayName(left).localeCompare(pluginDisplayName(right));
    if (sortBy === "version") return order * String(left.version || "").localeCompare(String(right.version || ""));
    if (sortBy === "status") return order * Number(Boolean(left.installed) - Boolean(right.installed));
    if (left.installed !== right.installed) return Number(left.installed) - Number(right.installed);
    return pluginDisplayName(left).localeCompare(pluginDisplayName(right));
  });
}

function renderMarketPluginCard(plugin) {
  const blocked = plugin.compatibility?.compatible === false;
  return `
    <article class="extension-card market-plugin-card">
      <header>
        <div class="extension-icon" aria-hidden="true">${escapeHtml(pluginDisplayName(plugin).slice(0, 2).toUpperCase())}</div>
        <div>
          <h3>${escapeHtml(pluginDisplayName(plugin))}</h3>
          <p>${escapeHtml(plugin.plugin_id)} · ${escapeHtml(plugin.version || "unknown")}</p>
        </div>
        ${pill(plugin.installed ? "已安装" : "未安装", plugin.installed ? "ok" : "warn")}
      </header>
      <p class="extension-description">${escapeHtml(plugin.readme?.summary || plugin.description || plugin.repo_url || "No description.")}</p>
      <div class="ui-chip-row">
        ${pill(plugin.package?.source?.kind || (plugin.repo_url ? "repository" : "none"))}
        ${pill(blocked ? "不兼容" : "兼容", blocked ? "error" : "ok")}
        ${plugin.pending_loader_reload ? pill("待重载", "warn") : ""}
      </div>
      <div class="extension-meta">${escapeHtml(plugin.repo_url || "No repository URL")}</div>
      <footer class="actions">
        <button class="button secondary" type="button" data-action="plugin-execute" data-plan="install" data-plugin="${escapeHtml(plugin.plugin_id)}">安装</button>
        <button class="button secondary" type="button" data-action="plugin-execute" data-plan="update" data-plugin="${escapeHtml(plugin.plugin_id)}" ${plugin.installed ? "" : "disabled"}>更新</button>
        <button class="button ghost" type="button" data-action="plugin-execute" data-plan="uninstall" data-plugin="${escapeHtml(plugin.plugin_id)}" ${plugin.installed ? "" : "disabled"}>卸载</button>
        <button class="button ghost" type="button" data-action="plugin-plan" data-plan="install" data-plugin="${escapeHtml(plugin.plugin_id)}">计划</button>
        <button class="button ghost" type="button" data-action="plugin-doc-open" data-doc="readme" data-plugin="${escapeHtml(plugin.plugin_id)}">README</button>
      </footer>
    </article>
  `;
}

function renderLocalSkills(skills) {
  return `
    <div class="grid cols-2">
      <section class="panel">
        <div class="panel-title-row"><h2>技能目录</h2><button class="button ghost" type="button" data-action="load-skills">刷新</button></div>
        ${(skills.skills || []).length ? `<div class="extension-card-grid">${skills.skills.map(renderSkillCard).join("")}</div>` : uiState({ state: "empty", title: "暂无技能", message: "后端未配置 skill state，或目录为空。" })}
      </section>
      <section class="panel">
        <h2>Batch Upload Plan</h2>
        <p class="empty">源端支持 ZIP batch upload；目标当前用 entries 生成 install plan 并可写入 in-memory catalog。</p>
        <div class="form-row"><label>ZIP entries</label><textarea id="skill-entries">writer/SKILL.md
writer/assets/template.txt</textarea></div>
        <div class="actions">
          <button class="button" type="button" data-action="skill-install-plan">生成安装计划</button>
          <button class="button secondary" type="button" data-action="skill-install">执行安装</button>
        </div>
        <div class="mt-16">${skills.sandbox_cache ? jsonBlock(skills.sandbox_cache) : uiState({ state: "empty", title: "Sandbox cache 未配置", compact: true })}</div>
      </section>
    </div>
  `;
}

function renderSkillCard(skill) {
  const readonly = !skill.local_exists;
  return `
    <article class="extension-card skill-card">
      <header>
        <div class="extension-icon" aria-hidden="true">SK</div>
        <div>
          <h3>${escapeHtml(skill.name)}</h3>
          <p>${escapeHtml(skill.path || "")}</p>
        </div>
        ${pill(skill.active ? "启用" : "停用", skill.active ? "ok" : "warn")}
      </header>
      <p class="extension-description">${escapeHtml(skill.description || "No description.")}</p>
      <div class="ui-chip-row">
        ${pill(skill.source_label || skill.source_type, skill.sandbox_exists && !skill.local_exists ? "warn" : "")}
        ${skill.local_exists ? pill("local", "ok") : pill("sandbox/read-only", "warn")}
      </div>
      <footer class="actions">
        <button class="button secondary" type="button" data-action="skill-active" data-skill="${escapeHtml(skill.name)}" data-active="${skill.active ? "false" : "true"}" ${readonly ? "disabled" : ""}>${skill.active ? "停用" : "启用"}</button>
        <button class="button ghost" type="button" data-action="skill-download" data-skill="${escapeHtml(skill.name)}" ${readonly ? "disabled" : ""}>Download</button>
        <button class="button ghost" type="button" data-action="skill-delete-plan" data-skill="${escapeHtml(skill.name)}" ${readonly ? "disabled" : ""}>删除计划</button>
        <button class="button ghost" type="button" data-action="skill-delete" data-skill="${escapeHtml(skill.name)}" ${readonly ? "disabled" : ""}>删除</button>
      </footer>
    </article>
  `;
}

function renderNeoSkills() {
  const neo = state.skillsNeo || {};
  const candidates = normalizeLegacyList(neo.candidates, "candidates");
  const releases = normalizeLegacyList(neo.releases, "releases");
  const unavailable = neoUnavailable();
  return `
    <section class="panel">
      <div class="panel-title-row"><h2>Neo Skills</h2><button class="button ghost" type="button" data-action="skill-neo-refresh">刷新 Neo</button></div>
      ${unavailable ? uiState({ state: "error", title: "Neo Skills endpoint unavailable", message: unavailable, compact: true }) : ""}
      <div class="grid cols-2">
        <div>
          <h3>Candidates (${candidates.length})</h3>
          ${renderNeoTable(candidates, "candidate")}
        </div>
        <div>
          <h3>Releases (${releases.length})</h3>
          ${renderNeoTable(releases, "release")}
        </div>
      </div>
    </section>
  `;
}

function renderNeoTable(items, kind) {
  if (!items.length) return uiState({ state: "empty", title: `No ${kind}s`, compact: true });
  return `
    <table class="table">
      <thead><tr><th>ID</th><th>Skill</th><th>Status</th><th>操作</th></tr></thead>
      <tbody>${items.map((item) => {
        const id = item.id || item.candidate_id || item.release_id || "";
        const payloadRef = item.payload_ref || "";
        return `
          <tr>
            <td>${escapeHtml(id)}</td>
            <td>${escapeHtml(item.skill_key || item.name || "-")}</td>
            <td>${pill(item.status || item.stage || (item.is_active ? "active" : "inactive"), item.is_active ? "ok" : "")}</td>
            <td class="button-cell">
              ${kind === "candidate" ? `
                <button class="button secondary" type="button" data-action="skill-neo-action" data-endpoint="evaluate" data-payload="${escapeHtml(JSON.stringify({ candidate_id: id, passed: true, score: 1, report: "approved_from_dashboard" }))}">Pass</button>
                <button class="button ghost" type="button" data-action="skill-neo-action" data-endpoint="promote" data-payload="${escapeHtml(JSON.stringify({ candidate_id: id, stage: "stable", sync_to_local: true }))}">Promote</button>
              ` : `
                <button class="button secondary" type="button" data-action="skill-neo-action" data-endpoint="sync" data-payload="${escapeHtml(JSON.stringify({ release_id: id }))}">Sync</button>
                <button class="button ghost" type="button" data-action="skill-neo-action" data-endpoint="rollback" data-payload="${escapeHtml(JSON.stringify({ release_id: id }))}">Rollback</button>
              `}
              <button class="button ghost" type="button" data-action="skill-payload-open" data-payload-ref="${escapeHtml(payloadRef)}" ${payloadRef ? "" : "disabled"}>Payload</button>
            </td>
          </tr>
        `;
      }).join("")}</tbody>
    </table>
  `;
}

function renderComponentFilters(commands) {
  const plugins = Array.from(new Set(commands.map((command) => command.plugin_name).filter(Boolean))).sort();
  return `
    <section class="panel extension-toolbar">
      <div class="form-grid cols-3">
        ${formField({ id: "command-search", label: "Command search", value: state.commandSearch, placeholder: "command, plugin, description" })}
        ${formField({ id: "command-plugin-filter", label: "Plugin", type: "select", value: state.commandPluginFilter || "all", options: [{ value: "all", label: "All plugins" }, ...plugins.map((plugin) => ({ value: plugin, label: plugin }))] })}
        ${formField({ id: "command-permission-filter", label: "Permission", type: "select", value: state.commandPermissionFilter || "all", options: [{ value: "all", label: "All" }, { value: "admin", label: "Admin" }, { value: "member", label: "Member" }, { value: "everyone", label: "Everyone" }] })}
        ${formField({ id: "command-status-filter", label: "Status", type: "select", value: state.commandStatusFilter || "all", options: [{ value: "all", label: "All" }, { value: "enabled", label: "Enabled" }, { value: "disabled", label: "Disabled" }] })}
        ${formField({ id: "tool-search", label: "Tool search", value: state.toolSearch, placeholder: "tool name or description" })}
      </div>
      <div class="actions"><button class="button" type="button" data-action="extension-filter">应用筛选</button></div>
    </section>
  `;
}

function filterCommands(commands) {
  const query = normalizeText(state.commandSearch);
  return commands.filter((command) => {
    if (state.commandPluginFilter !== "all" && state.commandPluginFilter && command.plugin_name !== state.commandPluginFilter) return false;
    if (state.commandPermissionFilter !== "all" && state.commandPermissionFilter && String(command.permission) !== state.commandPermissionFilter) return false;
    if (state.commandStatusFilter === "enabled" && !command.enabled) return false;
    if (state.commandStatusFilter === "disabled" && command.enabled) return false;
    if (!query) return true;
    return normalizeText([
      command.effective_command,
      command.current_fragment,
      command.plugin_name,
      command.handler_name,
      command.description,
      command.response,
      ...(command.aliases || []),
    ].join(" ")).includes(query);
  });
}

function renderCommandConflicts(conflicts) {
  if (!conflicts.length) return "";
  return `<div class="ui-state error compact"><div class="ui-state-icon">!</div><div class="ui-state-copy"><strong>Command conflicts</strong><span>${conflicts.map((conflict) => `${escapeHtml(conflict.command || conflict.effective_command || "-")} (${(conflict.handlers || []).map(escapeHtml).join(", ")})`).join("; ")}</span></div></div>`;
}

function renderCommandTable(commands) {
  if (!commands.length) return uiState({ state: "empty", title: "暂无 command plugin 配置", compact: true });
  return `
    <table class="table command-table">
      <thead><tr><th>Command</th><th>Type</th><th>Plugin</th><th>Permission</th><th>Status</th><th>操作</th></tr></thead>
      <tbody>${commands.map((command) => `
        <tr>
          <td><code>${escapeHtml(command.effective_command)}</code><br><span class="metric-label">${escapeHtml(command.description || command.response || "")}</span></td>
          <td>${pill(command.command_type || "command")}</td>
          <td>${escapeHtml(command.plugin_name)}.${escapeHtml(command.handler_name)}<br><span class="metric-label">priority ${escapeHtml(command.priority)}</span></td>
          <td>${pill(command.permission, command.permission === "admin" ? "warn" : "ok")}</td>
          <td>${pill(command.enabled ? "启用" : "停用", command.enabled ? "ok" : "warn")}</td>
          <td class="button-cell">
            <button class="button secondary" type="button" data-action="command-toggle" data-plugin="${escapeHtml(command.plugin_name)}" data-handler="${escapeHtml(command.handler_name)}" data-enabled="${command.enabled ? "false" : "true"}">${command.enabled ? "停用" : "启用"}</button>
            <button class="button ghost" type="button" data-action="command-rename-open" data-command="${escapeHtml(command.handler_full_name)}">Rename</button>
            <button class="button ghost" type="button" data-action="command-permission" data-plugin="${escapeHtml(command.plugin_name)}" data-handler="${escapeHtml(command.handler_name)}" data-permission="${command.permission === "admin" ? "member" : "admin"}">${command.permission === "admin" ? "Everyone" : "Admin"}</button>
            <button class="button ghost" type="button" data-action="command-details-open" data-command="${escapeHtml(command.handler_full_name)}">Details</button>
          </td>
        </tr>
      `).join("")}</tbody>
    </table>
  `;
}

function filterTools(tools) {
  const query = normalizeText(state.toolSearch);
  if (!query) return tools;
  return tools.filter((tool) => normalizeText([tool.name, tool.description, tool.origin, tool.origin_name, tool.source].join(" ")).includes(query));
}

function renderToolTable(tools) {
  if (!tools.length) return uiState({ state: "empty", title: "暂无工具", compact: true });
  return `
    <table class="table tool-table">
      <thead><tr><th>名称</th><th>来源</th><th>状态</th><th>操作</th></tr></thead>
      <tbody>${tools.map((tool) => `
        <tr>
          <td>${escapeHtml(tool.name)}<br><span class="metric-label">${escapeHtml(tool.description || "")}</span></td>
          <td>${escapeHtml(tool.origin_name || "-")}<br><span class="tag">${escapeHtml(tool.origin || tool.source || "-")}</span></td>
          <td>${pill(tool.active ? "启用" : "停用", tool.active ? "ok" : "warn")}</td>
          <td class="button-cell">
            <button class="button secondary" type="button" data-action="toggle-tool" data-tool="${escapeHtml(tool.name)}" data-active="${tool.active ? "false" : "true"}" ${tool.user_toggle_allowed ? "" : "disabled"}>${tool.active ? "停用" : "启用"}</button>
            <button class="button ghost" type="button" data-action="tool-details-open" data-tool="${escapeHtml(tool.name)}">Details</button>
          </td>
        </tr>
      `).join("")}</tbody>
    </table>
  `;
}

function renderCommandForm() {
  return `
    <section class="panel mt-16">
      <h2>Command Form</h2>
      <div class="grid cols-2">
        <div class="form-row"><label>Plugin</label><input id="command-plugin" value="dashboard" /></div>
        <div class="form-row"><label>Handler</label><input id="command-handler" value="ping" /></div>
        <div class="form-row"><label>Command</label><input id="command-command" value="ping" /></div>
        <div class="form-row"><label>Response</label><input id="command-response" value="pong" /></div>
        <div class="form-row"><label>Priority</label><input id="command-priority" type="number" value="0" /></div>
        <div class="form-row"><label>Permission</label><select id="command-permission"><option value="everyone">everyone</option><option value="member">member</option><option value="admin">admin</option></select></div>
      </div>
      <label class="check-row"><input id="command-enabled" type="checkbox" checked /> Enabled</label>
      <div class="actions"><button class="button" type="button" data-action="command-update">保存 Command</button></div>
    </section>
  `;
}

function renderMcpPanel(mcp) {
  const draft = state.mcpJsonDraft || JSON.stringify({ active: true, transport: "stdio", command: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem"], sessionReadTimeoutSeconds: 60 }, null, 2);
  return `
    <section class="panel mt-16">
      <div class="panel-title-row">
        <h2>MCP Servers</h2>
        <div class="actions compact">
          <button class="button ghost" type="button" data-action="load-mcp">刷新</button>
          <button class="button secondary" type="button" data-action="mcp-json-template" data-template="stdio">stdio template</button>
          <button class="button secondary" type="button" data-action="mcp-json-template" data-template="streamable_http">HTTP template</button>
        </div>
      </div>
      ${mcp.unavailable ? uiState({ state: "error", message: mcp.unavailable, compact: true }) : ""}
      ${(mcp.servers || []).length ? `
        <table class="table">
          <thead><tr><th>名称</th><th>Transport</th><th>状态</th><th>操作</th></tr></thead>
          <tbody>${mcp.servers.map((server) => `
            <tr>
              <td>${escapeHtml(server.name)}<br><span class="metric-label">${escapeHtml(server.command || server.url || "-")}</span></td>
              <td>${escapeHtml(server.transport)}</td>
              <td>${pill(server.active ? "启用" : "停用", server.active ? "ok" : "warn")} ${pill(server.valid ? "valid" : "invalid", server.valid ? "ok" : "error")}</td>
              <td class="button-cell">
                <button class="button secondary" type="button" data-action="mcp-check" data-mcp="${escapeHtml(server.name)}">检查</button>
                <button class="button secondary" type="button" data-action="mcp-sync" data-mcp="${escapeHtml(server.name)}">Sync</button>
                <button class="button ghost" type="button" data-action="mcp-edit-json" data-mcp="${escapeHtml(server.name)}">Edit JSON</button>
                <button class="button ghost" type="button" data-action="mcp-delete" data-mcp="${escapeHtml(server.name)}">删除</button>
              </td>
            </tr>
          `).join("")}</tbody>
        </table>
      ` : uiState({ state: "empty", title: "暂无 MCP server 配置", compact: true })}
      <div class="grid cols-2 mt-16">
        <section>
          <h3>JSON Dialog</h3>
          <div class="form-row"><label>Name</label><input id="mcp-json-name" value="${escapeHtml(state.mcpEditName || "docs")}" /></div>
          <div class="form-row"><label>Server JSON</label><textarea id="mcp-json" rows="10">${escapeHtml(draft)}</textarea></div>
          <div class="actions">
            <button class="button" type="button" data-action="mcp-json-upsert">保存 JSON MCP</button>
            <button class="button secondary" type="button" data-action="mcp-check-form">检查表单配置</button>
          </div>
        </section>
        <section>
          <h3>Source-style Form & Sync Provider</h3>
          <div class="form-row"><label>Name</label><input id="mcp-name" value="docs" /></div>
          <div class="form-row"><label>Transport</label><select id="mcp-transport"><option value="stdio">stdio</option><option value="sse">sse</option><option value="streamable_http">streamable_http</option></select></div>
          <div class="form-row"><label>Command</label><input id="mcp-command" value="npx" /></div>
          <div class="form-row"><label>URL</label><input id="mcp-url" placeholder="https://example.invalid/sse" /></div>
          <div class="form-row"><label>Args</label><textarea id="mcp-args">-y
@modelcontextprotocol/server-filesystem</textarea></div>
          <div class="form-row"><label>Timeout</label><input id="mcp-timeout" type="number" min="1" value="60" /></div>
          <label class="check-row"><input id="mcp-active" type="checkbox" checked /> Active</label>
          <label class="check-row"><input id="mcp-elicitation" type="checkbox" /> Elicitation</label>
          <label class="check-row"><input id="mcp-sampling" type="checkbox" /> Sampling</label>
          <div class="actions"><button class="button" type="button" data-action="mcp-upsert">保存 MCP</button><button class="button secondary" type="button" data-action="mcp-check-form">检查配置</button></div>
          <div class="form-row mt-16"><label>Sync provider</label><select id="mcp-sync-provider"><option value="modelscope">modelscope</option></select></div>
          <p class="empty">Provider token sync 当前在目标后端降级为配置 sync plan。</p>
          <button class="button secondary" type="button" data-action="mcp-sync-provider">Sync Provider</button>
        </section>
      </div>
    </section>
  `;
}

function renderExtensionDialogs() {
  const dialogs = [];
  if (state.extensionDialog === "plugin-doc" && state.extensionDoc) {
    dialogs.push(dialog({
      id: "plugin-doc-dialog",
      title: state.extensionDoc.title,
      open: true,
      maxWidth: "860px",
      body: `
        <div class="ui-chip-row">${closurePill(state.extensionDoc.capability || "runtime")}${state.extensionDoc.repo_url ? chip(state.extensionDoc.repo_url) : ""}</div>
        ${markdownViewer({ markdown: state.extensionDoc.markdown, emptyMessage: "No plugin document." })}
      `,
      actions: [{ label: "关闭", variant: "ghost", action: "plugin-doc-close" }],
    }));
  }
  if (state.extensionDialog === "plugin-config" && state.extensionDoc) {
    dialogs.push(dialog({
      id: "plugin-config-dialog",
      title: state.extensionDoc.title,
      open: true,
      maxWidth: "720px",
      body: `
        <div class="form-row"><label>Plugin ID</label><input id="plugin-config-id" value="${escapeHtml(state.extensionDoc.plugin_id)}" /></div>
        <div class="form-row"><label>Filename</label><input id="plugin-config-filename" value="${escapeHtml(state.extensionDoc.filename || "config.json")}" /></div>
        <div class="form-row"><label>JSON</label><textarea id="plugin-config-json" rows="10">${escapeHtml(JSON.stringify(state.extensionDoc.config || {}, null, 2))}</textarea></div>
      `,
      actions: [
        { label: "读取文件", variant: "secondary", action: "plugin-config-file-read" },
        { label: "写入文件", variant: "primary", action: "plugin-config-file-write" },
        { label: "关闭", variant: "ghost", action: "extension-dialog-close" },
      ],
    }));
  }
  if (state.extensionDialog === "plugin-source" && state.extensionDoc) {
    dialogs.push(dialog({
      id: "plugin-source-dialog",
      title: state.extensionDoc.title,
      open: true,
      maxWidth: "680px",
      body: `${renderChipList([...(state.extensionDoc.capabilities || []), ...(state.extensionDoc.permissions || [])])}${jsonBlock(state.extensionDoc.source || {})}`,
      actions: [{ label: "关闭", variant: "ghost", action: "extension-dialog-close" }],
    }));
  }
  if (state.commandDetailsId) {
    const command = (state.commands?.commands || []).find((item) => item.handler_full_name === state.commandDetailsId);
    dialogs.push(dialog({
      id: "command-details-dialog",
      title: "Command details",
      open: true,
      maxWidth: "720px",
      body: command ? jsonBlock(command) : uiState({ state: "empty", title: "Command not found", compact: true }),
      actions: [{ label: "关闭", variant: "ghost", action: "extension-dialog-close" }],
    }));
  }
  if (state.commandRenameId) {
    const command = (state.commands?.commands || []).find((item) => item.handler_full_name === state.commandRenameId);
    dialogs.push(dialog({
      id: "command-rename-dialog",
      title: "Rename command",
      open: true,
      maxWidth: "520px",
      body: command ? `
        <p class="empty">${escapeHtml(command.plugin_name)}.${escapeHtml(command.handler_name)}</p>
        <div class="form-row"><label>Command</label><input id="command-rename-command" value="${escapeHtml(command.current_fragment || command.effective_command || "")}" /></div>
        <div class="form-row"><label>Permission</label><select id="command-rename-permission"><option value="everyone" ${command.permission === "everyone" ? "selected" : ""}>everyone</option><option value="member" ${command.permission === "member" ? "selected" : ""}>member</option><option value="admin" ${command.permission === "admin" ? "selected" : ""}>admin</option></select></div>
        <label class="check-row"><input id="command-rename-enabled" type="checkbox" ${command.enabled ? "checked" : ""} /> Enabled</label>
      ` : uiState({ state: "empty", title: "Command not found", compact: true }),
      actions: [{ label: "保存", action: "command-rename-save" }, { label: "关闭", variant: "ghost", action: "extension-dialog-close" }],
    }));
  }
  if (state.toolDetailsName) {
    const tool = (state.tools?.tools || []).find((item) => item.name === state.toolDetailsName);
    dialogs.push(dialog({
      id: "tool-details-dialog",
      title: "Tool details",
      open: true,
      maxWidth: "720px",
      body: tool ? jsonBlock(tool) : uiState({ state: "empty", title: "Tool not found", compact: true }),
      actions: [{ label: "关闭", variant: "ghost", action: "extension-dialog-close" }],
    }));
  }
  if (state.extensionDialog === "skill-payload") {
    dialogs.push(dialog({
      id: "skill-payload-dialog",
      title: "Skill payload",
      open: true,
      maxWidth: "820px",
      body: jsonBlock(state.skillsPayload || {}),
      actions: [{ label: "关闭", variant: "ghost", action: "extension-dialog-close" }],
    }));
  }
  return dialogs.join("");
}

function pluginDisplayName(plugin = {}) {
  return plugin.display_name || plugin.name || plugin.plugin_id || "Plugin";
}

function pluginStatePill(plugin) {
  if (plugin.state === "failed") return pill("failed", "error");
  return pill(plugin.active ? "启用" : "停用", plugin.active ? "ok" : "warn");
}

function renderChipList(items) {
  return (items || []).filter(Boolean).map((item) => `<span class="tag">${escapeHtml(item)}</span>`).join(" ");
}

function normalizeText(value) {
  return String(value || "").trim().toLowerCase();
}

function normalizeLegacyList(payload, fallbackKey) {
  if (Array.isArray(payload)) return payload;
  if (Array.isArray(payload?.data)) return payload.data;
  if (Array.isArray(payload?.[fallbackKey])) return payload[fallbackKey];
  return [];
}

function neoUnavailable() {
  const candidates = state.skillsNeo?.candidates;
  const releases = state.skillsNeo?.releases;
  return candidates?.unavailable || releases?.unavailable || "";
}
