import { expect, test, type Page } from "@playwright/test";

const longTitle = "Night Drive - Neon Over Water (Extended Live Session)";

const customTheme = {
  schemaVersion: 1,
  name: "E2E Aurora",
  baseMode: "dark",
  tokens: {
    background: "#101113",
    surface: "#17181D",
    surfaceRaised: "#1D1E24",
    surfaceSoft: "#22232A",
    text: "#F3F1EC",
    textMuted: "#A8A7AE",
    textSubtle: "#807F87",
    border: "#2E2F36",
    borderStrong: "#4B4C55",
    accent: "#D7FF60",
    accentContrast: "#151617",
    success: "#81E2D0",
    warning: "#FFB570",
    danger: "#FF806F",
    info: "#8E7BFF",
  },
};

async function assertNoHorizontalOverflow(page: Page) {
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
}

async function openSettings(page: Page) {
  await page.goto("/settings");
  await expect(page.getByRole("heading", { name: /Make it yours/ })).toBeVisible();
  await expect(page.getByText("APPEARANCE", { exact: true })).toBeVisible();
}

test.describe("design system browser contract", () => {
  test("covers themes, density, focus, context actions, reduced motion, and viewport safety", async ({ page }, testInfo) => {
    const consoleErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") {
        consoleErrors.push(message.text());
      }
    });
    page.on("pageerror", (error) => consoleErrors.push(error.message));

    await openSettings(page);
    const root = page.locator("html");
    await expect(root).toHaveAttribute("data-theme", "dark");
    await expect(root).toHaveAttribute("data-layout", "comfortable");
    await expect(page.getByRole("radio", { name: "Custom" })).toBeDisabled();
    await page.screenshot({ path: testInfo.outputPath(`settings-appearance-${testInfo.project.name}.png`) });

    const darkFocus = page.getByRole("radio", { name: "Dark" });
    await darkFocus.focus();
    await expect.poll(() => darkFocus.evaluate((element) => getComputedStyle(element).outlineWidth)).toBe("2px");

    await page.goto("/");
    await expect(page.getByRole("heading", { name: /Make room for listening/ })).toBeVisible();
    await assertNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath(`home-dark-${testInfo.project.name}.png`) });

    await openSettings(page);
    await page.getByRole("radio", { name: "Light" }).click();
    await expect(root).toHaveAttribute("data-theme", "light");
    await expect(page.getByText("Light surfaces", { exact: true })).toBeVisible();
    await page.getByRole("link", { name: "Home", exact: true }).click();
    await expect(root).toHaveAttribute("data-theme", "light");
    await assertNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath(`home-light-${testInfo.project.name}.png`) });

    await openSettings(page);
    await page.getByRole("radio", { name: "Dark" }).click();
    await expect(root).toHaveAttribute("data-theme", "dark");

    const importInput = page.getByLabel("Import custom theme JSON");
    const invalidTheme = { ...customTheme, tokens: { ...customTheme.tokens, text: "#111111" } };
    await importInput.setInputFiles({
      name: "invalid.json",
      mimeType: "application/json",
      buffer: Buffer.from(JSON.stringify(invalidTheme)),
    });
    await expect(page.getByRole("alert").filter({ hasText: "Theme validation failed" })).toBeVisible();
    await expect(root).toHaveAttribute("data-theme", "dark");

    await importInput.setInputFiles({
      name: "aurora.json",
      mimeType: "application/json",
      buffer: Buffer.from(JSON.stringify(customTheme)),
    });
    await expect(root).toHaveAttribute("data-theme", "custom");
    await expect(page.getByText("E2E Aurora", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Export JSON" })).toBeEnabled();
    await page.getByRole("button", { name: "Export JSON" }).click();
    await expect(page.getByRole("textbox", { name: "Exported custom theme JSON" })).toHaveValue(/E2E Aurora/);
    await page.getByRole("button", { name: "Reset" }).click();
    await expect(root).toHaveAttribute("data-theme", "dark");
    await expect(page.getByText("No custom theme imported", { exact: true })).toBeVisible();

    await page.getByRole("radio", { name: "Dense" }).click();
    await expect(root).toHaveAttribute("data-layout", "dense");
    await page.getByRole("link", { name: "Your library", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Your collection, in focus." })).toBeVisible();
    await expect(page.getByText(longTitle, { exact: true }).first()).toBeVisible();
    await assertNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath(`library-dense-${testInfo.project.name}.png`) });

    const contextAnchor = page.locator(".library-track-context-menu").first();
    await contextAnchor.focus();
    await page.keyboard.press("Shift+F10");
    const menu = page.getByRole("menu", { name: "Context actions" });
    await expect(menu).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Play now" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("End");
    await page.keyboard.press("Home");
    await expect(page.getByRole("menuitem", { name: "Play now" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(contextAnchor).toBeFocused();

    await contextAnchor.click({ button: "right" });
    await expect(menu).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(contextAnchor).toBeFocused();

    await openSettings(page);
    await page.getByRole("radio", { name: "Comfortable" }).click();
    await expect(root).toHaveAttribute("data-layout", "comfortable");
    await page.getByRole("link", { name: "Lyrics & notes", exact: true }).click();
    await expect(page.getByRole("heading", { name: /Stay with the words/ })).toBeVisible();
    await assertNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath(`lyrics-comfortable-${testInfo.project.name}.png`) });

    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.getByRole("link", { name: "Home", exact: true }).click();
    const reducedMotion = await page.locator(".button-primary").first().evaluate((element) => ({
      scrollBehavior: getComputedStyle(document.documentElement).scrollBehavior,
      transitionDuration: getComputedStyle(element).transitionDuration,
    }));
    expect(reducedMotion.scrollBehavior).toBe("auto");
    expect(Number.parseFloat(reducedMotion.transitionDuration)).toBeLessThanOrEqual(0.01);
    await assertNoHorizontalOverflow(page);

    const projectViewport = page.viewportSize();
    expect(projectViewport).not.toBeNull();
    await page.setViewportSize({ width: 1080, height: 720 });
    await page.goto("/settings");
    await assertNoHorizontalOverflow(page);
    await page.setViewportSize(projectViewport!);

    expect(consoleErrors).toEqual([]);
  });
});
