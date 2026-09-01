/* global console, process, window */

import { chromium } from "@playwright/test";

const mode = process.argv[2] ?? "flow";
const cdpUrl = process.env.SPOTDIY_PACKAGED_CDP_URL;
const fixtureFolder = process.env.SPOTDIY_PACKAGED_FIXTURE;

if (!cdpUrl) {
  throw new Error("SPOTDIY_PACKAGED_CDP_URL is required");
}
if ((mode === "flow" || mode === "plan08") && !fixtureFolder) {
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
    if (snapshot.phase !== "idle" || snapshot.queueLength !== 2 || !snapshot.currentQueueEntryId || !snapshot.currentTrackId) {
      throw new Error(`the persistent playback queue was not restored without autoplay: ${JSON.stringify(snapshot)}`);
    }
    console.log("packaged restart persistent queue boundary passed");
  } else if (mode === "plan08") {
    await invoke("add_library_folders", { paths: [fixtureFolder] });
    await waitFor("the synthetic folder scan", async () => {
      const status = await invoke("get_library_status");
      return status.folders?.length === 1 && status.indexedTrackCount >= 2 && !status.isScanning ? status : false;
    });

    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 100, sort: "title", descending: false, folderId: null },
    });
    const tracks = [...(libraryPage.items ?? [])].sort((left, right) => left.title.localeCompare(right.title));
    if (tracks.length < 2 || !tracks[0].trackId || !tracks[0].sourceId || !tracks[1].trackId || !tracks[1].sourceId) {
      throw new Error(`the packaged Plan 08 fixture did not expose two playable tracks: ${JSON.stringify(libraryPage)}`);
    }
    const first = tracks[0];
    const second = tracks[1];

    const playlist = await invoke("create_playlist", { name: "Plan 08 Smoke" });
    await invoke("add_playlist_item", {
      playlistId: playlist.id,
      trackId: first.trackId,
      requestedSourceId: first.sourceId,
    });
    await invoke("add_playlist_item", {
      playlistId: playlist.id,
      trackId: second.trackId,
      requestedSourceId: second.sourceId,
    });
    await invoke("add_track_to_inbox", { trackId: first.trackId });
    await invoke("set_track_liked", { trackId: first.trackId, liked: true });
    await invoke("set_track_rating", { trackId: first.trackId, rating: 5 });
    const tag = await invoke("create_tag", { name: "  Plan   08  " });
    await invoke("add_track_tag", { trackId: first.trackId, tagId: tag.id });

    await invoke("play_track", { trackId: first.trackId, sourceId: first.sourceId });
    await waitFor("the first Plan 08 track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("enqueue_track", { trackId: second.trackId, sourceId: second.sourceId });
    await invoke("seek_playback", { positionMs: 1_000 });
    await invoke("set_repeat_mode", { repeatMode: "all" });
    await invoke("set_shuffle_enabled", { enabled: true });

    const queue = await invoke("get_queue_workspace");
    if (queue.current?.trackId !== first.trackId || queue.later?.length !== 1 || queue.autoplay?.length !== 0) {
      throw new Error(`the Plan 08 queue was not populated as expected: ${JSON.stringify(queue)}`);
    }
    const savedSnapshot = await invoke("save_queue_snapshot", { name: "Plan 08 Smoke" });
    if (savedSnapshot.entryCount !== 2 || savedSnapshot.currentPositionMs < 500) {
      throw new Error(`the Plan 08 snapshot did not capture queue state: ${JSON.stringify(savedSnapshot)}`);
    }
    const collection = await invoke("get_track_collection_states", { trackIds: [first.trackId] });
    if (collection.length !== 1 || !collection[0].liked || collection[0].rating !== 5 || !collection[0].inInbox || collection[0].tags?.every((item) => item.id !== tag.id)) {
      throw new Error(`the Plan 08 collection state was not persisted: ${JSON.stringify(collection)}`);
    }

    await invoke("toggle_play_pause");
    await waitFor("the Plan 08 first track to pause", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "paused" ? snapshot : false;
    });
    console.log("packaged Plan 08 persistence flow passed");
  } else if (mode === "plan08-restart") {
    const status = await waitFor("the indexed Plan 08 library after restart", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    if (!status) {
      throw new Error("the Plan 08 library was not available after restart");
    }
    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 100, sort: "title", descending: false, folderId: null },
    });
    const tracks = [...(libraryPage.items ?? [])].sort((left, right) => left.title.localeCompare(right.title));
    const first = tracks[0];
    if (!first?.trackId || !first.sourceId) {
      throw new Error(`the Plan 08 track identity was not retained: ${JSON.stringify(libraryPage)}`);
    }

    const playlists = await invoke("list_playlists");
    const playlist = playlists.find((item) => item.name === "Plan 08 Smoke");
    if (!playlist) {
      throw new Error(`the Plan 08 playlist was not retained: ${JSON.stringify(playlists)}`);
    }
    const playlistDetail = await invoke("get_playlist", { playlistId: playlist.id });
    if (!playlistDetail || playlistDetail.items?.length !== 2) {
      throw new Error(`the Plan 08 playlist items were not retained: ${JSON.stringify(playlistDetail)}`);
    }
    const collection = await invoke("get_track_collection_states", { trackIds: [first.trackId] });
    if (collection.length !== 1 || !collection[0].liked || collection[0].rating !== 5 || !collection[0].inInbox || collection[0].tags?.every((item) => item.name !== "Plan 08")) {
      throw new Error(`the Plan 08 collection state was not retained: ${JSON.stringify(collection)}`);
    }

    const queue = await invoke("get_queue_workspace");
    if (queue.current?.trackId !== first.trackId || queue.later?.length !== 1 || queue.repeatMode !== "all" || !queue.shuffleEnabled || queue.currentPositionMs < 500) {
      throw new Error(`the Plan 08 queue was not retained: ${JSON.stringify(queue)}`);
    }
    const snapshots = await invoke("list_queue_snapshots");
    const snapshot = snapshots.find((item) => item.name === "Plan 08 Smoke");
    if (!snapshot || snapshot.entryCount !== 2 || snapshot.currentPositionMs < 500) {
      throw new Error(`the Plan 08 snapshot was not retained: ${JSON.stringify(snapshots)}`);
    }
    const beforePlay = await invoke("get_playback_snapshot");
    if (beforePlay.phase !== "idle" || beforePlay.currentTrackId !== first.trackId) {
      throw new Error(`Plan 08 restart autoplayed or lost the current item: ${JSON.stringify(beforePlay)}`);
    }
    await invoke("toggle_play_pause");
    const resumed = await waitFor("the saved Plan 08 current track to resume", async () => {
      const next = await invoke("get_playback_snapshot");
      return next.phase === "playing" && next.positionMs >= Math.max(0, queue.currentPositionMs - 500) ? next : false;
    });
    if (resumed.currentTrackId !== first.trackId) {
      throw new Error(`the Plan 08 current position did not resume: ${JSON.stringify({ queue, resumed })}`);
    }
    console.log("packaged Plan 08 restart persistence passed");
  } else {
    throw new Error(`unknown smoke mode: ${mode}`);
  }
} finally {
  await browser.close();
}
