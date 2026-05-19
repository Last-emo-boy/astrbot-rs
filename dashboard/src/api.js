const MANAGEMENT_TOKEN_KEY = "astrbot.managementToken";
const OPENAPI_SECRET_KEY = "astrbot.openapiSecret";
const API_BASE_KEY = "astrbot.apiBase";
const DASHBOARD_PREFS_KEY = "astrbot.dashboardPreferences";

const DEFAULT_DASHBOARD_PREFERENCES = {
  theme: "light",
  sidebarCompact: false,
  primaryColor: "",
  secondaryColor: "",
  githubProxyEnabled: false,
  githubProxyUrl: "",
  apiBasePresets: [],
  sidebarMainItems: [],
  sidebarMoreItems: [],
};

export function managementToken() {
  return window.localStorage.getItem(MANAGEMENT_TOKEN_KEY) || "";
}

export function setManagementToken(token) {
  const normalized = token.trim();
  if (normalized) {
    window.localStorage.setItem(MANAGEMENT_TOKEN_KEY, normalized);
  } else {
    window.localStorage.removeItem(MANAGEMENT_TOKEN_KEY);
  }
}

export function openApiSecret() {
  return window.localStorage.getItem(OPENAPI_SECRET_KEY) || "";
}

export function setOpenApiSecret(secret) {
  const normalized = secret.trim();
  if (normalized) {
    window.localStorage.setItem(OPENAPI_SECRET_KEY, normalized);
  } else {
    window.localStorage.removeItem(OPENAPI_SECRET_KEY);
  }
}

export function apiBase() {
  return window.localStorage.getItem(API_BASE_KEY) || "";
}

export function setApiBase(base) {
  const normalized = base.trim().replace(/\/+$/, "");
  if (normalized) {
    window.localStorage.setItem(API_BASE_KEY, normalized);
  } else {
    window.localStorage.removeItem(API_BASE_KEY);
  }
}

export function apiUrl(path) {
  if (/^https?:\/\//.test(path)) return path;
  const base = apiBase();
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return `${base}${normalizedPath}`;
}

export function dashboardPreferences() {
  try {
    return normalizeDashboardPreferences(JSON.parse(window.localStorage.getItem(DASHBOARD_PREFS_KEY) || "{}"));
  } catch {
    return { ...DEFAULT_DASHBOARD_PREFERENCES };
  }
}

export function setDashboardPreferences(preferences) {
  const normalized = normalizeDashboardPreferences(preferences);
  window.localStorage.setItem(DASHBOARD_PREFS_KEY, JSON.stringify(normalized));
  applyDashboardPreferences(normalized);
}

export function applyDashboardPreferences(preferences = dashboardPreferences()) {
  document.body.dataset.theme = preferences.theme === "dark" ? "dark" : "light";
  document.body.classList.toggle("sidebar-compact", Boolean(preferences.sidebarCompact));
  applyOptionalCssVariable("--primary", sanitizeCssColor(preferences.primaryColor));
  applyOptionalCssVariable("--accent", sanitizeCssColor(preferences.secondaryColor));
}

function applyOptionalCssVariable(name, value) {
  if (value) {
    document.documentElement.style.setProperty(name, value);
  } else {
    document.documentElement.style.removeProperty(name);
  }
}

function sanitizeCssColor(value) {
  const color = String(value || "").trim();
  return /^#[0-9a-fA-F]{6}$/.test(color) ? color : "";
}

function normalizeDashboardPreferences(preferences = {}) {
  return {
    ...DEFAULT_DASHBOARD_PREFERENCES,
    theme: preferences.theme === "dark" ? "dark" : "light",
    sidebarCompact: Boolean(preferences.sidebarCompact),
    primaryColor: sanitizeCssColor(preferences.primaryColor),
    secondaryColor: sanitizeCssColor(preferences.secondaryColor),
    githubProxyEnabled: Boolean(preferences.githubProxyEnabled),
    githubProxyUrl: String(preferences.githubProxyUrl || "").trim().replace(/\/+$/, ""),
    apiBasePresets: normalizeApiBasePresets(preferences.apiBasePresets),
    sidebarMainItems: normalizeRouteIdList(preferences.sidebarMainItems),
    sidebarMoreItems: normalizeRouteIdList(preferences.sidebarMoreItems),
  };
}

function normalizeRouteIdList(value = []) {
  const seen = new Set();
  return (Array.isArray(value) ? value : [])
    .map((item) => String(item || "").trim())
    .filter((item) => {
      if (!item || seen.has(item)) return false;
      seen.add(item);
      return true;
    });
}

function normalizeApiBasePresets(presets = []) {
  return presets
    .map((preset) => ({
      name: String(preset?.name || "").trim(),
      url: String(preset?.url || "").trim().replace(/\/+$/, ""),
    }))
    .filter((preset) => preset.name && /^https?:\/\//.test(preset.url))
    .slice(0, 12);
}

export async function api(path, options = {}) {
  const token = managementToken();
  const response = await fetch(apiUrl(path), {
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(options.headers || {}),
    },
    ...options,
  });
  const text = await response.text();
  const data = text ? JSON.parse(text) : null;
  if (!response.ok) {
    const message = data?.error || data?.message || `${response.status} ${response.statusText}`;
    throw new Error(response.status === 401 ? `未授权：${message}` : message);
  }
  return data;
}

export async function openApi(path, options = {}) {
  const secret = openApiSecret();
  const response = await fetch(apiUrl(path), {
    headers: {
      "Content-Type": "application/json",
      ...(secret ? { Authorization: `Bearer ${secret}` } : {}),
      ...(options.headers || {}),
    },
    ...options,
  });
  const text = await response.text();
  const data = text ? JSON.parse(text) : null;
  if (!response.ok) {
    const message = data?.error || data?.message || `${response.status} ${response.statusText}`;
    throw new Error(response.status === 401 ? `OpenAPI 未授权：${message}` : message);
  }
  return data;
}

export async function safeApi(path, fallback, options = {}) {
  try {
    return await api(path, options);
  } catch (error) {
    return { ...fallback, unavailable: error.message };
  }
}

export function desktopBridgeSnapshot() {
  const bridge = globalThis.window?.astrbotDesktop;
  const updater = globalThis.window?.astrbotAppUpdater;
  return {
    mode: bridge ? "desktop" : "browser",
    bridgePresent: Boolean(bridge),
    updaterPresent: Boolean(updater),
    isDesktop: Boolean(bridge?.isDesktop),
    hasRuntimeProbe: typeof bridge?.isDesktopRuntime === "function",
    hasBackendState: typeof bridge?.getBackendState === "function",
    hasBackendRestart: typeof bridge?.restartBackend === "function",
    hasBackendStop: typeof bridge?.stopBackend === "function",
    hasTrayRestartListener: typeof bridge?.onTrayRestartBackend === "function",
    backendState: null,
    appUpdate: null,
    checkedAt: null,
    fallbackReason: bridge ? "" : "desktop bridge unavailable",
  };
}

export async function probeDesktopBridge() {
  const bridge = globalThis.window?.astrbotDesktop;
  const snapshot = desktopBridgeSnapshot();
  snapshot.checkedAt = new Date().toISOString();
  if (!bridge) return snapshot;

  if (typeof bridge.isDesktopRuntime === "function") {
    try {
      snapshot.isDesktop = snapshot.isDesktop || Boolean(await bridge.isDesktopRuntime());
    } catch (error) {
      snapshot.runtimeError = error.message;
    }
  }

  if (typeof bridge.getBackendState === "function") {
    try {
      snapshot.backendState = await bridge.getBackendState();
    } catch (error) {
      snapshot.backendError = error.message;
    }
  }
  return snapshot;
}

export async function restartDesktopBackend(authToken = managementToken()) {
  const bridge = globalThis.window?.astrbotDesktop;
  if (!bridge || typeof bridge.restartBackend !== "function") {
    return { ok: false, reason: "desktop bridge unavailable", fallback: true };
  }
  return bridge.restartBackend(authToken || null);
}

export async function checkDesktopAppUpdate() {
  const updater = globalThis.window?.astrbotAppUpdater;
  if (!updater || typeof updater.checkForAppUpdate !== "function") {
    return { ok: false, reason: "desktop app updater unavailable", hasUpdate: false, fallback: true };
  }
  return updater.checkForAppUpdate();
}

export async function installDesktopAppUpdate() {
  const updater = globalThis.window?.astrbotAppUpdater;
  if (!updater || typeof updater.installAppUpdate !== "function") {
    return { ok: false, reason: "desktop app updater unavailable", fallback: true };
  }
  return updater.installAppUpdate();
}
