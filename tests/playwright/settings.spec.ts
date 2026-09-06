import { expect, test } from "@playwright/test";

test.describe("runtime settings browser contract", () => {
  test("presents Spotify spotdl readiness and missing MPV actions", async ({ page }) => {
    await page.goto("/settings");
    await expect(page.getByRole("heading", { name: "Make it yours." })).toBeVisible();

    await expect(page.getByRole("switch", { name: "Enable Spotify catalog" })).toHaveCount(0);
    const spotifyRow = page.locator(".settings-source-row").filter({ hasText: "Spotify" });
    await expect(spotifyRow).toContainText("Ready");
    await expect(spotifyRow).toContainText("spotdl is ready");
    await expect(page.getByText(/client ID|PKCE|Spotify market|Enable Spotify catalog/i)).toHaveCount(0);

    const mpvRow = page.locator(".settings-tool-row").filter({ hasText: "MPV" });
    await expect(mpvRow).toContainText("Missing");
    await expect(mpvRow.getByRole("button", { name: "Configure…" })).toBeDisabled();
    await expect(mpvRow.getByRole("button", { name: "Re-scan" })).toBeDisabled();
  });
});
