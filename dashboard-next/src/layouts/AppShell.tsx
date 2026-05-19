import { A, useLocation, useNavigate } from "@solidjs/router";
import { For, Show, type Component, type JSX } from "solid-js";
import { logout, token } from "@/api/auth";
import { Button } from "@/components/Form";
import { locale, setLocale, t } from "@/i18n";
import { theme, toggleTheme, useThemeMount } from "@/styles/theme";

interface NavGroup {
  label: string;
  links: { to: string; key: string }[];
}

const NAV: NavGroup[] = [
  {
    label: "概览",
    links: [
      { to: "/", key: "nav.overview" },
      { to: "/console", key: "nav.console" },
      { to: "/trace", key: "nav.trace" },
      { to: "/observability", key: "nav.observability" },
    ],
  },
  {
    label: "对话",
    links: [
      { to: "/chat", key: "nav.chat" },
      { to: "/conversation", key: "nav.conversation" },
      { to: "/chat/projects", key: "nav.projects" },
      { to: "/persona", key: "nav.persona" },
    ],
  },
  {
    label: "模型与平台",
    links: [
      { to: "/providers", key: "nav.providers" },
      { to: "/platforms", key: "nav.platforms" },
      { to: "/mcp", key: "nav.mcp" },
      { to: "/tool-use", key: "nav.tools" },
    ],
  },
  {
    label: "扩展",
    links: [
      { to: "/extension", key: "nav.plugins" },
      { to: "/extension-marketplace", key: "nav.market" },
      { to: "/extension/skills", key: "nav.skills" },
      { to: "/subagent", key: "nav.subagent" },
    ],
  },
  {
    label: "数据",
    links: [
      { to: "/knowledge-base", key: "nav.knowledge" },
      { to: "/cron", key: "nav.cron" },
      { to: "/session-management", key: "nav.sessions" },
      { to: "/t2i-templates", key: "nav.t2i" },
    ],
  },
  {
    label: "运维",
    links: [
      { to: "/config", key: "nav.config" },
      { to: "/api-keys", key: "nav.apiKeys" },
      { to: "/settings/backup", key: "nav.backup" },
      { to: "/settings/update", key: "nav.update" },
      { to: "/settings", key: "nav.settings" },
      { to: "/about", key: "nav.about" },
    ],
  },
];

interface AppShellProps {
  children: JSX.Element;
}

export const AppShell: Component<AppShellProps> = (props) => {
  useThemeMount();
  const location = useLocation();
  const navigate = useNavigate();
  const isActive = (to: string) => {
    const path = location.pathname;
    if (to === "/") return path === "/" || path === "";
    return path === to || path.startsWith(`${to}/`);
  };

  const handleLogout = () => {
    logout();
    navigate("/auth/login");
  };

  return (
    <Show
      when={token() !== null}
      fallback={
        <div class="auth-shell">
          <div class="auth-shell__card">
            <p>{t("welcome.subtitle")}</p>
            <Button variant="primary" onClick={() => navigate("/auth/login")}>
              {t("login.submit")}
            </Button>
          </div>
        </div>
      }
    >
      <div class="app-shell">
        <aside class="app-shell__sidebar">
          <div class="sidebar__brand">{t("app.title")}</div>
          <For each={NAV}>
            {(group) => (
              <>
                <div class="sidebar__group">{group.label}</div>
                <For each={group.links}>
                  {(link) => (
                    <A
                      href={link.to}
                      class="sidebar__link"
                      classList={{ "sidebar__link--active": isActive(link.to) }}
                    >
                      {t(link.key)}
                    </A>
                  )}
                </For>
              </>
            )}
          </For>
        </aside>
        <header class="app-shell__topbar">
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setLocale(locale() === "zh" ? "en" : "zh")}
          >
            {t("locale.toggle")}
          </Button>
          <Button size="sm" variant="ghost" onClick={toggleTheme}>
            {theme() === "dark" ? "☾" : "☀"}
          </Button>
          <Button size="sm" variant="ghost" onClick={handleLogout}>
            {t("common.logout")}
          </Button>
        </header>
        <main class="app-shell__main">{props.children}</main>
      </div>
    </Show>
  );
};
