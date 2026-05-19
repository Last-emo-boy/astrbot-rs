import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config for the dashboard-next end-to-end suite.
 *
 * The dev server is launched on `localhost:5173` (vite's default) before
 * any tests run. CI runs only in Chromium to keep wall-clock down; locally
 * developers can `npx playwright test --project="webkit"` etc. to widen
 * coverage.
 */
export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "npm run dev",
    url: "http://127.0.0.1:5173",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
