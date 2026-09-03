/* global console, process, window */

import { chromium } from "@playwright/test";

const mode = process.argv[2] ?? "flow";
const cdpUrl = process.env.SPOTDIY_PACKAGED_CDP_URL;
const fixtureFolder = process.env.SPOTDIY_PACKAGED_FIXTURE;

if (!cdpUrl) {
  throw new Error("SPOTDIY_PACKAGED_CDP_URL is required");
}
if ((mode === "flow" || mode === "plan08" || mode === "plan09" || mode === "plan11" || mode === "plan12" || mode === "plan14" || mode === "plan15") && !fixtureFolder) {
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
  } else if (mode === "plan15") {
    await invoke("add_library_folders", { paths: [fixtureFolder] });
    const status = await waitFor("the Plan 15 synthetic folder scan", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    if (!status) {
      throw new Error("the Plan 15 library scan did not complete");
    }

    const dataset = await invoke("get_visual_library_dataset", {
      request: { query: null, genre: null, artist: null, likedOnly: false, limit: 2_000 },
    });
    const serializedDataset = JSON.stringify(dataset);
    if (
      dataset.totalTracks < 2 ||
      dataset.returnedTracks !== dataset.tracks?.length ||
      dataset.returnedTracks > 5_000 ||
      serializedDataset.includes(fixtureFolder) ||
      /(?:localMediaPath|mediaPath|providerUrl|credentials?)/i.test(serializedDataset)
    ) {
      throw new Error(`Plan 15 dataset contract failed: ${serializedDataset}`);
    }

    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 100, sort: "title", descending: false, folderId: null },
    });
    const first = [...(libraryPage.items ?? [])].sort((left, right) => left.title.localeCompare(right.title))[0];
    if (!first?.trackId || !first.sourceId) {
      throw new Error(`the Plan 15 fixture did not expose a playable track: ${JSON.stringify(libraryPage)}`);
    }

    await invoke("play_track", { trackId: first.trackId, sourceId: first.sourceId });
    await waitFor("the Plan 15 fixture track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("toggle_play_pause");
    await waitFor("the Plan 15 fixture track to pause", async () => (await invoke("get_playback_snapshot")).phase === "paused");

    await page.getByRole("link", { name: "Music Map", exact: true }).click();
    await page.getByRole("heading", { name: "See the connections." }).waitFor({ state: "visible" });
    await page.locator(".music-map-svg").waitFor({ state: "visible" });
    await page.locator(".visual-node-list-item").filter({ hasText: first.title }).first().click();
    await page.getByText("SELECTED TRACK", { exact: true }).waitFor({ state: "visible" });
    await page.getByRole("button", { name: "Close inspector", exact: true }).click();
    await page.getByRole("button", { name: "Preview", exact: true }).click();
    await waitFor("the native Plan 15 preview", async () => {
      const preview = await invoke("get_preview_state");
      return preview.phase === "playing" && preview.trackId === first.trackId ? preview : false;
    });
    await page.getByRole("button", { name: "Cancel Preview", exact: true }).click();
    await waitFor("the Plan 15 preview cancellation", async () => (await invoke("get_preview_state")).phase === "idle");

    await page.getByRole("link", { name: "Library Galaxy", exact: true }).click();
    await page.getByRole("heading", { name: "Library Galaxy" }).waitFor({ state: "visible" });
    await page.locator("canvas[aria-label='Library Galaxy canvas']").waitFor({ state: "visible" });
    await page.locator(".visual-node-list-item").filter({ hasText: first.title }).first().click();
    await page.getByText("SELECTED TRACK", { exact: true }).waitFor({ state: "visible" });
    await page.getByRole("button", { name: "Close inspector", exact: true }).click();

    await page.getByRole("link", { name: "Theme Studio", exact: true }).click();
    await page.getByRole("heading", { name: "Make a place to listen." }).waitFor({ state: "visible" });
    if (await page.locator(".theme-token-field").count() !== 15) {
      throw new Error("Plan 15 Theme Studio did not expose all 15 token fields");
    }
    await page.getByRole("button", { name: "Preview on App", exact: true }).click();
    await page.locator("html[data-theme='custom']").waitFor({ state: "attached" });
    await page.getByRole("button", { name: "Stop App Preview", exact: true }).click();
    await page.locator("html[data-theme='dark']").waitFor({ state: "attached" });
    console.log("packaged Plan 15 visual routes, dataset contract, local preview, and Theme Studio passed");
  } else if (mode === "plan15-restart") {
    const status = await waitFor("the indexed Plan 15 library after restart", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    const dataset = await invoke("get_visual_library_dataset", {
      request: { query: null, genre: null, artist: null, likedOnly: false, limit: 2_000 },
    });
    const preview = await invoke("get_preview_state");
    const playback = await invoke("get_playback_snapshot");
    if (!status || dataset.returnedTracks < 2 || preview.phase !== "idle" || playback.phase !== "idle") {
      throw new Error(`Plan 15 restart boundary failed: ${JSON.stringify({ status, dataset, preview, playback })}`);
    }
    await page.getByRole("link", { name: "Music Map", exact: true }).click();
    await page.getByRole("heading", { name: "See the connections." }).waitFor({ state: "visible" });
    await page.getByRole("link", { name: "Library Galaxy", exact: true }).click();
    await page.getByRole("heading", { name: "Library Galaxy" }).waitFor({ state: "visible" });
    await page.getByRole("link", { name: "Theme Studio", exact: true }).click();
    await page.getByRole("heading", { name: "Make a place to listen." }).waitFor({ state: "visible" });
    console.log("packaged Plan 15 restart dataset, preview isolation, and visual-route boundary passed");
  } else if (mode === "plan14") {
    await invoke("add_library_folders", { paths: [fixtureFolder] });
    const status = await waitFor("the Plan 14 synthetic folder scan", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    if (!status) {
      throw new Error("the Plan 14 library scan did not complete");
    }

    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 100, sort: "title", descending: false, folderId: null },
    });
    const tracks = [...(libraryPage.items ?? [])].sort((left, right) => left.title.localeCompare(right.title));
    if (tracks.length < 2 || !tracks[0].trackId || !tracks[0].sourceId || !tracks[1].trackId || !tracks[1].sourceId) {
      throw new Error(`the Plan 14 fixture did not expose two playable tracks: ${JSON.stringify(libraryPage)}`);
    }
    const first = tracks[0];
    const second = tracks[1];
    const initialOverview = await invoke("get_analytics_overview");

    await invoke("play_track", { trackId: first.trackId, sourceId: first.sourceId });
    await waitFor("the first Plan 14 track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.currentTrackId === first.trackId && snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("enqueue_track", { trackId: second.trackId, sourceId: second.sourceId });
    await page.waitForTimeout(500);
    await invoke("next_track");
    await waitFor("the Plan 14 qualified track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.currentTrackId === second.trackId && snapshot.phase === "playing" ? snapshot : false;
    });
    await waitFor("the early Plan 14 skip to persist", async () => {
      const next = await invoke("get_analytics_overview");
      return next.skips > initialOverview.skips ? next : false;
    });
    await page.waitForTimeout(3_000);
    await invoke("next_track");
    const listened = await waitFor("the qualified Plan 14 play to persist", async () => {
      const next = await invoke("get_analytics_overview");
      return next.qualifiedPlays > initialOverview.qualifiedPlays && next.sessionCount === 1 ? next : false;
    });
    if (listened.listenedMs <= initialOverview.listenedMs || listened.skips !== initialOverview.skips + 1) {
      throw new Error(`Plan 14 history qualification was not recorded: ${JSON.stringify({ initialOverview, listened })}`);
    }

    await invoke("set_private_session", { enabled: true });
    const privateBefore = await invoke("get_analytics_overview");
    await invoke("play_track", { trackId: first.trackId, sourceId: first.sourceId });
    await waitFor("the private Plan 14 track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("next_track");
    const privateAfter = await invoke("get_analytics_overview");
    if (JSON.stringify(privateAfter) !== JSON.stringify(privateBefore)) {
      throw new Error(`Private Session added history: ${JSON.stringify({ privateBefore, privateAfter })}`);
    }
    await invoke("set_private_session", { enabled: false });

    await invoke("play_track", { trackId: first.trackId, sourceId: first.sourceId });
    await waitFor("the durable Plan 14 queue track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("enqueue_track", { trackId: second.trackId, sourceId: second.sourceId });
    await invoke("toggle_play_pause");
    await waitFor("the durable Plan 14 queue to pause", async () => (await invoke("get_playback_snapshot")).phase === "paused");
    const durableQueue = await invoke("get_queue_workspace");
    await invoke("enter_temporary_mode");
    await invoke("enqueue_track", { trackId: first.trackId, sourceId: first.sourceId });
    await invoke("exit_temporary_mode");
    const restoredQueue = await invoke("get_queue_workspace");
    const restoredSnapshot = await invoke("get_playback_snapshot");
    if (restoredSnapshot.phase !== "idle" || JSON.stringify(restoredQueue) !== JSON.stringify(durableQueue)) {
      throw new Error(`Temporary Mode did not restore the durable queue: ${JSON.stringify({ durableQueue, restoredQueue, restoredSnapshot })}`);
    }

    await invoke("set_track_liked", { trackId: first.trackId, liked: true });
    const smartPlaylist = await invoke("create_smart_playlist", {
      input: {
        name: "Plan 14 Smart",
        rule: {
          type: "group",
          operator: "and",
          children: [{ type: "predicate", field: "liked", operation: "true", value: null }],
        },
        sortMode: "title",
        sortDirection: "asc",
        limitCount: 10,
      },
    });
    const preview = await invoke("preview_smart_playlist", { playlistId: smartPlaylist.id, page: 0, pageSize: 20 });
    if (preview.total < 1 || !preview.items.some((item) => item.trackId === first.trackId)) {
      throw new Error(`Plan 14 smart preview did not use local collection state: ${JSON.stringify(preview)}`);
    }
    const mix = await invoke("open_smart_mix", {
      pool: { smartPlaylist: smartPlaylist.id },
      options: { familiarity: 50, variety: 70, freshness: 50, count: 1, recentTrackIds: [] },
      seed: 42,
    });
    if (mix.phase !== "idle" || mix.currentTrackId !== null || mix.queueLength !== 1) {
      throw new Error(`Plan 14 smart mix did not replace the queue without autoplay: ${JSON.stringify(mix)}`);
    }

    await page.getByRole("link", { name: "Analytics", exact: true }).click();
    await page.getByText("On repeat", { exact: true }).waitFor({ state: "visible" });
    await page.getByText("Weekly rhythm", { exact: true }).waitFor({ state: "visible" });
    console.log("packaged Plan 14 history, privacy, temporary queue, smart mix, and analytics flow passed");
  } else if (mode === "plan12") {
    const settings = await invoke("get_settings_snapshot");
    if (
      settings.windowsIntegration?.smtcEnabled !== true ||
      settings.windowsIntegration?.globalShortcutsEnabled !== false ||
      !Array.isArray(settings.globalShortcuts) ||
      settings.globalShortcuts.length !== 9 ||
      !Array.isArray(settings.outputProfiles) ||
      settings.outputProfiles.length !== 0
    ) {
      throw new Error(`Plan 12 defaults were not loaded from schema 8: ${JSON.stringify(settings)}`);
    }

    let integration = await invoke("get_windows_integration_snapshot");
    if (!integration.platformSupported || integration.trayStatus !== "ready") {
      throw new Error(`the packaged Windows integration did not initialize its tray: ${JSON.stringify(integration)}`);
    }
    if (integration.globalShortcutsEnabled || integration.shortcutStatuses.length !== 9) {
      throw new Error(`global shortcuts were not disabled by default: ${JSON.stringify(integration)}`);
    }
    if (integration.smtcStatus === "ready") {
      console.log("SMTC READY");
    } else if (["failed", "unsupported"].includes(integration.smtcStatus) && integration.smtcDetail) {
      console.log(`SMTC UNAVAILABLE -- ${integration.smtcDetail}`);
    } else {
      throw new Error(`SMTC did not report ready or a truthful unavailable state: ${JSON.stringify(integration)}`);
    }

    for (const status of integration.shortcutStatuses) {
      await invoke("update_global_shortcut", {
        binding: {
          action: status.action,
          accelerator: status.accelerator,
          enabled: false,
        },
      });
    }
    await invoke("update_global_shortcut", {
      binding: {
        action: "toggleMiniOverlay",
        accelerator: "Ctrl+Alt+Shift+F12",
        enabled: true,
      },
    });
    integration = await invoke("set_global_shortcuts_enabled", { enabled: true });
    const testShortcut = integration.shortcutStatuses.find((status) => status.action === "toggleMiniOverlay");
    if (!testShortcut || !["registered", "conflict", "failed"].includes(testShortcut.status)) {
      throw new Error(`the controlled shortcut did not produce a native registration status: ${JSON.stringify(integration)}`);
    }
    console.log(`global shortcut status: ${testShortcut.status}`);

    await invoke("add_library_folders", { paths: [fixtureFolder] });
    await waitFor("the Plan 12 synthetic folder scan", async () => {
      const status = await invoke("get_library_status");
      return status.folders?.length === 1 && status.indexedTrackCount >= 2 && !status.isScanning ? status : false;
    });
    const libraryPage = await invoke("get_library_page", {
      request: { page: 0, pageSize: 1, sort: "title", descending: false, folderId: null },
    });
    const firstTrack = libraryPage.items?.[0];
    if (!firstTrack?.trackId || !firstTrack.sourceId) {
      throw new Error(`the Plan 12 fixture did not expose a playable track: ${JSON.stringify(libraryPage)}`);
    }
    await invoke("play_track", { trackId: firstTrack.trackId, sourceId: firstTrack.sourceId });
    await waitFor("the Plan 12 synthetic track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    await invoke("toggle_play_pause");
    await waitFor("the Plan 12 synthetic track to pause", async () => (await invoke("get_playback_snapshot")).phase === "paused");

    async function findOverlayPage(kind) {
      const pages = browser.contexts().flatMap((context) => context.pages());
      for (const candidate of pages) {
        if (await candidate.locator(`[data-overlay-kind="${kind}"]`).count()) {
          return candidate;
        }
      }
      return null;
    }

    async function waitForOverlay(kind, expectedLabel) {
      return waitFor(`${kind} overlay window`, async () => {
        const overlayPage = await findOverlayPage(kind);
        if (!overlayPage) {
          return false;
        }
        const facts = await overlayPage.evaluate(async () => {
          const label = window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? null;
          const alwaysOnTop = await window.__TAURI_INTERNALS__?.invoke("plugin:window|is_always_on_top", { label });
          return { label, alwaysOnTop };
        });
        if (facts.label !== expectedLabel || facts.alwaysOnTop !== true) {
          throw new Error(`overlay window facts were not applied: ${JSON.stringify(facts)}`);
        }
        return { page: overlayPage, facts };
      });
    }

    async function openAndCheckOverlay(kind, expectedLabel) {
      integration = await invoke("open_overlay", { kind });
      const state = integration.overlays.find((overlay) => overlay.kind === kind);
      if (state?.status !== "open") {
        throw new Error(`the ${kind} overlay did not report open: ${JSON.stringify(integration)}`);
      }
      const first = await waitForOverlay(kind, expectedLabel);
      const duplicate = await invoke("open_overlay", { kind });
      const duplicateState = duplicate.overlays.find((overlay) => overlay.kind === kind);
      if (duplicateState?.status !== "open" || (await findOverlayPage(kind)) !== first.page) {
        throw new Error(`opening ${kind} twice did not reuse its native window`);
      }
      return first.page;
    }

    const miniPage = await openAndCheckOverlay("mini", "overlay-mini");
    await invoke("close_overlay", { kind: "mini" });
    await waitFor("Mini overlay close", async () => (await findOverlayPage("mini")) === null);
    await openAndCheckOverlay("mini", "overlay-mini");
    await invoke("close_overlay", { kind: "mini" });
    await waitFor("Mini overlay reopen close", async () => (await findOverlayPage("mini")) === null);

    await openAndCheckOverlay("edge", "overlay-edge");
    await openAndCheckOverlay("lyrics", "overlay-lyrics");
    const gamingPage = await openAndCheckOverlay("gaming", "overlay-gaming");
    if (!miniPage || !gamingPage) {
      throw new Error("the packaged overlay pages were not exposed to WebView2");
    }

    integration = await invoke("set_gaming_click_through", { enabled: true });
    if (!integration.gamingClickThrough) {
      throw new Error(`Gaming click-through did not enable after rescue registration: ${JSON.stringify(integration)}`);
    }
    integration = await invoke("set_gaming_click_through", { enabled: false });
    if (integration.gamingClickThrough) {
      throw new Error(`Gaming click-through did not disable through the native recovery path: ${JSON.stringify(integration)}`);
    }

    const beforePlayback = await invoke("get_playback_snapshot");
    if (beforePlayback.phase !== "paused" || !beforePlayback.currentTrackId) {
      throw new Error(`unexpected playback state before Plan 12 output validation: ${JSON.stringify(beforePlayback)}`);
    }
    const outputState = await invoke("create_output_profile", { name: "  Plan   12 Auto  " });
    let profile = outputState.outputProfiles.find((candidate) => candidate.name === "Plan 12 Auto");
    if (!profile) {
      throw new Error(`the Plan 12 output profile was not persisted: ${JSON.stringify(outputState)}`);
    }
    if (profile.audioDeviceName.toLowerCase() !== "auto") {
      profile = { ...profile, audioDeviceName: "auto" };
      const normalized = await invoke("update_output_profile", { profile });
      profile = normalized.outputProfiles.find((candidate) => candidate.id === profile.id) ?? profile;
    }
    const targetVolume = beforePlayback.volumePercent === 0 ? 1 : beforePlayback.volumePercent - 1;
    const targetProfile = { ...profile, volumePercent: targetVolume, muted: !beforePlayback.muted };
    const updated = await invoke("update_output_profile", { profile: targetProfile });
    const savedProfile = updated.outputProfiles.find((candidate) => candidate.id === profile.id);
    if (!savedProfile || savedProfile.audioDeviceName !== "auto" || savedProfile.volumePercent !== targetVolume || savedProfile.muted !== !beforePlayback.muted) {
      throw new Error(`the Plan 12 output profile edit was not persisted: ${JSON.stringify(updated)}`);
    }
    const applied = await invoke("apply_output_profile", { id: profile.id });
    if (applied.selectedAudioDevice !== "auto" || applied.volumePercent !== targetVolume || applied.muted !== !beforePlayback.muted || applied.currentTrackId !== beforePlayback.currentTrackId || applied.queueLength !== beforePlayback.queueLength) {
      throw new Error(`the Plan 12 output profile apply changed unexpected playback state: ${JSON.stringify({ beforePlayback, applied })}`);
    }
    const restoredProfile = {
      ...savedProfile,
      audioDeviceName: beforePlayback.selectedAudioDevice,
      volumePercent: beforePlayback.volumePercent,
      muted: beforePlayback.muted,
    };
    await invoke("update_output_profile", { profile: restoredProfile });
    const restored = await invoke("apply_output_profile", { id: profile.id });
    if (restored.selectedAudioDevice !== beforePlayback.selectedAudioDevice || restored.volumePercent !== beforePlayback.volumePercent || restored.muted !== beforePlayback.muted || restored.currentTrackId !== beforePlayback.currentTrackId || restored.queueLength !== beforePlayback.queueLength) {
      throw new Error(`the Plan 12 output profile restore did not recover the prior output state: ${JSON.stringify({ beforePlayback, restored })}`);
    }

    for (const kind of ["edge", "lyrics", "gaming"]) {
      await invoke("close_overlay", { kind });
    }
    integration = await invoke("get_windows_integration_snapshot");
    if (integration.overlays.some((overlay) => overlay.status !== "closed") || integration.gamingClickThrough) {
      throw new Error(`overlays were not fully closed before restart: ${JSON.stringify(integration)}`);
    }
    console.log("packaged Plan 12 native integration flow passed");
  } else if (mode === "plan11") {
    const legacySettings = await invoke("get_settings_snapshot");
    if (legacySettings.theme !== "light" || legacySettings.firstRun !== false || legacySettings.storageMode !== "standard" || legacySettings.layoutProfile !== "comfortable") {
      throw new Error(`schema-6 settings were not preserved through migration 7: ${JSON.stringify(legacySettings)}`);
    }

    const customTheme = {
      schemaVersion: 1,
      name: "Packaged Plan 11",
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
    await invoke("set_setting", { setting: { key: "layoutProfile", value: "dense" } });
    await invoke("set_setting", { setting: { key: "customTheme", value: customTheme } });
    const savedSettings = await invoke("set_setting", { setting: { key: "theme", value: "custom" } });
    if (savedSettings.theme !== "custom" || savedSettings.layoutProfile !== "dense" || savedSettings.customTheme?.name !== customTheme.name) {
      throw new Error(`Plan 11 appearance settings did not persist after migration 7: ${JSON.stringify(savedSettings)}`);
    }

    await invoke("add_library_folders", { paths: [fixtureFolder] });
    const status = await waitFor("the Plan 11 synthetic folder scan", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    if (!status) {
      throw new Error("the Plan 11 library scan did not complete");
    }

    await page.reload();
    await page.getByRole("link", { name: "Home", exact: true }).waitFor({ state: "visible" });
    await page.getByRole("link", { name: "Home", exact: true }).click();
    await page.getByText("YOUR MUSIC, YOUR MACHINE", { exact: true }).waitFor({ state: "visible" });
    if (await page.getByText("START HERE", { exact: true }).count() > 0) {
      throw new Error("the populated Plan 11 home dashboard still rendered onboarding");
    }

    await page.getByRole("link", { name: "Your library", exact: true }).click();
    await page.getByRole("heading", { name: "Your local collection" }).waitFor({ state: "visible" });
    const rows = page.locator("[data-testid^='library-track-']");
    await waitFor("two indexed Plan 11 tracks", async () => (await rows.count()) === 2);
    const firstRow = rows.nth(0);
    const secondRow = rows.nth(1);
    const firstTrack = await invoke("get_library_page", {
      request: { page: 0, pageSize: 1, sort: "title", descending: false, folderId: null },
    });
    const firstItem = firstTrack.items?.[0];
    if (!firstItem?.trackId || !firstItem.sourceId) {
      throw new Error(`the Plan 11 library did not expose a playable track identity: ${JSON.stringify(firstTrack)}`);
    }

    await firstRow.getByRole("button", { name: /Play now/ }).click();
    const playing = await waitFor("the first Plan 11 track to play", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.phase === "playing" ? snapshot : false;
    });
    await firstRow.getByRole("button", { name: /Inspect/ }).click();
    const inspector = page.locator(".inspector-panel");
    await inspector.getByText("PERSISTED LOCAL TRACK", { exact: true }).waitFor({ state: "visible" });
    await inspector.getByText("SOURCES", { exact: true }).waitFor({ state: "visible" });
    const inspectorText = await inspector.innerText();
    if (inspectorText.includes(fixtureFolder) || inspectorText.includes("spotdiy-mpv-")) {
      throw new Error("the packaged Track Inspector exposed a local path or owned process detail");
    }
    await page.keyboard.press("Escape");
    await inspector.waitFor({ state: "hidden" });

    const inspectorDto = await invoke("get_track_inspector", { trackId: firstItem.trackId });
    if (inspectorDto.sources?.some((source) => source.provider === "local" && source.canonicalUrl !== null)) {
      throw new Error(`the packaged local inspector returned a canonical URL: ${JSON.stringify(inspectorDto)}`);
    }

    await secondRow.getByRole("button", { name: /Add .* to queue/ }).click();
    const queued = await waitFor("the second Plan 11 track to join the queue", async () => {
      const snapshot = await invoke("get_playback_snapshot");
      return snapshot.queueLength === 2 ? snapshot : false;
    });
    const beforeModes = await invoke("get_playback_snapshot");
    await page.getByRole("button", { name: "Open mini player" }).click();
    await page.getByRole("contentinfo", { name: "Mini now playing" }).waitFor({ state: "visible" });
    const miniSnapshot = await invoke("get_playback_snapshot");
    if (miniSnapshot.currentTrackId !== beforeModes.currentTrackId || miniSnapshot.phase !== beforeModes.phase) {
      throw new Error(`entering Mini mode changed playback state: ${JSON.stringify({ beforeModes, miniSnapshot })}`);
    }

    await page.getByRole("button", { name: "Open expanded now playing" }).click();
    const expanded = page.getByRole("dialog", { name: "Expanded now playing" });
    await expanded.getByText("QUALITY / PROVENANCE", { exact: true }).waitFor({ state: "visible" });
    await expanded.getByRole("link", { name: "Lyrics" }).waitFor({ state: "visible" });
    await expanded.getByRole("button", { name: "Inspect track" }).waitFor({ state: "visible" });
    await page.keyboard.press("Escape");
    await expanded.waitFor({ state: "hidden" });

    await page.getByRole("button", { name: "Open queue" }).click();
    const queue = page.getByRole("dialog", { name: "Persistent queue" });
    await queue.waitFor({ state: "visible" });
    await page.keyboard.press("Escape");
    await queue.waitFor({ state: "hidden" });

    await page.getByRole("link", { name: "Open lyrics" }).click();
    await page.waitForURL(/\/lyrics/);
    await page.getByRole("link", { name: "Your library", exact: true }).click();
    await page.getByRole("heading", { name: "Your local collection" }).waitFor({ state: "visible" });

    await page.keyboard.press("Control+k");
    const palette = page.getByRole("region", { name: "Command palette" });
    await palette.getByRole("button", { name: "Open queue" }).waitFor({ state: "visible" });
    await palette.getByRole("textbox", { name: "Search commands" }).fill("expanded");
    await palette.getByRole("button", { name: "Open expanded now playing" }).waitFor({ state: "visible" });
    await page.keyboard.press("Escape");
    await palette.waitFor({ state: "hidden" });

    const finalSnapshot = await invoke("get_playback_snapshot");
    if (finalSnapshot.currentTrackId !== playing.currentTrackId || finalSnapshot.queueLength !== queued.queueLength) {
      throw new Error(`the Plan 11 shell flow changed the persistent playback queue unexpectedly: ${JSON.stringify({ playing, queued, finalSnapshot })}`);
    }
    console.log("packaged Plan 11 shell, inspector, appearance, queue, and no-autoplay flow passed");
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
  } else if (mode === "plan14-restart") {
    const status = await waitFor("the indexed Plan 14 library after restart", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    const overview = await invoke("get_analytics_overview");
    const modeState = await invoke("get_listening_mode_state");
    const smartPlaylists = await invoke("list_smart_playlists");
    const smartPlaylist = smartPlaylists.find((item) => item.name === "Plan 14 Smart");
    if (!status || overview.sessionCount !== 1 || overview.qualifiedPlays < 1 || overview.skips < 1) {
      throw new Error(`Plan 14 analytics did not survive restart: ${JSON.stringify({ status, overview })}`);
    }
    if (modeState.privateSession || modeState.temporary || !smartPlaylist) {
      throw new Error(`Plan 14 session-only state or smart playlist restart boundary failed: ${JSON.stringify({ modeState, smartPlaylists })}`);
    }
    const preview = await invoke("preview_smart_playlist", { playlistId: smartPlaylist.id, page: 0, pageSize: 20 });
    if (preview.total < 1) {
      throw new Error(`Plan 14 smart playlist did not survive restart: ${JSON.stringify(preview)}`);
    }
    const snapshot = await invoke("get_playback_snapshot");
    if (snapshot.phase !== "idle" || snapshot.currentTrackId !== null || snapshot.queueLength !== 1) {
      throw new Error(`the Plan 14 smart mix restarted with autoplay or changed queue state: ${JSON.stringify(snapshot)}`);
    }
    await page.getByRole("link", { name: "Analytics", exact: true }).click();
    await page.getByText("On repeat", { exact: true }).waitFor({ state: "visible" });
    console.log("packaged Plan 14 restart analytics, smart playlist, private-state, and no-autoplay boundary passed");
  } else if (mode === "plan12-restart") {
    const settings = await invoke("get_settings_snapshot");
    if (settings.windowsIntegration?.smtcEnabled !== true || settings.windowsIntegration?.globalShortcutsEnabled !== true) {
      throw new Error(`Plan 12 Windows settings did not persist across restart: ${JSON.stringify(settings)}`);
    }
    const integration = await invoke("get_windows_integration_snapshot");
    const profile = integration.outputProfiles.find((candidate) => candidate.name === "Plan 12 Auto");
    const shortcut = integration.shortcutStatuses.find((status) => status.action === "toggleMiniOverlay");
    if (!profile || profile.audioDeviceName !== "auto" || !shortcut || shortcut.accelerator !== "Ctrl+Alt+Shift+F12") {
      throw new Error(`Plan 12 output profile or controlled shortcut did not persist across restart: ${JSON.stringify(integration)}`);
    }
    if (integration.overlays.some((overlay) => overlay.status !== "closed") || integration.gamingClickThrough) {
      throw new Error(`Plan 12 session-only overlay state persisted across restart: ${JSON.stringify(integration)}`);
    }
    if (integration.smtcStatus === "ready") {
      console.log("SMTC READY after restart");
    } else if (["failed", "unsupported"].includes(integration.smtcStatus) && integration.smtcDetail) {
      console.log(`SMTC UNAVAILABLE after restart — ${integration.smtcDetail}`);
    } else {
      throw new Error(`SMTC restart state was not truthful: ${JSON.stringify(integration)}`);
    }
    await invoke("delete_output_profile", { id: profile.id });
    console.log("packaged Plan 12 restart persistence and session-state boundary passed");
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
  } else if (mode === "plan11-restart") {
    const status = await waitFor("the indexed Plan 11 library after restart", async () => {
      const next = await invoke("get_library_status");
      return next.folders?.length === 1 && next.indexedTrackCount >= 2 && !next.isScanning ? next : false;
    });
    const settings = await invoke("get_settings_snapshot");
    if (settings.theme !== "custom" || settings.layoutProfile !== "dense" || settings.customTheme?.name !== "Packaged Plan 11" || settings.firstRun !== false) {
      throw new Error(`the migrated Plan 11 appearance settings were not retained after restart: ${JSON.stringify(settings)}`);
    }
    const snapshot = await invoke("get_playback_snapshot");
    const queue = await invoke("get_queue_workspace");
    if (!status || snapshot.phase !== "idle" || snapshot.queueLength !== 2 || !snapshot.currentTrackId || !queue.current || queue.later?.length !== 1) {
      throw new Error(`the Plan 11 queue did not persist without autoplay: ${JSON.stringify({ status, snapshot, queue })}`);
    }
    const inspector = await invoke("get_track_inspector", { trackId: snapshot.currentTrackId });
    if (inspector.sources?.some((source) => source.provider === "local" && source.canonicalUrl !== null)) {
      throw new Error(`the restarted Plan 11 inspector returned a local canonical URL: ${JSON.stringify(inspector)}`);
    }
    console.log("packaged Plan 11 restart appearance, queue, inspector, and no-autoplay boundary passed");
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
