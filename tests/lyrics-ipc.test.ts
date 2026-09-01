import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  IpcError,
  findLrclibBest,
  getLyrics,
  searchLrclib,
} from "../src/services/ipc";
import { activeCueIndex } from "../src/hooks/useLyrics";
import type { LyricsDocument, SourceId, TrackId } from "../src/types/domain";

const trackId = "track-lyrics" as TrackId;
const document: LyricsDocument = {
  trackId,
  source: "sidecar",
  syncKind: "timed",
  plainText: "First synthetic line\nSecond synthetic line",
  cues: [
    { startMs: 1_000, lines: ["First synthetic line"] },
    { startMs: 2_500, lines: ["Second synthetic line"] },
  ],
  instrumental: false,
  editable: false,
  attribution: null,
};

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
});

describe("lyrics IPC contracts", () => {
  it("loads a typed document without accepting a path argument", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValue(document);

    await expect(getLyrics(trackId, "source-lyrics" as SourceId)).resolves.toEqual(document);
    expect(invokeMock).toHaveBeenCalledWith("get_lyrics", {
      trackId,
      currentSourceId: "source-lyrics",
    });
    expect(invokeMock.mock.calls[0][1]).not.toHaveProperty("path");
  });

  it("accepts only metadata-only LRCLIB candidates", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValue([
      {
        providerRecordId: 7,
        trackName: "Synthetic Track",
        artistName: "Synthetic Artist",
        albumName: null,
        durationMs: 181_000,
        instrumental: false,
        hasPlain: true,
        hasSynced: false,
      },
    ]);

    await expect(searchLrclib(trackId)).resolves.toMatchObject([{ providerRecordId: 7, hasPlain: true }]);
    invokeMock.mockResolvedValue([{ providerRecordId: 7, trackName: "Synthetic Track", artistName: "Synthetic Artist", albumName: null, durationMs: 181_000, instrumental: false, hasPlain: true, hasSynced: false, plainLyrics: "not allowed here" }]);
    await expect(searchLrclib(trackId)).rejects.toBeInstanceOf(IpcError);
  });

  it("surfaces typed provider errors and does not invent a browser result", async () => {
    enableNativeRuntime();
    invokeMock.mockRejectedValue({ code: "rateLimited", detail: "LRCLIB rate limit exceeded", retryAfterSeconds: 4 });

    await expect(findLrclibBest(trackId)).rejects.toMatchObject({ message: "LRCLIB rate limit exceeded" });
    await expect(getLyrics(trackId)).rejects.toBeInstanceOf(IpcError);
  });
});

describe("client-side timed lyric synchronization", () => {
  it("uses binary-search semantics at cue boundaries", () => {
    expect(activeCueIndex(document.cues, 0)).toBe(-1);
    expect(activeCueIndex(document.cues, 999)).toBe(-1);
    expect(activeCueIndex(document.cues, 1_000)).toBe(0);
    expect(activeCueIndex(document.cues, 2_499)).toBe(0);
    expect(activeCueIndex(document.cues, 2_500)).toBe(1);
    expect(activeCueIndex(document.cues, 99_000)).toBe(1);
  });
});
