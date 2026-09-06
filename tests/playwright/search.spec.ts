import { expect, test, type Page } from "@playwright/test";

const searchInput = (page: Page) => page.getByRole("textbox", { name: "Search music" });
const provider = (page: Page, kind: string) => page.locator(`[data-provider="${kind}"]`);

async function openSearch(page: Page) {
  await page.goto("/search");
  await expect(searchInput(page)).toBeVisible();
}

async function typeSearch(page: Page, query: string) {
  await searchInput(page).fill(query);
}

test.describe("provider search browser contract", () => {
  test("empty_search", async ({ page }) => {
    await openSearch(page);

    await expect(page.locator("[data-provider]")).toHaveCount(0);
  });

  test("independent_provider_loading", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "signal");

    await expect(provider(page, "local")).toHaveClass(/provider-result-state-loading/);
    await expect(provider(page, "youtube")).toHaveClass(/provider-result-state-loading/);
    await expect(provider(page, "soundcloud")).toHaveClass(/provider-result-state-loading/);
    await expect(provider(page, "spotify")).toHaveClass(/provider-result-state-loading/);
  });

  test("local_before_youtube", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "signal");

    await expect(provider(page, "youtube").locator(".provider-result-loading")).toBeVisible();
    await expect(provider(page, "local").locator(".search-result-card").first()).toBeVisible();
  });

  test("youtube_completion", async ({ page }) => {
    await openSearch(page);
    await page.getByRole("tab", { name: "YOUTUBE" }).click();
    await typeSearch(page, "signal");

    const youtube = provider(page, "youtube");
    await expect(youtube.locator(".search-result-card").first()).toBeVisible();
    await expect(youtube.locator(".search-result-heading strong")).toContainText("YouTube");
    await expect(youtube.getByRole("button", { name: "Open source" })).toBeVisible();
    const downloadMode = youtube.getByRole("combobox", { name: /Download mode for/ });
    await expect(downloadMode).toHaveCount(1);
    await expect(downloadMode.locator("option")).toHaveText(["Audio", "Video"]);
    await expect(youtube.getByRole("button", { name: "Download", exact: true })).toBeDisabled();
  });

  test("soundcloud_audio_only_actions", async ({ page }) => {
    await openSearch(page);
    await page.getByRole("tab", { name: "SOUNDCLOUD" }).click();
    await typeSearch(page, "soundcloud download fixture");

    const soundcloud = provider(page, "soundcloud");
    await expect(soundcloud.locator(".search-result-card").first()).toBeVisible();
    await expect(soundcloud.getByRole("button", { name: "Open source" })).toBeVisible();
    await expect(soundcloud.getByRole("combobox", { name: /Download mode for/ })).toHaveCount(0);
    await expect(soundcloud.getByRole("button", { name: "Download audio" })).toBeDisabled();
  });

  test("soundcloud_error", async ({ page }) => {
    await openSearch(page);
    await page.getByRole("tab", { name: "SOUNDCLOUD" }).click();
    await typeSearch(page, "signal");

    const soundcloud = provider(page, "soundcloud");
    await expect(soundcloud).toHaveClass(/provider-result-state-failed/);
    await expect(soundcloud).toContainText("Synthetic partial-provider failure");
  });

  test("partial_results_remain_usable", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "signal");

    await expect(provider(page, "local").locator(".search-result-card").first()).toBeVisible();
    await expect(provider(page, "youtube").getByRole("button", { name: "Open source" })).toBeVisible();
    await expect(provider(page, "soundcloud")).toContainText("Synthetic partial-provider failure");
  });

  test("sort_interaction", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "signal");

    const sort = page.getByRole("combobox", { name: "Sort search results" });
    await sort.selectOption("duration");
    await expect(sort).toHaveValue("duration");
    await expect(provider(page, "local").locator(".search-result-card").first()).toBeVisible();
  });

  test("lens_switching", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "signal");
    await expect(provider(page, "local").locator(".search-result-card").first()).toBeVisible();

    await page.getByRole("tab", { name: "ARTISTS" }).click();
    await expect(page.locator('[data-provider="local"]')).toHaveCount(1);
    await expect(page.locator('[data-provider="youtube"]')).toHaveCount(0);
    await expect(page.getByRole("tab", { name: "ARTISTS" })).toHaveAttribute("aria-selected", "true");

    await page.getByRole("tab", { name: "YOUTUBE" }).click();
    await expect(page.locator('[data-provider="local"]')).toHaveCount(0);
    await expect(page.locator('[data-provider="youtube"]')).toHaveCount(1);
  });

  test("stale_event_ignored", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "old signal");
    await expect(provider(page, "local").locator(".search-result-card").first()).toBeVisible();

    await typeSearch(page, "new signal");
    await expect(page.getByRole("heading", { name: /“new signal”/ })).toBeVisible();
    await expect(provider(page, "local").locator(".search-result-card").first()).toBeVisible();
    await expect(page.getByText(/old signal/)).toHaveCount(0);
  });

  test("spotify_search_and_audio_download_action", async ({ page }) => {
    await openSearch(page);
    await page.getByRole("tab", { name: "SPOTIFY" }).click();
    await typeSearch(page, "signal");

    const spotify = provider(page, "spotify");
    await expect(spotify).toHaveClass(/provider-result-state-ready/);
    await expect(spotify.locator(".search-result-card").first()).toBeVisible();
    await expect(spotify.getByRole("button", { name: "Open on Spotify" })).toBeVisible();
    await expect(spotify.getByRole("button", { name: "Download audio" })).toBeDisabled();
    await expect(spotify.getByRole("button", { name: /Play/ })).toHaveCount(0);
  });

  test("long_title_overflow", async ({ page }) => {
    await openSearch(page);
    await page.getByRole("tab", { name: "YOUTUBE" }).click();
    const longQuery = "Very long title ".repeat(16).trim();
    await typeSearch(page, longQuery);

    const title = provider(page, "youtube").locator(".search-result-heading strong").first();
    await expect(title).toBeVisible();
    await expect(title).toHaveAttribute("title", /YouTube$/);
    await expect.poll(async () => title.evaluate((element) => {
      const style = window.getComputedStyle(element);
      return style.overflow === "hidden" && style.textOverflow === "ellipsis";
    })).toBe(true);
  });

  test("provider rows do not duplicate source artwork or badges", async ({ page }) => {
    await openSearch(page);
    await typeSearch(page, "signal");

    const local = provider(page, "local");
    await expect(local.locator(".search-result-art")).toHaveCount(0);
    await expect(local.locator(".search-result-card .provider-badge")).toHaveCount(0);
  });

  test("keeps the compact player lyrics action reachable", async ({ page }) => {
    await openSearch(page);

    const lyricsLink = page.getByRole("link", { name: "Open lyrics" });
    await expect(lyricsLink).toHaveClass(/icon-only-button/);
    await lyricsLink.click();

    await expect(page).toHaveURL(/\/lyrics$/);
    await expect(page.locator(".lyrics-page")).toBeVisible();
    await expect(page.getByText("LYRICS & NOTES", { exact: true })).toBeVisible();
  });
});
