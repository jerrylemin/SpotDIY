import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

const routes = [
  "/",
  "/search",
  "/library",
  "/playlists",
  "/downloads",
  "/lyrics",
  "/analytics",
  "/settings",
  "/music-map",
  "/library-galaxy",
  "/theme-studio",
];

test.describe("Plan 16 accessibility contract", () => {
  test.describe.configure({ timeout: 120_000 });

  test("has no serious or critical axe violations on representative routes", async ({ page }) => {
    for (const route of routes) {
      await page.goto(route);
      await expect(page.locator("main")).toBeVisible();
      const results = await new AxeBuilder({ page }).analyze();
      const blocking = results.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical");
      expect(blocking, `${route} accessibility violations`).toEqual([]);
    }
  });

  test("keeps keyboard navigation, focus restoration, and reduced motion usable", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/");
    await expect(page.getByRole("navigation", { name: "Primary navigation" })).toBeVisible();
    await page.keyboard.press("Control+KeyK");
    await expect(page.getByRole("region", { name: "Command palette" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("region", { name: "Command palette" })).toBeHidden();

    const homeLink = page.getByRole("navigation", { name: "Primary navigation" }).getByRole("link", { name: "Home", exact: true });
    await homeLink.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { name: /Make room for listening/ })).toBeVisible();

    await page.goto("/library");
    const contextAnchor = page.locator(".library-track-context-menu").first();
    await contextAnchor.focus();
    await page.keyboard.press("Shift+F10");
    await expect(page.getByRole("menu", { name: "Context actions" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(contextAnchor).toBeFocused();
    await page.getByRole("button", { name: "Open queue" }).click();
    await expect(page.getByRole("dialog", { name: "Persistent queue" })).toBeVisible();
    await page.getByRole("dialog", { name: "Persistent queue" }).getByRole("button", { name: "Close queue" }).click();
    await page.getByRole("button", { name: `Inspect Night Drive - Neon Over Water (Extended Live Session)` }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toBeHidden();

    await page.goto("/music-map");
    const mapNode = page.locator(".visual-node-list-item").filter({ hasText: "Night Drive - Neon Over Water (Extended Live Session)" }).first();
    await mapNode.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByText("SELECTED TRACK", { exact: true })).toBeVisible();
    const visualActions = page.locator(".visual-track-actions");
    await expect(visualActions).toBeVisible();
    await visualActions.focus();
    await expect(visualActions).toBeFocused();
    await page.keyboard.press("Shift+F10");
    await expect(page.getByRole("menu", { name: "Radial track actions" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(visualActions).toBeFocused();

    await page.goto("/library-galaxy");
    const galaxyNode = page.locator(".visual-node-list-item").first();
    await galaxyNode.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByText("SELECTED TRACK", { exact: true })).toBeVisible();

    await page.goto("/theme-studio");
    await page.getByRole("button", { name: "Preview on App", exact: true }).focus();
    await expect.poll(() => page.evaluate(() => document.activeElement?.matches(":focus-visible"))).toBe(true);
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });
});
