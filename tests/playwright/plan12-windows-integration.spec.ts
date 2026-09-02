import { expect, test } from "@playwright/test";

test.describe("Plan 12 Windows integration browser contract", () => {
  test("renders settings controls, browser-preview overlay state, shortcuts, and output profiles", async ({ page }) => {
    await page.goto("/settings");
    const section = page.locator(".windows-integration-section");
    await expect(section.getByText("WINDOWS & OVERLAYS", { exact: true })).toBeVisible();
    await expect(section.getByText("Windows media controls", { exact: true })).toBeVisible();
    await expect(section.getByText("Desktop app only", { exact: true })).toBeVisible();

    const smtc = section.getByRole("checkbox", { name: "SMTC enabled" });
    await expect(smtc).toBeChecked();
    await smtc.uncheck();
    await expect(smtc).not.toBeChecked();

    const shortcuts = section.getByRole("checkbox", { name: "Global shortcuts enabled" });
    await expect(shortcuts).not.toBeChecked();
    await shortcuts.check();
    await expect(shortcuts).toBeChecked();
    await expect(section.getByLabel("Play / Pause accelerator")).toHaveValue("Ctrl+Alt+Space");
    await expect(section.getByRole("button", { name: "Restore shortcut defaults" })).toBeEnabled();

    const mini = section.getByRole("button", { name: /^Mini/ });
    await expect(mini).toContainText("Closed");
    await mini.click();
    await expect(mini).toContainText("Open");

    const gaming = section.getByRole("button", { name: /^Gaming/ });
    await gaming.click();
    const clickThrough = section.getByRole("checkbox", { name: "Gaming click-through" });
    await expect(clickThrough).toBeEnabled();
    await clickThrough.click();
    await expect(clickThrough).not.toBeChecked();
    await expect(section.getByRole("alert")).toContainText("native desktop app");

    await section.getByLabel("New output profile name").fill("Desk");
    await section.getByRole("button", { name: "Create from current output" }).click();
    await expect(section.getByText("Desk", { exact: true })).toBeVisible();
    await expect(section).toContainText("auto");
    await expect(section).toContainText("/ 16");

    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });

  test("keeps native-only command palette actions unavailable in browser preview", async ({ page }) => {
    await page.goto("/");
    await page.keyboard.press("Control+KeyK");
    const palette = page.locator('[aria-label="Command palette"]');
    await expect(palette).toBeVisible();
    await expect(palette.getByText("Toggle Mini Overlay", { exact: true })).toBeHidden();
    await expect(palette.getByText("Show SpotDIY", { exact: true })).toBeHidden();
  });
});
