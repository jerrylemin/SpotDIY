/* global console, process, window */

import { chromium } from "@playwright/test";

const mode = process.argv[2] ?? "flow";
const cdpUrl = process.env.SPOTDIY_PACKAGED_CDP_URL;
const fixtureFolder = process.env.SPOTDIY_PACKAGED_FIXTURE;

if (!cdpUrl) {
  throw new Error("SPOTDIY_PACKAGED_CDP_URL is required");
}
if (mode === "flow" && !fixtureFolder) {
  throw new Error("SPOTDIY_PACKAGED_FIXTURE is required for the playback flow");
}

const browser = await chromium.connectOverCDP(cdpUrl);
try {
  const pages = browser.contexts().flatMap((context) => context.pages());
  const page = pages[0];
  if (!page) {
    throw new Error("the packaged app did not expose a WebView page");
  }

  async function invoke(command, args = undefined) {
    return page.evaluate(async ({ command: commandName, args: commandArgs }) => {
      const internals = window.__TAURI_INTERNALS__;
      if (!internals || typeof internals.invoke !== "function") {
        throw new Error("the packaged page does not expose the Tauri invoke bridge");
      }
      return internals.invoke(commandName, commandArgs);
    }, { command, args });
  }

  async function waitFor(description, predicate, timeoutMs = 20_000) {
    const deadline = Date.now() + timeoutMs;
    let lastValue;
    while (Date.now() < deadline) {
      lastValue = await predicate();
      if (lastValue) {
        return lastValue;
      }
      await page.waitForTimeout(250);
    }
    throw new Error(`timed out waiting for ${description}; last value: ${JSON.stringify(lastValue)}`);
  }

  await page.waitForLoadState("domcontentloaded");
  await page.getByRole("link", { name: "Your library", exact: true }).waitFor({ state: "visible", timeout: 20_000 });

  if (mode === "flow") {
    await invoke("add_library_folders", { paths: [fixtureFolder] });
    let lastScanStatus = null;
    try {
      await waitFor("the synthetic folder scan", async () => {
        const status = await invoke("get_library_status");
        lastScanStatus = status;
        return status.folders?.length === 1 && status.indexedTrackCount >= 2 && !status.isScanning ? status : false;
      });
    } catch (error) {
      throw new Error(
        `${error instanceof Error ? error.message : String(error)}; last scan status: ${JSON.stringify(lastScanStatus)}`,
        { cause: error },
      );
    }

    await page.getByRole("link", { name: "Your library", exact: true }).click();
    await page.getByRole("heading", { name: "Your local collection" }).waitFor({ state: "visible" });
    const rows = page.locator("[data-testid^='library-track-']");
    await waitFor("two indexed synthetic tracks", async () => (await rows.count()) === 2);

    const firstRow = rows.nth(0);
    const secondRow = rows.nth(1);
    await firstRow.getByRole("button", { name: /Play now/ }).click();
    const firstSnapshot = await waitFor("Playing state", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    const firstPosition = firstSnapshot.positionMs;
    await page.waitForTimeout(1_000);
    const advancedPosition = (await invoke("get_playback_snapshot")).positionMs;
    if (advancedPosition <= firstPosition) {
      throw new Error(`position did not advance (${firstPosition} -> ${advancedPosition})`);
    }

    await page.getByRole("button", { name: "Pause" }).click();
    await waitFor("Paused state", async () => (await invoke("get_playback_snapshot")).phase === "paused");

    const seek = page.getByRole("slider", { name: "Seek within current track" });
    await seek.fill("1000");
    await seek.blur();
    await waitFor("seek position", async () => {
      const position = (await invoke("get_playback_snapshot")).positionMs;
      return position >= 500 && position <= 2_000 ? position : false;
    });

    await page.getByRole("button", { name: "Play", exact: true }).click();
    await waitFor("resumed Playing state", async () => (await invoke("get_playback_snapshot")).phase === "playing");

    await secondRow.getByRole("button", { name: /Add .* to queue/ }).click();
    await waitFor("the queued second track", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.queueLength === 2 ? snapshot : false;
    });

    await page.getByRole("button", { name: "Next track" }).click();
    await waitFor("the second track after Next", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.currentTrackId && snapshot.phase === "playing" && snapshot.queueLength === 2 && snapshot.currentQueueEntryId && snapshot.currentTrackId !== firstSnapshot.currentTrackId ? snapshot : false;
    });
    console.log("packaged playback flow passed");
  } else if (mode === "restart") {
    const status = await invoke("get_library_status");
    if (status.folders?.length !== 1 || status.indexedTrackCount < 2) {
      throw new Error("the indexed local library was not retained across restart");
    }
    const snapshot = await invoke("get_playback_snapshot");
    if (snapshot.queueLength !== 0 || snapshot.currentTrackId !== null) {
      throw new Error(`the transient playback queue was persisted: ${JSON.stringify(snapshot)}`);
    }
    console.log("packaged restart persistence boundary passed");
  } else {
    throw new Error(`unknown smoke mode: ${mode}`);
  }
} finally {
  await browser.close();
}
