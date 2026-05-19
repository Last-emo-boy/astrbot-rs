import { apiBase } from "../api.js";
import { escapeHtml } from "../dom.js";
import { state } from "../state.js";
import { closurePill, markdownViewer, metric, pill, statusItem, uiState } from "./shared.js";

export function renderOverview() {
  if (!state.status) {
    return `<div class="panel">${uiState({ state: "loading", title: "正在读取 Runtime", message: "正在连接 AstrBot management API。" })}</div>`;
  }

  const sourcePath = normalizePath(state.routeSourcePath || state.routePath || "/welcome");
  if (sourcePath === "/dashboard/default") {
    return renderDefaultDashboard();
  }
  return renderWelcome();
}

function renderWelcome() {
  const status = state.status;
  const stats = state.stats || {};
  const providers = status.providers || {};
  const platforms = status.platforms || {};
  const backendDone = Boolean(status);
  const platformDone = Number(platforms.platform_count || 0) > 0;
  const providerDone = Number(providers.chat_provider_count || 0) > 0;
  const announcement = welcomeAnnouncement();

  return `
    <div class="welcome-page" data-page="welcome">
      <section class="dashboard-banner welcome-hero">
        <div>
          <div class="eyebrow">AstrBot</div>
          <h2>${escapeHtml(greetingText())}</h2>
          <p>完成后端连接、平台接入和 Provider 配置后，Dashboard 会进入完整运行视图。</p>
        </div>
        <div class="banner-actions">
          ${pill(backendDone ? "Backend connected" : "Backend pending", backendDone ? "ok" : "warn")}
          ${pill(`${providers.chat_provider_count || 0} providers`, providerDone ? "ok" : "warn")}
          ${pill(`${platforms.platform_count || 0} platforms`, platformDone ? "ok" : "warn")}
        </div>
      </section>

      <section class="panel welcome-card">
        <h2>Getting Started</h2>
        <div class="timeline">
          ${welcomeStep(1, "配置后端地址", "保存 AstrBot 后端 API Base 并验证 /api/management/status 可访问。", backendDone, `
            <div class="form-row compact">
              <label for="welcome-api-base">Backend URL</label>
              <input id="welcome-api-base" value="${escapeHtml(apiBase() || "same-origin")}" readonly />
            </div>
          `)}
          ${welcomeStep(2, "接入平台", "创建或启用一个 Platform，让 Runtime 可以接收消息。", platformDone, `
            <button class="button secondary" type="button" data-route="platforms" ${backendDone ? "" : "disabled"}>配置 Platform</button>
          `)}
          ${welcomeStep(3, "配置 Provider", "添加 Chat Provider 并选择默认模型，供会话和插件调用。", providerDone, `
            <button class="button secondary" type="button" data-route="providers">配置 Provider</button>
          `)}
        </div>
      </section>

      <section class="panel">
        <h2>Resources</h2>
        <div class="resource-grid">
          ${resourceCard("GitHub", "AstrBot 项目仓库与 issue 跟踪。", "https://github.com/AstrBotDevs/AstrBot/")}
          ${resourceCard("Docs", "部署、配置和插件开发文档。", "https://docs.astrbot.app")}
          ${resourceCard("Afdian", "支持 AstrBot 团队持续维护。", "https://afdian.com/a/astrbot_team")}
        </div>
      </section>

      <section class="panel">
        <h2>Announcement</h2>
        ${markdownViewer({ markdown: announcement, emptyMessage: "暂无公告。" })}
      </section>

      ${renderCapabilityOverview(status, stats)}
    </div>
  `;
}

function renderDefaultDashboard() {
  const status = state.status;
  const stats = normalizeDashboardStats(status, state.stats || {}, state.dashboardStats || {});
  return `
    <div class="default-dashboard" data-page="default-dashboard">
      ${state.dashboardNotice ? `
        <section class="notice-banner ${escapeHtml(state.dashboardNotice.type || "info")}">
          <strong>${escapeHtml(state.dashboardNotice.title || "Notice")}</strong>
          <span>${escapeHtml(state.dashboardNotice.content || "")}</span>
        </section>
      ` : ""}
      <div class="grid cols-4 dashboard-stat-grid">
        ${sourceStatCard("message-card", "✉", "Total Messages", formatNumber(stats.messageCount), stats.dailyIncrease ? `+${formatNumber(stats.dailyIncrease)} today` : "Message total")}
        ${sourceStatCard("platform-card", "▤", "Online Platforms", stats.platformCount, "Connected platform count")}
        ${sourceStatCard("uptime-card", "◷", "Running Time", formatDuration(stats.uptimeSeconds), "Runtime uptime")}
        ${sourceStatCard("memory-card", "▣", "Memory Usage", `${formatNumber(stats.memory.process)} MiB`, `${formatNumber(stats.memory.system)} MiB total · CPU ${stats.cpuPercent}%`)}
      </div>
      <div class="grid cols-2">
        <section class="panel chart-card">
          <div class="chart-header">
            <div>
              <h2>Message Trend</h2>
              <p>最近消息时间序列和增长率。</p>
            </div>
            <select class="time-select" aria-label="Time range">
              <option>1 day</option>
              <option>3 days</option>
              <option>1 week</option>
              <option>1 month</option>
            </select>
          </div>
          ${messageTrend(stats.messageSeries)}
        </section>
        <section class="panel platform-stat-card">
          <h2>Platform Stat</h2>
          ${platformRanking(stats.platforms, stats.messageCount)}
        </section>
      </div>
      <div class="dashboard-footer">
        ${pill(`Last update: ${escapeHtml(stats.lastUpdated)}`, "ok")}
        <button class="button ghost icon" type="button" data-action="refresh-core" aria-label="Refresh">↻</button>
      </div>
    </div>
  `;
}

function renderCapabilityOverview(status, stats) {
  const services = state.capabilities?.services || [];
  const runtimeCount = services.filter((service) => service.configured && service.closure_level === "runtime").length;
  const inMemoryCount = services.filter((service) => service.configured && service.closure_level === "in_memory").length;
  const planCount = services.filter((service) => service.configured && service.closure_level === "plan_only").length;
  const unavailableCount = services.filter((service) => !service.configured).length;
  return `
    <section class="dashboard-banner">
      <div>
        <div class="eyebrow">AstrBot RS Dashboard</div>
        <h2>业务闭环看板</h2>
        <p>统一服务静态前端、WebChat、Management API 与安全 plan/in-memory 后台能力。</p>
      </div>
      <div class="banner-actions">
        <span class="status-pill ok">${runtimeCount} runtime</span>
        <span class="status-pill">${inMemoryCount} in-memory</span>
        <span class="status-pill warn">${planCount} plan-only</span>
        <span class="status-pill ${unavailableCount ? "error" : "ok"}">${unavailableCount} unavailable</span>
      </div>
    </section>
    <div class="grid cols-4">
      ${metric("Chat Provider", status.providers.chat_provider_count, status.providers.default_chat_provider_id || "未选择")}
      ${metric("平台", status.platforms.platform_count, status.platforms.platform_ids.join(", ") || "无")}
      ${metric("WebChat", status.platforms.webchat_platform_count, "前端 Chat 入口")}
      ${metric("Uptime", formatDuration(stats.uptime_seconds || 0), `${stats.log_count ?? 0} logs / ${stats.trace_count ?? 0} traces`)}
    </div>
    <div class="grid cols-2">
      <section class="panel">
        <div class="panel-title-row">
          <h2>服务闭环</h2>
          <button class="button ghost" type="button" data-action="refresh-core">刷新</button>
        </div>
        <table class="table">
          <thead><tr><th>服务</th><th>状态</th><th>层级</th><th>API</th></tr></thead>
          <tbody>
            ${services.map((service) => `
              <tr>
                <td>
                  <strong>${escapeHtml(service.label)}</strong>
                  ${service.notes?.length ? `<br><span class="metric-label">${escapeHtml(service.notes[0])}</span>` : ""}
                </td>
                <td>${pill(service.configured ? "已接入" : "未配置", service.configured ? "ok" : "error")}</td>
                <td>${closurePill(service.closure_level)}</td>
                <td><code>${escapeHtml(service.api_base)}</code></td>
              </tr>
            `).join("")}
          </tbody>
        </table>
      </section>
      <section class="panel">
        <h2>当前状态</h2>
        <div class="status-list">
          ${statusItem("默认 Chat Provider", status.providers.default_chat_provider_id || "未配置")}
          ${statusItem("平台 ID", status.platforms.platform_ids.join(", ") || "无")}
          ${statusItem("Provider 族", `${status.providers.chat_provider_count} chat / ${status.providers.embedding_provider_count} embedding / ${status.providers.rerank_provider_count} rerank`)}
          ${statusItem("插件 Handler", `${status.plugins.handler_count} 个`)}
        </div>
      </section>
    </div>
    <div class="grid cols-2">
      <section class="panel">
        <h2>Platform Stats</h2>
        ${stats.platform_counts?.length ? `${barChart(stats.platform_counts.map((item) => ({
          label: `${item.platform_id} · ${item.platform_type}`,
          value: item.count,
        })))}<table class="table"><thead><tr><th>Platform</th><th>Type</th><th>Count</th></tr></thead><tbody>${stats.platform_counts.map((item) => `<tr><td>${escapeHtml(item.platform_id)}</td><td>${escapeHtml(item.platform_type)}</td><td>${item.count}</td></tr>`).join("")}</tbody></table>` : `<p class="empty">暂无 platform metric event。</p>`}
      </section>
      <section class="panel">
        <h2>Provider Usage</h2>
        ${stats.provider_usage?.length ? `${barChart(stats.provider_usage.map((item) => ({
          label: item.provider_id,
          value: item.total_tokens || item.calls,
        })))}<table class="table"><thead><tr><th>Provider</th><th>Calls</th><th>Tokens</th></tr></thead><tbody>${stats.provider_usage.map((item) => `<tr><td>${escapeHtml(item.provider_id)}</td><td>${item.calls}</td><td>${item.total_tokens}</td></tr>`).join("")}</tbody></table>` : `<p class="empty">暂无 provider usage metric。</p>`}
      </section>
    </div>
  `;
}

function welcomeStep(index, title, description, done, actionHtml) {
  return `
    <article class="timeline-step ${done ? "completed" : "pending"}">
      <div class="timeline-dot">${done ? "✓" : index}</div>
      <div class="timeline-body">
        <h3>${escapeHtml(title)}</h3>
        <p>${escapeHtml(description)}</p>
        <div class="timeline-action">${actionHtml}</div>
      </div>
    </article>
  `;
}

function resourceCard(title, description, href) {
  return `
    <a class="resource-card" href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">
      <strong>${escapeHtml(title)}</strong>
      <span>${escapeHtml(description)}</span>
    </a>
  `;
}

function sourceStatCard(kind, icon, title, value, subtitle) {
  return `
    <section class="source-stat-card ${kind}">
      <div class="source-stat-icon">${escapeHtml(icon)}</div>
      <div class="source-stat-content">
        <div class="source-stat-title">${escapeHtml(title)}</div>
        <div class="source-stat-value">${escapeHtml(value)}</div>
        <div class="source-stat-subtitle">${escapeHtml(subtitle)}</div>
      </div>
    </section>
  `;
}

function messageTrend(series) {
  const values = series.length ? series : [[Date.now(), 0]];
  const total = values.reduce((sum, item) => sum + (Number(item[1]) || 0), 0);
  const dailyAverage = Math.round(total / Math.max(1, values.length));
  const growth = growthRate(values);
  return `
    <div class="chart-stats">
      ${chartStat("Total", formatNumber(total))}
      ${chartStat("Average", formatNumber(dailyAverage))}
      ${chartStat("Growth", `${Math.abs(growth)}%`, growth > 0 ? "trend-up" : growth < 0 ? "trend-down" : "")}
    </div>
    <div class="chart-bars" aria-label="Message trend bars">
      ${barChart(values.map((item) => ({ label: formatSeriesLabel(item[0]), value: item[1] })))}
    </div>
  `;
}

function chartStat(label, value, kind = "") {
  return `<div class="stat-box ${escapeHtml(kind)}"><div class="stat-label">${escapeHtml(label)}</div><div class="stat-number">${escapeHtml(value)}</div></div>`;
}

function platformRanking(platforms, totalMessages) {
  if (!platforms.length) {
    return uiState({ state: "empty", title: "No platform data", compact: true });
  }
  const sorted = [...platforms].sort((a, b) => b.count - a.count);
  const total = sorted.reduce((sum, item) => sum + Number(item.count || 0), 0) || totalMessages || 1;
  const top = sorted[0];
  return `
    <div class="platform-list">
      ${sorted.map((platform, index) => `
        <div class="platform-item">
          <span class="platform-rank ${index < 3 ? "top-rank" : ""}">${index + 1}</span>
          <strong>${escapeHtml(platform.name || platform.platform_id || "-")}</strong>
          <span>${formatNumber(platform.count || 0)} messages</span>
        </div>
      `).join("")}
    </div>
    <div class="platform-stats-summary">
      ${statusItem("Platform count", sorted.length)}
      ${statusItem("Most active", top.name || top.platform_id || "-")}
      ${statusItem("Top share", `${Math.round(((top.count || 0) / total) * 100)}%`)}
    </div>
    ${barChart(sorted.slice(0, 5).map((item) => ({ label: item.name || item.platform_id || "-", value: item.count || 0 })))}
  `;
}

function normalizeDashboardStats(status, stats, sourceStats) {
  const platformCounts = stats.platform_counts || [];
  const sourcePlatforms = sourceStats.platform || sourceStats.platforms;
  return {
    messageCount: sourceStats.message_count ?? stats.total_messages ?? 0,
    dailyIncrease: sourceStats.daily_increase ?? stats.daily_increase ?? 0,
    platformCount: sourceStats.platform_count ?? status.platforms?.platform_count ?? platformCounts.length,
    uptimeSeconds: sourceStats.uptime_seconds ?? stats.uptime_seconds ?? runningToSeconds(sourceStats.running),
    memory: {
      process: sourceStats.memory?.process ?? stats.memory?.process ?? stats.process_memory_mib ?? 0,
      system: sourceStats.memory?.system ?? stats.memory?.system ?? stats.system_memory_mib ?? 0,
    },
    cpuPercent: sourceStats.cpu_percent ?? stats.cpu_percent ?? 0,
    platforms: Array.isArray(sourcePlatforms)
      ? sourcePlatforms.map((item) => ({ name: item.name || item.platform_id, count: Number(item.count || 0) }))
      : platformCounts.map((item) => ({ name: `${item.platform_id} · ${item.platform_type}`, count: Number(item.count || 0) })),
    messageSeries: sourceStats.message_time_series || sourceStats.messageTimeSeries || seriesFromRecentEvents(stats.recent_events || []),
    lastUpdated: sourceStats.last_updated || new Date((stats.generated_at_unix || Date.now() / 1000) * 1000).toLocaleTimeString(),
  };
}

function seriesFromRecentEvents(events) {
  const buckets = new Map();
  for (const event of events) {
    if (event.kind !== "platform_message") continue;
    const key = event.timestamp || "unknown";
    buckets.set(key, (buckets.get(key) || 0) + Number(event.count || 0));
  }
  return [...buckets.entries()].map(([timestamp, count]) => [timestamp, count]);
}

function barChart(items) {
  const max = Math.max(...items.map((item) => Number(item.value) || 0), 1);
  return `
    <div class="bar-list">
      ${items.map((item) => {
        const value = Math.max(0, Number(item.value) || 0);
        const width = Math.max(4, Math.round((value / max) * 100));
        return `
          <div class="bar-row">
            <span>${escapeHtml(item.label)}</span>
            <div class="bar-track"><div class="bar-fill" style="width: ${width}%"></div></div>
            <strong>${escapeHtml(value)}</strong>
          </div>
        `;
      }).join("")}
    </div>
  `;
}

function growthRate(series) {
  if (series.length < 4) return 0;
  const half = Math.floor(series.length / 2);
  const first = series.slice(0, half).reduce((sum, item) => sum + Number(item[1] || 0), 0);
  const second = series.slice(half).reduce((sum, item) => sum + Number(item[1] || 0), 0);
  if (first === 0) return second > 0 ? 100 : 0;
  return Math.round(((second - first) / first) * 100);
}

function welcomeAnnouncement() {
  const raw = state.welcomeAnnouncement;
  if (typeof raw === "string") return raw.trim();
  if (raw && typeof raw === "object") {
    return raw.zh_CN || raw["zh-CN"] || raw.zh || raw.en_US || raw["en-US"] || raw.en || "";
  }
  return "### Welcome to AstrBot\n\n完成后端、平台和 Provider 配置后即可开始使用。";
}

function greetingText() {
  const hour = new Date().getHours();
  if (hour < 12) return "早上好 😊";
  if (hour < 18) return "下午好 😊";
  return "晚上好 😊";
}

function runningToSeconds(running) {
  if (!running) return 0;
  return Number(running.hours || 0) * 3600 + Number(running.minutes || 0) * 60 + Number(running.seconds || 0);
}

function formatDuration(seconds) {
  const value = Number(seconds || 0);
  if (value < 60) return `${value}s`;
  const minutes = Math.floor(value / 60);
  const remaining = value % 60;
  if (minutes < 60) return `${minutes}m ${remaining}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

function formatNumber(value) {
  return new Intl.NumberFormat("zh-CN").format(Number(value || 0));
}

function formatSeriesLabel(value) {
  if (typeof value === "number") {
    return new Date(value).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  }
  return String(value || "-").replace("T", " ").replace("Z", "");
}

function normalizePath(path) {
  const normalized = String(path || "/welcome").replace(/\/+$/, "");
  return normalized || "/";
}
