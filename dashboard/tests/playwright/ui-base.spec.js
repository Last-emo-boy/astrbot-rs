import { expect, test } from "@playwright/test";

test("shared UI base renders modal, form, and state surfaces", async ({ page }) => {
  await page.goto("/tests/playwright/ui-base-fixture.html");

  await expect(page.getByRole("dialog", { name: "Unsaved changes" })).toBeVisible();
  await expect(page.getByText("The config form has local edits.")).toBeVisible();
  const modalScreenshot = await page.screenshot();
  expect(modalScreenshot.length).toBeGreaterThan(3_000);

  await page.getByRole("button", { name: "Stay" }).click();
  await expect(page.locator(".ui-dialog-backdrop")).toBeHidden();
  await expect(page.getByRole("tab", { name: "Form" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".ui-form-errors")).toBeVisible();
  await expect(page.locator(".ui-file-input")).toBeVisible();
  await expect(page.locator(".ui-data-table")).toBeVisible();
  await expect(page.locator(".ui-state.loading")).toBeVisible();
  await expect(page.locator(".ui-state.empty")).toBeVisible();
  await expect(page.locator(".ui-state.error")).toBeVisible();

  await page.getByRole("tab", { name: "Markdown" }).click();
  await expect(page.locator(".markdown-body")).toBeVisible();
  await expect(page.locator(".ui-code-block")).toBeVisible();

  const surfaceScreenshot = await page.screenshot({ fullPage: true });
  expect(surfaceScreenshot.length).toBeGreaterThan(3_000);
});
