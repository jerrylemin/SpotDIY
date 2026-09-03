import { expect, test, type Page } from "@playwright/test";

const longTitle = "Night Drive - Neon Over Water (Extended Live Session)";

async function assertNoHorizontalOverflow(page: Page) {
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
}

test("Plan 15 visual routes work at the targeted ultrawide viewport", async ({ page }, testInfo) => {
  const consoleErrors: string[] = [];
  page.on("console", (message) => { if (message.type() === "error") consoleErrors.push(message.text()); });
  page.on("pageerror", (error) => consoleErrors.push(error.message));
  await page.emulateMedia({ reducedMotion: "reduce" });

  await page.goto("/music-map");
  await expect(page.getByRole("heading", { name: "See the connections." })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Map Navigator" })).toBeVisible();
  await expect(page.locator(".music-map-svg")).toBeVisible();
  await page.locator(".visual-node-list-item").filter({ hasText: longTitle }).first().click();
  await expect(page.getByText("SELECTED TRACK", { exact: true })).toBeVisible();
  const actions = page.locator(".visual-track-actions");
  await actions.focus();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menu", { name: "Radial track actions" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(actions).toBeFocused();

  await page.getByRole("button", { name: "Preview", exact: true }).click();
  await expect(page.locator(".visual-preview-status")).toBeVisible();
  await page.getByRole("button", { name: "Cancel Preview", exact: true }).click();
  await expect(page.locator(".visual-preview-status")).toHaveCount(0);
  await page.getByRole("button", { name: "Preview", exact: true }).click();
  await expect(page.locator(".visual-preview-status")).toBeVisible();
  await actions.getByRole("button", { name: "Play", exact: true }).click();
  await expect(page.locator(".visual-preview-status")).toHaveCount(0, { timeout: 3_000 });

  await page.goto("/library-galaxy");
  await expect(page.getByRole("heading", { name: "Library Galaxy" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Galaxy Navigator" })).toBeVisible();
  await expect(page.locator("canvas[aria-label='Library Galaxy canvas']")).toBeVisible();
  await page.locator(".visual-node-list-item").filter({ hasText: longTitle }).first().click();
  await expect(page.getByText("SELECTED TRACK", { exact: true })).toBeVisible();

  await page.goto("/theme-studio");
  await expect(page.getByRole("heading", { name: "Make a place to listen." })).toBeVisible();
  await expect(page.getByText("Schema v1 · 15 tokens", { exact: true })).toBeVisible();
  await expect(page.locator(".theme-token-field")).toHaveCount(15);
  await page.getByRole("button", { name: "Preview on App", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "custom");
  await page.getByRole("button", { name: "Stop App Preview", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: /Dense/ }).click();
  await expect(page.locator("html")).toHaveAttribute("data-layout", "dense");

  for (const route of ["/music-map", "/library-galaxy", "/theme-studio"]) {
    await page.goto(route);
    await assertNoHorizontalOverflow(page);
    await expect(page.getByText(/coming soon/i)).toHaveCount(0);
  }
  await page.screenshot({ path: testInfo.outputPath("plan15-ultrawide-theme-studio.png"), fullPage: true });
  expect(consoleErrors).toEqual([]);
});
