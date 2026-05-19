import { createSignal, onMount } from "solid-js";

export type Theme = "light" | "dark";

const THEME_KEY = "astrbot.dashboard.theme";

function detect(): Theme {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "light" || stored === "dark") return stored;
  } catch {
    /* ignore */
  }
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

const [theme, setThemeSignal] = createSignal<Theme>(detect());

export { theme };

export function setTheme(value: Theme): void {
  setThemeSignal(value);
  try {
    localStorage.setItem(THEME_KEY, value);
  } catch {
    /* ignore */
  }
  document.documentElement.dataset["theme"] = value;
}

export function toggleTheme(): void {
  setTheme(theme() === "dark" ? "light" : "dark");
}

export function useThemeMount(): void {
  onMount(() => {
    document.documentElement.dataset["theme"] = theme();
  });
}
