import { dashboardPreferences } from "./api.js";
import { t } from "./i18n.js";

const DEFAULT_SIDEBAR_MAIN_IDS = [
  "overview",
  "chat",
  "platforms",
  "providers",
  "config",
  "plugins",
  "knowledge",
  "personas",
];

const routeDefinitions = [
  {
    titleKey: "group.runtime",
    routes: [
      {
        id: "overview",
        icon: "⊞",
        path: "/welcome",
        aliases: ["/", "/dashboard/default"],
        redirects: { "/main": "/welcome" },
      },
      {
        id: "chat",
        icon: "✉",
        path: "/chat",
        patterns: ["/chat/:conversationId"],
      },
      {
        id: "chatbox",
        icon: "▣",
        path: "/chatbox",
        patterns: ["/chatbox/:conversationId"],
        layout: "blank",
        requiresAuth: false,
      },
      { id: "conversation", icon: "▥", path: "/conversation" },
      {
        id: "console",
        icon: "⌁",
        path: "/console",
        redirects: { "/logs": "/console" },
      },
      { id: "trace", icon: "⎇", path: "/trace" },
    ],
  },
  {
    titleKey: "group.config",
    routes: [
      {
        id: "config",
        icon: "⚙",
        path: "/config",
        redirects: {
          "/normal": "/config#normal",
          "/system": "/config#system",
        },
      },
      { id: "providers", icon: "◈", path: "/providers" },
      { id: "platforms", icon: "▤", path: "/platforms" },
      { id: "sessions", icon: "☷", path: "/session-management" },
      { id: "personas", icon: "◐", path: "/persona" },
      { id: "cron", icon: "◷", path: "/cron" },
    ],
  },
  {
    titleKey: "group.extensions",
    routes: [
      { id: "plugins", icon: "✦", path: "/extension" },
      { id: "market", icon: "⌘", path: "/extension-marketplace" },
      { id: "skills", icon: "✎", path: "/extension/skills" },
      { id: "subagent", icon: "⎈", path: "/subagent" },
      {
        id: "tools",
        icon: "⌬",
        path: "/extension/tools",
        redirects: { "/tool-use": "/extension/tools" },
      },
    ],
  },
  {
    titleKey: "group.data",
    routes: [
      {
        id: "knowledge",
        icon: "▣",
        path: "/knowledge-base",
        aliases: ["/alkaid/knowledge-base"],
        patterns: ["/knowledge-base/:kbId", "/knowledge-base/:kbId/document/:docId"],
      },
      { id: "projects", icon: "□", path: "/chat/projects" },
      { id: "backup", icon: "⇅", path: "/settings/backup" },
      { id: "update", icon: "↑", path: "/settings/update" },
      { id: "settings", icon: "◌", path: "/settings" },
      {
        id: "about",
        icon: "i",
        path: "/about",
        aliases: ["/alkaid", "/alkaid/long-term-memory", "/alkaid/other"],
        replacementFor: "legacy-alkaid",
      },
    ],
  },
];

const hiddenRoutes = [
  {
    id: "login",
    path: "/auth/login",
    layout: "blank",
    requiresAuth: false,
    title: "登录 Dashboard",
    subtitle: "Management authentication",
  },
];

function localizeRoute(route) {
  return {
    ...route,
    label: t(`route.${route.id}.label`),
    title: t(`route.${route.id}.title`),
    subtitle: t(`route.${route.id}.subtitle`),
  };
}

export function localizedRouteGroups() {
  const sidebar = resolveSidebarPreferences();
  if (!sidebar.customized) {
    return routeDefinitions.map((group) => ({
      title: t(group.titleKey),
      routes: group.routes.map(localizeRoute),
    }));
  }

  const routeById = new Map(routeDefinitions.flatMap((group) => group.routes.map((route) => [route.id, route])));
  const visibleGroups = routeDefinitions
    .map((group) => ({
      title: t(group.titleKey),
      routes: sortByOrder(
        group.routes.filter((route) => sidebar.mainItems.includes(route.id)),
        sidebar.mainItems,
      ).map(localizeRoute),
    }))
    .filter((group) => group.routes.length);
  const moreRoutes = sidebar.moreItems
    .map((id) => routeById.get(id))
    .filter(Boolean)
    .map(localizeRoute);
  return moreRoutes.length
    ? [...visibleGroups, { title: t("core.navigation.groups.more"), routes: moreRoutes }]
    : visibleGroups;
}

export function localizedRoutes() {
  return routeDefinitions.flatMap((group) => group.routes).map(localizeRoute);
}

export const routeGroups = localizedRouteGroups();
export const routes = localizedRoutes();

export function dashboardRouteRecords() {
  return [
    ...routeDefinitions.flatMap((group) => group.routes),
    ...hiddenRoutes,
  ].map((route) => ({
    layout: "full",
    requiresAuth: true,
    aliases: [],
    patterns: [],
    redirects: {},
    ...route,
  }));
}

export function dashboardRouteById(routeId) {
  return dashboardRouteRecords().find((route) => route.id === routeId) || null;
}

export function allSidebarRoutes() {
  return routeDefinitions.flatMap((group) => group.routes).map(localizeRoute);
}

export function defaultSidebarCustomization() {
  const allIds = routeDefinitions.flatMap((group) => group.routes.map((route) => route.id));
  return {
    mainItems: DEFAULT_SIDEBAR_MAIN_IDS.filter((id) => allIds.includes(id)),
    moreItems: allIds.filter((id) => !DEFAULT_SIDEBAR_MAIN_IDS.includes(id)),
  };
}

export function resolveSidebarPreferences(preferences = dashboardPreferences()) {
  const defaults = defaultSidebarCustomization();
  const allIds = [...defaults.mainItems, ...defaults.moreItems];
  const hasCustomization = preferences.sidebarMainItems.length > 0 || preferences.sidebarMoreItems.length > 0;
  if (!hasCustomization) {
    return { ...defaults, customized: false };
  }

  const mainItems = normalizeSidebarIds(preferences.sidebarMainItems, allIds);
  const mainSet = new Set(mainItems);
  const moreItems = normalizeSidebarIds(preferences.sidebarMoreItems, allIds).filter((id) => !mainSet.has(id));
  const used = new Set([...mainItems, ...moreItems]);
  const missing = allIds.filter((id) => !used.has(id));
  return {
    mainItems: [...mainItems, ...missing.filter((id) => defaults.mainItems.includes(id))],
    moreItems: [...moreItems, ...missing.filter((id) => defaults.moreItems.includes(id))],
    customized: true,
  };
}

export function routeStateFromLocation(location = globalThis.window?.location) {
  const hash = location?.hash || "";
  if (hash && hash !== "#") {
    return routeStateFromRouteInput(hash.slice(1));
  }
  return routeStateFromRouteInput(`${location?.pathname || "/"}${location?.search || ""}`);
}

export function routeStateFromRouteId(routeId, options = {}) {
  const route = dashboardRouteById(routeId) || dashboardRouteById("overview");
  return routeStateForRecord(route, {
    path: options.path || route.path,
    params: options.params || {},
    fragment: options.fragment || "",
    returnUrl: options.returnUrl || "",
  });
}

export function routeStateFromRouteInput(input) {
  const parsed = parseRouteInput(input);
  if (!parsed.path.startsWith("/")) {
    const byId = dashboardRouteById(parsed.path);
    if (byId) {
      return routeStateForRecord(byId, parsed);
    }
  }

  const normalizedPath = normalizeDashboardPath(parsed.path);
  for (const route of dashboardRouteRecords()) {
    const redirected = route.redirects[normalizedPath];
    if (redirected) {
      const target = parseRouteInput(redirected);
      return routeStateForRecord(route, {
        ...parsed,
        path: target.path,
        fragment: target.fragment || parsed.fragment,
        redirectedFrom: normalizedPath,
      });
    }

    if ([route.path, ...route.aliases].some((path) => normalizeDashboardPath(path) === normalizedPath)) {
      return routeStateForRecord(route, parsed);
    }
  }

  for (const route of dashboardRouteRecords()) {
    for (const pattern of route.patterns) {
      const params = matchPattern(pattern, normalizedPath);
      if (params) {
        return routeStateForRecord(route, {
          ...parsed,
          params,
          pattern,
        });
      }
    }
  }

  return {
    ...routeStateFromRouteId("overview"),
    notFound: true,
    sourcePath: normalizedPath,
    returnUrl: parsed.returnUrl,
  };
}

export function guardDashboardRoute(routeState, token) {
  if (routeState.id === "login" && token) {
    return {
      action: "redirect",
      target: routeStateFromRouteInput(routeState.returnUrl || "/welcome"),
    };
  }
  if (routeState.requiresAuth && !token) {
    return {
      action: "redirect",
      target: routeStateFromRouteInput(
        `/auth/login?returnUrl=${encodeURIComponent(routeState.sourcePath || routeState.path)}`,
      ),
    };
  }
  return { action: "allow", target: routeState };
}

export function normalizeDashboardPath(path) {
  let normalized = String(path || "/").trim();
  if (!normalized) return "/";
  if (!normalized.startsWith("/")) return normalized;
  normalized = normalized.replace(/\/+$/, "");
  return normalized || "/";
}

function routeStateForRecord(route, parsed) {
  const path = normalizeDashboardPath(parsed.path || route.path);
  const sourcePath = normalizeDashboardPath(parsed.redirectedFrom || parsed.path || route.path);
  const fragment = parsed.fragment || "";
  const returnUrl = parsed.returnUrl || "";
  const query = returnUrl && route.id === "login" ? `?returnUrl=${encodeURIComponent(returnUrl)}` : "";
  const isReplacementAlias = Boolean(route.replacementFor)
    && sourcePath !== normalizeDashboardPath(route.path);
  return {
    id: route.id,
    path,
    sourcePath,
    canonicalPath: route.path,
    hash: `#${path}${query}${fragment ? `#${fragment}` : ""}`,
    layout: route.layout,
    requiresAuth: route.requiresAuth,
    params: parsed.params || {},
    fragment,
    returnUrl,
    redirectedFrom: parsed.redirectedFrom || "",
    replacementFor: isReplacementAlias ? route.replacementFor : "",
    notFound: false,
  };
}

function parseRouteInput(input) {
  let route = String(input || "/").trim();
  if (!route) route = "/";
  if (route.startsWith("#")) route = route.slice(1);
  if (!route) route = "/";

  const fragmentParts = route.split("#");
  const beforeFragment = fragmentParts.shift() || "/";
  const fragment = fragmentParts.join("#");
  const queryIndex = beforeFragment.indexOf("?");
  const path = queryIndex >= 0 ? beforeFragment.slice(0, queryIndex) || "/" : beforeFragment;
  const query = queryIndex >= 0 ? beforeFragment.slice(queryIndex + 1) : "";
  const params = new URLSearchParams(query);

  return {
    path: normalizeDashboardPath(path),
    fragment,
    returnUrl: params.get("returnUrl") || "",
    params: {},
  };
}

function matchPattern(pattern, path) {
  const patternParts = normalizeDashboardPath(pattern).split("/").filter(Boolean);
  const pathParts = normalizeDashboardPath(path).split("/").filter(Boolean);
  if (patternParts.length !== pathParts.length) return null;

  const params = {};
  for (let index = 0; index < patternParts.length; index += 1) {
    const patternPart = patternParts[index];
    const pathPart = pathParts[index];
    if (patternPart.startsWith(":")) {
      params[patternPart.slice(1)] = decodeURIComponent(pathPart);
    } else if (patternPart !== pathPart) {
      return null;
    }
  }
  return params;
}

function sortByOrder(routesToSort, order) {
  const orderIndex = new Map(order.map((id, index) => [id, index]));
  return [...routesToSort].sort((left, right) => (
    (orderIndex.get(left.id) ?? Number.MAX_SAFE_INTEGER)
    - (orderIndex.get(right.id) ?? Number.MAX_SAFE_INTEGER)
  ));
}

function normalizeSidebarIds(ids, allIds) {
  const valid = new Set(allIds);
  const seen = new Set();
  return (Array.isArray(ids) ? ids : [])
    .map((id) => String(id || "").trim())
    .filter((id) => {
      if (!valid.has(id) || seen.has(id)) return false;
      seen.add(id);
      return true;
    });
}
