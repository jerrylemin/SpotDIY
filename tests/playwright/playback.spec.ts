import { expect, test, type Page } from "@playwright/test";

const longTitle = "Night Drive - Neon Over Water (Extended Live Session)";

async function openLibrary(page: Page, scenario = "default") {
  await page.goto(`/library?playbackScenario=${scenario}`);
  await expect(page.getByRole("heading", { name: "Your collection, in focus." })).toBeVisible();
  await expect(page.getByText(longTitle, { exact: true }).first()).toBeVisible();
}

test.describe("playback engine browser contract", () => {
  test("covers idle, transport, queue, controls, accessibility, and visuals", async ({ page }, testInfo) => {
    const consoleErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") {
        consoleErrors.push(message.text());
      }
    });
    page.on("pageerror", (error) => consoleErrors.push(error.message));

    await openLibrary(page);

    const playButton = page.getByRole("button", { name: "Play", exact: true });
    await expect(page.getByText("Nothing queued", { exact: true }).first()).toBeVisible();
    await expect(playButton).toBeDisabled();
    await expect(page.getByRole("button", { name: "Previous track" })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Next track" })).toBeDisabled();
    await expect(page.locator(".player-art-placeholder img")).toHaveCount(0);
    await expect(page.getByText("LOCAL", { exact: true }).first()).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath(`idle-${testInfo.project.name}.png`) });

    await page.getByRole("button", { name: /Play now Night Drive/ }).click();
    await expect(page.getByText("Loading track", { exact: true }).or(page.getByText("Now playing", { exact: true }))).toBeVisible();
    await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeVisible();
    await expect(page.getByText("Now playing", { exact: true })).toBeVisible();
    await expect(page.getByTitle(longTitle).first()).toBeVisible();

    const progress = page.getByRole("slider", { name: "Seek within current track" });
    await progress.fill("42000");
    await progress.blur();
    await expect(page.getByText("0:42", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await expect(page.getByText("Paused", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Play", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Play", exact: true }).click();
    await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeVisible();

    const volume = page.getByRole("slider", { name: "Playback volume" });
    await volume.fill("42");
    await volume.blur();
    await expect(page.getByText("42%", { exact: true })).toBeVisible();
    const mute = page.getByRole("button", { name: "Mute", exact: true });
    await mute.click();
    await expect(page.getByRole("button", { name: "Unmute", exact: true })).toHaveAttribute("aria-pressed", "true");

    await page.getByRole("button", { name: /Play next Static Bloom/ }).click();
    await expect(page.getByText("Queue 1 of 2", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: /Add Static Bloom to queue/ }).click();
    await expect(page.getByText("Queue 1 of 3", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Next track", exact: true }).click();
    await expect(page.getByTestId("library-track-track-e2e-2")).toHaveClass(/library-track-current/);
    await expect(page.getByText("Now playing", { exact: true })).toBeVisible();

    const shuffle = page.getByRole("button", { name: "Toggle shuffle", exact: true });
    await expect(shuffle).toBeEnabled();
    await shuffle.click();
    await expect(shuffle).toHaveAttribute("aria-pressed", "true");

    const repeat = page.getByRole("button", { name: "Repeat off", exact: true });
    await repeat.click();
    await expect(page.getByRole("button", { name: "Repeat one", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Repeat one", exact: true }).click();
    await expect(page.getByRole("button", { name: "Repeat all", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Repeat all", exact: true }).click();
    await expect(page.getByRole("button", { name: "Repeat off", exact: true })).toBeVisible();

    const deviceMenu = page.getByRole("combobox", { name: "Playback audio device" });
    await deviceMenu.focus();
    await expect(deviceMenu.locator("option", { hasText: "USB Headphones" })).toHaveCount(1);
    await deviceMenu.selectOption("headphones");
    await expect(deviceMenu).toHaveValue("headphones");

    await page.keyboard.press("Control+KeyK");
    const commandPalette = page.locator('[aria-label="Command palette"]');
    await expect(commandPalette).toBeVisible();
    await page.getByRole("textbox", { name: "Search commands" }).fill("Next track");
    await expect(page.getByRole("button", { name: "Next track", exact: true })).toBeEnabled();
    await page.keyboard.press("Escape");
    await expect(commandPalette).toBeHidden();

    await page.screenshot({ path: testInfo.outputPath(`playing-${testInfo.project.name}.png`) });
    expect(consoleErrors).toEqual([]);
  });

  test("renders missing-tool, recovering, failed, and retry states", async ({ page }) => {
    await openLibrary(page, "toolMissing");
    await expect(page.getByText("Player engine unavailable", { exact: true })).toBeVisible();
    await expect(page.getByText(/Install mpv to play local music/)).toBeVisible();
    const retryButton = page.getByRole("button", { name: "Retry Player Engine", exact: true });
    await expect(retryButton).toBeEnabled();
    await retryButton.click();
    await expect(page.getByText("Nothing queued", { exact: true }).first()).toBeVisible();

    await openLibrary(page, "recovering");
    await expect(page.getByText("Recovering playback", { exact: true })).toBeVisible();
    await expect(page.getByText(/Reconnecting to the playback backend/)).toBeVisible();
    await page.getByRole("button", { name: "Retry Player Engine", exact: true }).click();
    await expect(page.getByText("Paused", { exact: true })).toBeVisible();

    await openLibrary(page, "failed");
    await expect(page.getByText("Playback unavailable", { exact: true })).toBeVisible();
    await expect(page.getByText("Playback recovery is exhausted.", { exact: true }).first()).toBeVisible();
    await page.getByRole("button", { name: "Retry Player Engine", exact: true }).click();
    await expect(page.getByText("Paused", { exact: true })).toBeVisible();
  });

  test("preserves queue identity while switching the current E2E playback source", async ({ page }) => {
    await openLibrary(page);

    const states = await page.evaluate(async () => {
      const ipc = await import("/src/services/ipc.ts");
      const trackRequest = (trackId: string, sourceId: string) => ({
        trackId: trackId as Parameters<typeof ipc.playTrack>[0]["trackId"],
        sourceId: sourceId as Parameters<typeof ipc.playTrack>[0]["sourceId"],
      });

      await ipc.playTrack(trackRequest("track-e2e-1", "source-e2e-1"));
      await ipc.enqueueTrack(trackRequest("track-e2e-2", "source-e2e-2"));
      await ipc.enqueueTrack(trackRequest("track-e2e-1", "source-e2e-1"));
      await ipc.nextTrack();
      await new Promise((resolve) => window.setTimeout(resolve, 160));
      const before = await ipc.getPlaybackSnapshot();

      await ipc.switchPlaybackSource({
        trackId: "track-e2e-2" as Parameters<typeof ipc.switchPlaybackSource>[0]["trackId"],
        sourceId: "source-e2e-2-alternate" as Parameters<typeof ipc.switchPlaybackSource>[0]["sourceId"],
      });
      await new Promise((resolve) => window.setTimeout(resolve, 160));
      const after = await ipc.getPlaybackSnapshot();
      return { before, after };
    });

    expect(states.after.queueLength).toBe(states.before.queueLength);
    expect(states.after.queueIndex).toBe(states.before.queueIndex);
    expect(states.after.currentQueueEntryId).toBe(states.before.currentQueueEntryId);
    expect(states.after.currentTrackId).toBe(states.before.currentTrackId);
    expect(states.after.currentSourceId).toBe("source-e2e-2-alternate");
    expect(states.after.currentSourceId).not.toBe(states.before.currentSourceId);
  });

  test("opens the queue workspace with section priorities and the Autoplay empty state", async ({ page }) => {
    await openLibrary(page);
    await page.getByRole("button", { name: /Play now Night Drive/ }).click();
    await expect(page.getByRole("button", { name: "Open queue" })).toBeVisible();

    await page.getByRole("button", { name: "Open queue" }).click();
    await expect(page.getByRole("dialog", { name: "Persistent queue" })).toBeVisible();
    await expect(page.getByText("CURRENT", { exact: true })).toBeVisible();
    await expect(page.getByText("UP NEXT", { exact: true })).toBeVisible();
    await expect(page.getByText("LATER", { exact: true })).toBeVisible();
    await expect(page.getByText("AUTOPLAY", { exact: true })).toBeVisible();
    await expect(page.getByText("Autoplay recommendations are not enabled yet.", { exact: true })).toBeVisible();
    await expect(page.getByText("Queue editing and snapshots are available in the native SpotDIY app.", { exact: true })).toBeVisible();
  });
});
