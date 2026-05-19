import { expect, test } from "@playwright/test";

test("shared ui base renders modal form table and state screenshots", async ({ page }) => {
  await page.goto("/chatbox/shared-ui");

  await page.evaluate(async () => {
    const { renderUiBaseShowcase } = await import("/src/render/shared.js");
    document.body.dataset.layout = "full";
    const content = document.querySelector("#content");
    content.className = "content shared-preview-host";
    content.innerHTML = renderUiBaseShowcase();
  });

  await expect(page.locator('[role="dialog"]')).toBeVisible();
  await expect(page.locator('[role="tablist"]')).toBeVisible();
  await expect(page.locator(".ui-form")).toBeVisible();
  await expect(page.locator(".ui-data-table")).toBeVisible();
  await expect(page.locator(".ui-state.loading")).toBeVisible();
  await expect(page.locator(".ui-state.empty")).toBeVisible();
  await expect(page.locator(".ui-state.error")).toBeVisible();

  const dialogBox = await page.locator('[role="dialog"]').boundingBox();
  expect(dialogBox.width).toBeGreaterThan(300);
  expect(dialogBox.height).toBeGreaterThan(140);

  const screenshot = await page.screenshot({ fullPage: true });
  expect(screenshot.length).toBeGreaterThan(4_000);
});
