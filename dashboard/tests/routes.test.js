import assert from "node:assert/strict";
import test from "node:test";

globalThis.window = {
  location: { hash: "", pathname: "/", search: "" },
  localStorage: {
    getItem() {
      return null;
    },
    setItem() {},
    removeItem() {},
  },
};

const {
  dashboardRouteById,
  guardDashboardRoute,
  routeStateFromLocation,
  routeStateFromRouteId,
  routeStateFromRouteInput,
} = await import("../src/routes.js");

test("source dashboard routes resolve to target route ids and layouts", () => {
  const cases = [
    ["/", "overview", "full"],
    ["/main", "overview", "full"],
    ["/welcome", "overview", "full"],
    ["/dashboard/default", "overview", "full"],
    ["/extension", "plugins", "full"],
    ["/extension-marketplace", "market", "full"],
    ["/extension/tools", "tools", "full"],
    ["/extension/skills", "skills", "full"],
    ["/platforms", "platforms", "full"],
    ["/providers", "providers", "full"],
    ["/config", "config", "full"],
    ["/conversation", "conversation", "full"],
    ["/session-management", "sessions", "full"],
    ["/persona", "personas", "full"],
    ["/subagent", "subagent", "full"],
    ["/cron", "cron", "full"],
    ["/console", "console", "full"],
    ["/trace", "trace", "full"],
    ["/knowledge-base", "knowledge", "full"],
    ["/chat/projects", "projects", "full"],
    ["/settings", "settings", "full"],
    ["/settings/backup", "backup", "full"],
    ["/settings/update", "update", "full"],
    ["/about", "about", "full"],
    ["/auth/login", "login", "blank"],
    ["/chatbox", "chatbox", "blank"],
  ];

  for (const [path, id, layout] of cases) {
    const state = routeStateFromRouteInput(path);
    assert.equal(state.id, id, path);
    assert.equal(state.layout, layout, path);
  }
});

test("dynamic and legacy routes keep params or replacement targets", () => {
  assert.deepEqual(routeStateFromRouteInput("/chat/conversation-1").params, {
    conversationId: "conversation-1",
  });
  assert.deepEqual(routeStateFromRouteInput("/chatbox/conversation-2").params, {
    conversationId: "conversation-2",
  });
  assert.deepEqual(routeStateFromRouteInput("/knowledge-base/kb-1/document/doc-1").params, {
    kbId: "kb-1",
    docId: "doc-1",
  });

  assert.equal(routeStateFromRouteInput("/normal").id, "config");
  assert.equal(routeStateFromRouteInput("/normal").fragment, "normal");
  assert.equal(routeStateFromRouteInput("/system").fragment, "system");
  assert.equal(routeStateFromRouteInput("/logs").id, "console");
  assert.equal(routeStateFromRouteInput("/tool-use").id, "tools");
  assert.equal(routeStateFromRouteInput("/settings/backup").id, "backup");
  assert.equal(routeStateFromRouteInput("/settings/update").id, "update");
  assert.equal(routeStateFromRouteInput("/chat/projects").id, "projects");
  assert.equal(routeStateFromRouteInput("/about").replacementFor, "");
  const legacyKb = routeStateFromRouteInput("/alkaid/knowledge-base");
  assert.equal(legacyKb.id, "knowledge");
  assert.equal(legacyKb.sourcePath, "/alkaid/knowledge-base");
  assert.equal(legacyKb.replacementFor, "");

  for (const legacyPath of ["/alkaid", "/alkaid/long-term-memory", "/alkaid/other"]) {
    const legacyRoute = routeStateFromRouteInput(legacyPath);
    assert.equal(legacyRoute.id, "about", legacyPath);
    assert.equal(legacyRoute.sourcePath, legacyPath);
    assert.equal(legacyRoute.replacementFor, "legacy-alkaid", legacyPath);
  }
  assert.equal(routeStateFromRouteInput("/missing").notFound, true);
});

test("auth guard preserves returnUrl and keeps blank public routes open", () => {
  const protectedRoute = routeStateFromRouteInput("/providers");
  const guarded = guardDashboardRoute(protectedRoute, "");
  assert.equal(guarded.action, "redirect");
  assert.equal(guarded.target.id, "login");
  assert.equal(guarded.target.layout, "blank");
  assert.equal(guarded.target.returnUrl, "/providers");

  const loggedInLogin = guardDashboardRoute(
    routeStateFromRouteInput("/auth/login?returnUrl=%2Fproviders"),
    "token",
  );
  assert.equal(loggedInLogin.action, "redirect");
  assert.equal(loggedInLogin.target.id, "providers");

  assert.equal(guardDashboardRoute(routeStateFromRouteInput("/chatbox/demo"), "").action, "allow");
});

test("hash and id inputs remain compatible with existing navigation", () => {
  assert.equal(routeStateFromRouteInput("providers").id, "providers");
  assert.equal(routeStateFromRouteInput("#/providers").id, "providers");
  assert.equal(routeStateFromRouteId("providers").hash, "#/providers");

  globalThis.window.location = { hash: "#/auth/login?returnUrl=%2Fconfig", pathname: "/", search: "" };
  const state = routeStateFromLocation();
  assert.equal(state.id, "login");
  assert.equal(state.returnUrl, "/config");
  assert.equal(dashboardRouteById("login").requiresAuth, false);
});
