/* global console, process, window */

import { chromium } from "@playwright/test";

const mode = process.argv[2] ?? "flow";
const cdpUrl = process.env.SPOTDIY_PACKAGED_CDP_URL;
const fixtureFolder = process.env.SPOTDIY_PACKAGED_FIXTURE;

if (!cdpUrl) {
  throw new Error("SPOTDIY_PACKAGED_CDP_URL is required");
}
if ((mode === "flow" || mode === "plan08" || mode === "plan09") && !fixtureFolder) {
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
    const result = await page.evaluate(async ({ command: commandName, args: commandArgs }) => {
      try {
        const internals = window.__TAURI_INTERNALS__;
        if (!internals || typeof internals.invoke !== "function") {
          throw new Error("the packaged page does not expose the Tauri invoke bridge");
        }
        return { ok: true, value: await internals.invoke(commandName, commandArgs) };
      } catch (error) {
        let detail;
        try {
          detail = JSON.stringify(error);
        } catch {
          detail = String(error);
        }
        return { ok: false, error: detail };
      }
    }, { command, args });
    if (!result.ok) {
      throw new Error(`${command} failed: ${result.error}`);
    }
    return result.value;
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
  } else if (mode === "plan09") {
    await invoke("add_library_folders", { paths: [fixtureFolder] });
    await waitFor("the synthetic lyrics folder scan", async () => {
      const status = await invoke("get_library_status");
      return status.folders?.length === 1 && status.indexedTrackCount >= 2 && !status.isScanning ? status : false;
    });

    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 100, sort: "title", descending: false, folderId: null },
    });
    const tracks = [...(libraryPage.items ?? [])].sort((left, right) => left.title.localeCompare(right.title));
    if (tracks.length < 2 || !tracks[0].trackId || !tracks[0].sourceId || !tracks[1].trackId || !tracks[1].sourceId) {
      throw new Error("the packaged Plan 09 fixture did not expose two playable tracks");
    }
    const first = tracks[0];
    const second = tracks[1];

    const sidecar = await invoke("get_lyrics", { trackId: first.trackId, currentSourceId: first.sourceId });
    if (sidecar?.source !== "sidecar" || sidecar.syncKind !== "timed" || sidecar.cues?.length !== 2) {
      throw new Error(`synthetic sidecar lyrics were not loaded as timed cues: ${JSON.stringify({ source: sidecar?.source, syncKind: sidecar?.syncKind, cueCount: sidecar?.cues?.length })}`);
    }

    await invoke("play_track", { trackId: first.trackId, sourceId: first.sourceId });
    await waitFor("the first Plan 09 track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.currentTrackId === first.trackId && snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("toggle_play_pause");
    await waitFor("the first Plan 09 track to pause", async () => (await invoke("get_playback_snapshot")).phase === "paused");

    await invoke("seek_playback", { positionMs: 500 });
    await waitFor("the Plan 09 A seek position to settle", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.positionMs >= 400 && snapshot.positionMs <= 800 ? snapshot : false;
    });
    const pointA = await invoke("set_ab_loop_a");
    if (pointA.abLoop?.aMs === null || pointA.abLoop?.bMs !== null || pointA.abLoop?.active) {
      throw new Error(`the Plan 09 A point was not stored: ${JSON.stringify(pointA.abLoop)}`);
    }
    const expectedA = pointA.abLoop.aMs;
    await invoke("seek_playback", { positionMs: 1_500 });
    await waitFor("the Plan 09 B seek position to settle", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.positionMs >= 1_000 ? snapshot : false;
    });
    const pointB = await invoke("set_ab_loop_b");
    if (!pointB.abLoop?.active || pointB.abLoop.aMs !== expectedA || pointB.abLoop.bMs === null || pointB.abLoop.bMs <= expectedA) {
      throw new Error(`the Plan 09 B point did not activate the loop: ${JSON.stringify(pointB.abLoop)}`);
    }

    const preset = await invoke("save_ab_loop_preset", { trackId: first.trackId, name: "  Synthetic   Loop  " });
    const presets = await invoke("list_ab_loop_presets", { trackId: first.trackId });
    if (!preset?.id || presets.length !== 1 || presets[0].name !== "Synthetic Loop") {
      throw new Error("the Plan 09 A/B preset was not persisted");
    }

    const bookmark = await invoke("create_bookmark", {
      trackId: first.trackId,
      positionMs: 750,
      note: "Synthetic bookmark",
    });
    const bookmarks = await invoke("list_bookmarks", { trackId: first.trackId });
    if (!bookmark?.id || bookmarks.length !== 1 || bookmarks[0].positionMs !== 750) {
      throw new Error("the Plan 09 bookmark was not persisted");
    }

    const manual = await invoke("save_manual_lyrics", {
      trackId: first.trackId,
      mode: "lrc",
      text: "[00:00.25]Manual synthetic line\n[00:01.50]Manual synthetic second line\n",
    });
    if (manual?.source !== "manual" || manual.syncKind !== "timed") {
      throw new Error("the Plan 09 manual lyrics override was not created");
    }
    await invoke("delete_manual_lyrics", { trackId: first.trackId });
    const fallback = await invoke("get_lyrics", { trackId: first.trackId, currentSourceId: first.sourceId });
    if (fallback?.source !== "sidecar" || fallback.cues?.length !== 2) {
      throw new Error("deleting the manual lyrics did not restore the sidecar fallback");
    }

    await invoke("enqueue_track", { trackId: second.trackId, sourceId: second.sourceId });
    await invoke("next_track");
    const boundary = await waitFor("the second Plan 09 track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.currentTrackId === second.trackId && snapshot.phase === "playing" ? snapshot : false;
    });
    if (boundary.abLoop?.aMs !== null || boundary.abLoop?.bMs !== null || boundary.abLoop?.active) {
      throw new Error(`the live A/B loop survived the track boundary: ${JSON.stringify(boundary.abLoop)}`);
    }

    await invoke("toggle_play_pause");
    await waitFor("the second Plan 09 track to pause", async () => (await invoke("get_playback_snapshot")).phase === "paused");
    await invoke("seek_playback", { positionMs: 1_000 });
    const queue = await invoke("get_queue_workspace");
    const snapshot = await invoke("get_playback_snapshot");
    if (snapshot.queueLength !== 2 || queue.current?.trackId !== second.trackId) {
      throw new Error(`the Plan 09 queue state was not retained before restart: ${JSON.stringify({ snapshot: { currentTrackId: snapshot.currentTrackId, queueLength: snapshot.queueLength, positionMs: snapshot.positionMs }, currentQueueTrackId: queue.current?.trackId, laterCount: queue.later?.length })}`);
    }
    console.log("packaged Plan 09 lyrics, bookmark, A/B loop, preset, and queue flow passed");
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
    await invoke("seek_playback", { positionMs: 1_000 });
    await waitFor("the Plan 08 seek position to settle", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.positionMs >= 500 ? snapshot : false;
    });

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
  } else if (mode === "plan09-restart") {
    const status = await waitFor("the indexed Plan 09 library after restart", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    if (!status) {
      throw new Error("the Plan 09 library was not available after restart");
    }
    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 100, sort: "title", descending: false, folderId: null },
    });
    const tracks = [...(libraryPage.items ?? [])].sort((left, right) => left.title.localeCompare(right.title));
    const first = tracks[0];
    const second = tracks[1];
    if (!first?.trackId || !first.sourceId || !second?.trackId || !second.sourceId) {
      throw new Error("the Plan 09 track identities were not retained");
    }

    const bookmarks = await invoke("list_bookmarks", { trackId: first.trackId });
    const presets = await invoke("list_ab_loop_presets", { trackId: first.trackId });
    if (bookmarks.length !== 1 || bookmarks[0].positionMs !== 750 || presets.length !== 1 || presets[0].name !== "Synthetic Loop") {
      throw new Error("the Plan 09 bookmark or A/B preset was not retained after restart");
    }
    const sidecar = await invoke("get_lyrics", { trackId: first.trackId, currentSourceId: first.sourceId });
    if (sidecar?.source !== "sidecar" || sidecar.cues?.length !== 2) {
      throw new Error("the Plan 09 sidecar fallback was not retained after restart");
    }

    const snapshot = await invoke("get_playback_snapshot");
    const queue = await invoke("get_queue_workspace");
    if (snapshot.phase !== "idle" || snapshot.queueLength !== 2 || snapshot.currentTrackId !== second.trackId || snapshot.abLoop?.active || snapshot.abLoop?.aMs !== null || snapshot.abLoop?.bMs !== null || queue.current?.trackId !== second.trackId) {
      throw new Error("the Plan 09 queue restart boundary reactivated A/B or lost queue state");
    }
    console.log("packaged Plan 09 restart bookmark, preset, sidecar, queue, and no-autoplay boundary passed");
  } else {
    throw new Error(`unknown smoke mode: ${mode}`);
  }
} finally {
  await browser.close();
}
