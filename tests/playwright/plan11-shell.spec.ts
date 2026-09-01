import { expect, test } from "@playwright/test";

const longTitle = "Night Drive - Neon Over Water (Extended Live Session)";

test.describe("Plan 11 shell surfaces", () => {
  test("renders the populated home dashboard from live preview data", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("YOUR MUSIC, YOUR MACHINE", { exact: true })).toBeVisible();
    await expect(page.locator(".home-dashboard-grid").getByText("LIBRARY", { exact: true })).toBeVisible();
    await expect(page.getByText("START HERE", { exact: true })).toBeHidden();
  });

  test("opens a persisted track inspector without exposing local paths", async ({ page }) => {
    await page.goto("/library");
    await expect(page.getByText(longTitle, { exact: true }).first()).toBeVisible();
    await page.getByRole("button", { name: `Inspect ${longTitle}` }).click();

    const inspector = page.getByRole("dialog");
    await expect(inspector.getByText("PERSISTED LOCAL TRACK", { exact: true })).toBeVisible();
    await expect(inspector.getByText("SOURCES", { exact: true })).toBeVisible();
    await expect(inspector.getByText("CAPABILITIES", { exact: true })).toBeVisible();
    await expect(inspector).not.toContainText("C:\\Synthetic Music");

    await page.keyboard.press("Escape");
    await expect(inspector).toBeHidden();
  });

  test("keeps online search inspection ephemeral and capability-aware", async ({ page }) => {
    await page.goto("/search");
    await page.getByRole("tab", { name: "YOUTUBE" }).click();
    await page.getByRole("textbox", { name: "Search music" }).fill("signal");
    const youtube = page.locator('[data-provider="youtube"]');
    await expect(youtube.locator(".search-result-card").first()).toBeVisible();
    await youtube.getByRole("button", { name: "Inspect" }).first().click();

    const inspector = page.getByRole("dialog");
    await expect(inspector.getByText("NOT IN LOCAL LIBRARY", { exact: true })).toBeVisible();
    await expect(inspector.getByText("EPHEMERAL SEARCH RESULT", { exact: true })).toBeVisible();
    await expect(inspector.getByText(/Online playback is not implemented/)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(inspector).toBeHidden();
  });

  test("switches standard, mini, and expanded player modes from the shell", async ({ page }) => {
    await page.goto("/library");
    await page.getByRole("button", { name: /Play now Night Drive/ }).click();
    await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Playback source" }).first()).toBeEnabled();
    await expect(page.getByRole("button", { name: "Open mini player" })).toBeVisible();
    await page.getByRole("button", { name: "Open mini player" }).click();
    await expect(page.getByRole("contentinfo", { name: "Mini now playing" })).toBeVisible();
    await page.getByRole("button", { name: "Open expanded now playing" }).click();
    await expect(page.getByRole("dialog", { name: "Expanded now playing" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Expanded now playing" })).toBeHidden();
    await expect(page.getByRole("button", { name: "Open mini player" })).toBeVisible();
  });
});
