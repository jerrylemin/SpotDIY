import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  createSmartPlaylist,
  enterTemporaryMode,
  exitTemporaryMode,
  getAnalyticsOverview,
  getListeningHeatmap,
  getListeningModeState,
  listListeningSessions,
  listSmartPlaylists,
  openSmartMix,
  parseAnalyticsOverview,
  parseListeningHeatmap,
  parseSmartPlaylist,
  previewSmartPlaylist,
  setPrivateSession,
} from "../src/services/ipc";
import type {
  AnalyticsOverview,
  ListeningModeChange,
  ListeningSession,
  PlaybackSnapshot,
  SmartPlaylist,
  SmartPlaylistPreview,
} from "../src/types/domain";

const overview: AnalyticsOverview = {
  listenedMs: 90_000,
  qualifiedPlays: 2,
  skips: 1,
  uniqueTracks: 3,
  uniqueArtists: 2,
  sessionCount: 1,
};

const heatmap = Array.from({ length: 168 }, (_, index) => ({
  weekday: Math.floor(index / 24),
  hour: index % 24,
  listenedMs: index === 25 ? 90_000 : 0,
}));

const session: ListeningSession = {
  id: "session-1" as ListeningSession["id"],
  startedAt: "2026-09-01T10:00:00Z",
  endedAt: "2026-09-01T10:20:00Z",
  label: null,
  eventCount: 2,
  listenedMs: 90_000,
};

const smartPlaylist: SmartPlaylist = {
  id: "smart-1" as SmartPlaylist["id"],
  name: "Rock rotation",
  rule: { type: "predicate", field: "genre", operation: "equals", value: "rock" },
  sortMode: "lastPlayed",
  sortDirection: "desc",
  limitCount: 100,
  createdAt: "2026-09-01T10:00:00Z",
  updatedAt: "2026-09-01T10:00:00Z",
};

const preview: SmartPlaylistPreview = {
  items: [{
    trackId: "track-1" as SmartPlaylistPreview["items"][number]["trackId"],
    title: "Night Drive",
    artists: ["Luna Max"],
    album: "Afterglow",
    durationMs: 185_000,
    dateAdded: "2026-08-31T00:00:00Z",
    lastPlayed: null,
    playCount: 2,
    rating: 5,
    audioQuality: "lossless",
  }],
  total: 1,
  page: 0,
  pageSize: 20,
};

const playbackSnapshot: PlaybackSnapshot = {
  revision: 3,
  phase: "idle",
  currentQueueEntryId: null,
  currentTrackId: null,
  currentSourceId: null,
  title: null,
  artists: [],
  album: null,
  artworkPath: null,
  sources: [],
  positionMs: 0,
  durationMs: null,
  volumePercent: 100,
  muted: false,
  repeatMode: "off",
  shuffleEnabled: false,
  queueLength: 1,
  queueIndex: null,
  selectedAudioDevice: "auto",
  backendHealth: { ready: true, connected: true, detail: null, recoveryAction: null },
  recovering: false,
  error: null,
  abLoop: { aMs: null, bMs: null, active: false },
};

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
}

function nestedRule(depth: number): unknown {
  if (depth === 0) {
    return smartPlaylist.rule;
  }
  return { type: "group", operator: "and", children: [nestedRule(depth - 1)] };
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
});

describe("smart analytics IPC contracts", () => {
  it("strictly validates analytics and smart-playlist DTOs", () => {
    expect(parseAnalyticsOverview(overview)).toEqual(overview);
    expect(parseListeningHeatmap(heatmap)).toEqual(heatmap);
    expect(parseSmartPlaylist(smartPlaylist)).toEqual(smartPlaylist);
    expect(() => parseAnalyticsOverview({ ...overview, rawSql: "SELECT 1" })).toThrow();
    expect(() => parseSmartPlaylist({ ...smartPlaylist, rawPath: "C:\\Music\\track.flac" })).toThrow();
    expect(() => parseSmartPlaylist({ ...smartPlaylist, rule: nestedRule(5) })).toThrow();
  });

  it("round-trips native analytics, smart playlist, mix, and listening-mode calls", async () => {
    enableNativeRuntime();
    const modeChange: ListeningModeChange = {
      state: { privateSession: true, temporary: false },
      reason: "privateEnabled",
    };
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "get_analytics_overview":
          return overview;
        case "get_listening_heatmap":
          return heatmap;
        case "list_listening_sessions":
          return { items: [session], total: 1, page: 0, pageSize: 20 };
        case "list_smart_playlists":
          return [smartPlaylist];
        case "create_smart_playlist":
          return smartPlaylist;
        case "preview_smart_playlist":
          return preview;
        case "open_smart_mix":
          return playbackSnapshot;
        case "get_listening_mode_state":
          return modeChange.state;
        case "set_private_session":
          return modeChange;
        case "enter_temporary_mode":
          return { state: { privateSession: true, temporary: true }, reason: "temporaryEntered" };
        case "exit_temporary_mode":
          return { state: { privateSession: false, temporary: false }, reason: "temporaryExited" };
        default:
          throw new Error(`Unexpected native command: ${command}`);
      }
    });

    await expect(getAnalyticsOverview()).resolves.toEqual(overview);
    await expect(getListeningHeatmap()).resolves.toEqual(heatmap);
    await expect(listListeningSessions()).resolves.toEqual({ items: [session], total: 1, page: 0, pageSize: 20 });
    await expect(listSmartPlaylists()).resolves.toEqual([smartPlaylist]);
    await expect(createSmartPlaylist({
      name: "Rock rotation",
      rule: smartPlaylist.rule,
      sortMode: "lastPlayed",
      sortDirection: "desc",
      limitCount: 100,
    })).resolves.toEqual(smartPlaylist);
    await expect(previewSmartPlaylist(smartPlaylist.id)).resolves.toEqual(preview);
    await expect(openSmartMix("library", { familiarity: 40, variety: 70, freshness: 60, count: 25 }, 42)).resolves.toEqual(playbackSnapshot);
    await expect(getListeningModeState()).resolves.toEqual(modeChange.state);
    await expect(setPrivateSession(true)).resolves.toEqual(modeChange);
    await expect(enterTemporaryMode()).resolves.toEqual({
      state: { privateSession: true, temporary: true },
      reason: "temporaryEntered",
    });
    await expect(exitTemporaryMode()).resolves.toEqual({
      state: { privateSession: false, temporary: false },
      reason: "temporaryExited",
    });

    expect(invokeMock).toHaveBeenCalledWith("open_smart_mix", {
      pool: "library",
      options: { familiarity: 40, variety: 70, freshness: 60, count: 25, recentTrackIds: [] },
      seed: 42,
    });
    expect(invokeMock).not.toHaveBeenCalledWith("open_smart_mix", expect.objectContaining({ path: expect.anything() }));
  });
});
