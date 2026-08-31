import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import {
  enqueueTrack,
  getAudioDevices,
  parsePlaybackAudioDevices,
  parsePlaybackSnapshot,
  playTrack,
  subscribeToPlaybackState,
  switchPlaybackSource,
} from "../src/services/ipc";
import { usePlayerStore } from "../src/stores/player-store";
import type { PlaybackSnapshot, TrackId } from "../src/types/domain";

const snapshot: PlaybackSnapshot = {
  revision: 7,
  phase: "paused",
  currentQueueEntryId: "queue-entry-1" as PlaybackSnapshot["currentQueueEntryId"],
  currentTrackId: "track-1" as TrackId,
  currentSourceId: "source-1" as PlaybackSnapshot["currentSourceId"],
  title: "Night Drive",
  artists: ["Luna Max"],
  album: "Afterglow",
  artworkPath: null,
  sources: [{ sourceId: "source-1" as PlaybackSnapshot["currentSourceId"] & string, provider: "local", label: "LOCAL", available: true }],
  positionMs: 1_250,
  durationMs: 185_000,
  volumePercent: 72,
  muted: false,
  repeatMode: "off",
  shuffleEnabled: false,
  queueLength: 1,
  queueIndex: 0,
  selectedAudioDevice: "auto",
  backendHealth: { ready: true, connected: true, detail: null, recoveryAction: null },
  recovering: false,
  error: null,
};

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
  listenMock.mockReset();
  usePlayerStore.getState().reset();
});

describe("playback DTO validation", () => {
  it("accepts the exact snapshot shape and excludes raw path fields", () => {
    const parsed = parsePlaybackSnapshot(snapshot);
    expect(parsed).toEqual(snapshot);
    expect(parsed).not.toHaveProperty("path");
    expect(parsed).not.toHaveProperty("audioDevice");
  });

  it("rejects malformed snapshots and audio-device records", () => {
    expect(() => parsePlaybackSnapshot({ ...snapshot, durationMs: "185000" })).toThrow();
    expect(() => parsePlaybackSnapshot({ ...snapshot, path: "C:\\Music\\song.wav" })).toThrow();
    expect(() => parsePlaybackAudioDevices([{ name: "auto", description: "Default", isDefault: true }])).toThrow();
    expect(parsePlaybackAudioDevices([
      { name: "auto", description: "Default output", selected: true },
    ])).toEqual([{ name: "auto", description: "Default output", selected: true }]);
  });
});

describe("typed playback commands", () => {
  it("forwards only track and source IDs for play and queue operations", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValue(snapshot);
    const request = { trackId: "track-1" as TrackId, sourceId: "source-1" as PlaybackSnapshot["currentSourceId"] & string };

    await playTrack(request);
    await enqueueTrack(request);
    await switchPlaybackSource(request);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "play_track", request);
    expect(invokeMock).toHaveBeenNthCalledWith(2, "enqueue_track", request);
    expect(invokeMock).toHaveBeenNthCalledWith(3, "switch_playback_source", request);
    for (const [, args] of invokeMock.mock.calls) {
      expect(args).not.toHaveProperty("path");
      expect(args).not.toHaveProperty("command");
    }
  });

  it("validates native device responses", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValue([{ name: "auto", description: "Default output", selected: true }]);

    await expect(getAudioDevices()).resolves.toEqual([
      { name: "auto", description: "Default output", selected: true },
    ]);
  });

  it("validates structured native playback errors before surfacing them", async () => {
    enableNativeRuntime();
    invokeMock.mockRejectedValue({ code: "queueEmpty", summary: "the queue is empty", retryable: false });

    await expect(playTrack({
      trackId: "track-1" as TrackId,
      sourceId: "source-1" as PlaybackSnapshot["currentSourceId"] & string,
    })).rejects.toMatchObject({ message: "the queue is empty" });
  });
});

describe("playback state events", () => {
  it("parses valid events, ignores malformed events, and keeps newer revisions", async () => {
    enableNativeRuntime();
    let eventHandler: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation(async (_eventName: string, handler: (event: { payload: unknown }) => void) => {
      eventHandler = handler;
      return () => undefined;
    });
    const received: PlaybackSnapshot[] = [];
    const errors: Error[] = [];
    const unsubscribe = await subscribeToPlaybackState(
      (next) => received.push(next),
      (error) => errors.push(error),
    );

    eventHandler?.({ payload: { ...snapshot, revision: 8 } });
    eventHandler?.({ payload: { ...snapshot, revision: 9, error: { code: "protocolError", summary: "bad frame", retryable: true } } });
    eventHandler?.({ payload: { ...snapshot, revision: 10, path: "C:\\Music\\raw.wav" } });

    expect(received).toHaveLength(2);
    expect(received.at(-1)?.revision).toBe(9);
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("invalid playback state event");
    unsubscribe();

    usePlayerStore.getState().setSnapshot(snapshot);
    usePlayerStore.getState().setSnapshot({ ...snapshot, revision: 6, title: "stale" });
    expect(usePlayerStore.getState().snapshot.title).toBe("Night Drive");
  });
});
