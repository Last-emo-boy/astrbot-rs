import { expect, test } from "@playwright/test";

/**
 * Smoke test: the dashboard root loads without runtime errors.
 *
 * We do not assume an authenticated session — the unauthenticated landing
 * (either /login or the welcome page) is enough to confirm that the SPA
 * mounted and the topbar/sidebar rendered. Anything more aggressive must
 * stub the backend API.
 */
test.describe("dashboard shell", () => {
  test("loads the SPA root without console errors", async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") {
        consoleErrors.push(message.text());
      }
    });

    await page.goto("/", { waitUntil: "domcontentloaded" });
    // SolidJS apps mount synchronously; wait for the root container to
    // render something other than the index.html skeleton.
    await expect(page.locator("body")).not.toBeEmpty();

    // Filter known-benign errors that depend on backend availability
    // (the dashboard probes /api/management/* before the user signs in).
    const fatal = consoleErrors.filter((line) => {
      return !/api\/management\//i.test(line) && !/fetch/i.test(line);
    });
    expect(fatal, fatal.join("\n")).toHaveLength(0);
  });

  test("renders chat route stub when no conversation id present", async ({
    page,
  }) => {
    await page.goto("/#/chat", { waitUntil: "domcontentloaded" });
    await expect(page.locator("body")).not.toBeEmpty();
  });
});
