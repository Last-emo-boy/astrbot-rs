import { lazy, type Component } from "solid-js";
import type { RouteDefinition, RouteSectionProps } from "@solidjs/router";
import { AppShell } from "./layouts/AppShell";

const wrap = (importer: () => Promise<{ default: Component }>): Component<RouteSectionProps> => {
  const Comp = lazy(importer);
  return () => (
    <AppShell>
      <Comp />
    </AppShell>
  );
};

const Login = lazy(() => import("./pages/login"));
const Welcome = lazy(() => import("./pages/welcome"));

export const routes: RouteDefinition[] = [
  { path: "/auth/login", component: Login },
  { path: "/welcome", component: wrap(() => import("./pages/welcome")) },
  { path: "/", component: wrap(() => import("./pages/overview")) },
  { path: "/main", component: wrap(() => import("./pages/overview")) },
  { path: "/console", component: wrap(() => import("./pages/console")) },
  { path: "/logs", component: wrap(() => import("./pages/console")) },
  { path: "/trace", component: wrap(() => import("./pages/trace")) },
  { path: "/observability", component: wrap(() => import("./pages/observability")) },
  { path: "/chat", component: wrap(() => import("./pages/chat")) },
  { path: "/chat/projects", component: wrap(() => import("./pages/projects")) },
  { path: "/chatbox", component: wrap(() => import("./pages/chat")) },
  { path: "/chatbox/:id", component: wrap(() => import("./pages/chat")) },
  { path: "/chat/:id", component: wrap(() => import("./pages/chat")) },
  { path: "/conversation", component: wrap(() => import("./pages/conversation")) },
  { path: "/persona", component: wrap(() => import("./pages/persona")) },
  { path: "/providers", component: wrap(() => import("./pages/providers")) },
  { path: "/platforms", component: wrap(() => import("./pages/platforms")) },
  { path: "/mcp", component: wrap(() => import("./pages/mcp")) },
  { path: "/tool-use", component: wrap(() => import("./pages/tools")) },
  { path: "/extension", component: wrap(() => import("./pages/plugins")) },
  { path: "/extension/skills", component: wrap(() => import("./pages/skills")) },
  { path: "/extension/tools", component: wrap(() => import("./pages/tools")) },
  { path: "/extension-marketplace", component: wrap(() => import("./pages/market")) },
  { path: "/subagent", component: wrap(() => import("./pages/subagent")) },
  { path: "/knowledge-base", component: wrap(() => import("./pages/knowledge")) },
  { path: "/knowledge-base/:kbId", component: wrap(() => import("./pages/knowledge")) },
  { path: "/knowledge-base/:kbId/document/:docId", component: wrap(() => import("./pages/knowledge")) },
  { path: "/alkaid", component: wrap(() => import("./pages/knowledge")) },
  { path: "/alkaid/knowledge-base", component: wrap(() => import("./pages/knowledge")) },
  { path: "/alkaid/long-term-memory", component: wrap(() => import("./pages/observability")) },
  { path: "/alkaid/other", component: wrap(() => import("./pages/observability")) },
  { path: "/cron", component: wrap(() => import("./pages/cron")) },
  { path: "/session-management", component: wrap(() => import("./pages/sessions")) },
  { path: "/config", component: wrap(() => import("./pages/config")) },
  { path: "/api-keys", component: wrap(() => import("./pages/api-keys")) },
  { path: "/dashboard/default", component: wrap(() => import("./pages/overview")) },
  { path: "/normal", component: wrap(() => import("./pages/overview")) },
  { path: "/system", component: wrap(() => import("./pages/overview")) },
  { path: "/settings", component: wrap(() => import("./pages/settings")) },
  { path: "/settings/backup", component: wrap(() => import("./pages/backup")) },
  { path: "/settings/update", component: wrap(() => import("./pages/update")) },
  { path: "/about", component: wrap(() => import("./pages/about")) },
  { path: "/t2i-templates", component: wrap(() => import("./pages/t2i")) },
  { path: "*", component: wrap(() => import("./pages/overview")) },
];

export { Login as LoginPage, Welcome as WelcomePage };
