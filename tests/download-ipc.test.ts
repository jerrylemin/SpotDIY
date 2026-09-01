import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());
const openMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));

import {
  DOWNLOAD_STATE_EVENT,
  getDownloadSnapshot,
  parseDownloadSnapshot,
  queueSearchResultDownload,
  subscribeToDownloadState,
} from "../src/services/ipc";

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
}

function task() {
  return {
    id: "download-1",
    providerKind: "youtube",
    providerItemId: "video-1",
    canonicalUrl: "https://www.youtube.com/watch?v=video-1",
    targetTrackId: null,
    targetSourceId: null,
    title: "Fixture video",
    artists: ["Fixture artist"],
    artworkUrl: null,
    mode: "audio",
    state: "queued",
    destinationDirectory: "C:\\Downloads",
    outputPath: null,
    outputExtension: null,
    outputCodec: null,
    sourceQualityProvenance: "providerEncoded",
    transcoded: false,
    expectedBytes: null,
    downloadedBytes: 0,
    progressPermille: 0,
    speedBytesPerSecond: null,
    etaSeconds: null,
    retryCount: 0,
    errorCode: null,
    errorDetail: null,
    createdAt: "2026-09-01T00:00:00Z",
    updatedAt: "2026-09-01T00:00:00Z",
    startedAt: null,
    completedAt: null,
    outputMissing: false,
  };
}

function snapshot() {
  return {
    revision: 1,
    tasks: [task()],
    maxConcurrent: 2,
    downloadsDirectory: "C:\\Downloads",
    tools: {
      ytDlp: { status: "ready", version: "2026.08.19", detail: null },
      ffmpeg: { status: "missing", version: null, detail: "Install FFmpeg for video downloads." },
    },
  };
}

function result() {
  return {
    provider: "youtube" as const,
    entityKind: "track" as const,
    providerItemId: "video-1",
    canonicalUrl: "https://www.youtube.com/watch?v=video-1",
    title: "Fixture video",
    artists: ["Fixture artist"],
    album: null,
    durationMs: 4_000,
    artworkUrl: null,
    publishedAt: null,
    engagementCount: null,
    engagementKind: null,
    explicit: null,
    localTrackId: null,
    localSourceId: null,
    originalRank: 1,
  };
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
  listenMock.mockReset();
  openMock.mockReset();
});

describe("download IPC contracts", () => {
  it("returns an explicit native-unavailable browser preview snapshot", async () => {
    await expect(getDownloadSnapshot()).resolves.toMatchObject({
      tasks: [],
      downloadsDirectory: null,
      tools: { ytDlp: { status: "missing" }, ffmpeg: { status: "missing" } },
    });
  });

  it("parses the strict native snapshot and rejects unknown fields", () => {
    expect(parseDownloadSnapshot(snapshot()).tasks[0]?.providerKind).toBe("youtube");
    expect(() => parseDownloadSnapshot({ ...snapshot(), unexpected: true })).toThrow();
  });

  it("forwards a typed provider download request", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValueOnce(task());

    await expect(queueSearchResultDownload(result(), "video")).resolves.toMatchObject({
      id: "download-1",
      mode: "audio",
    });
    expect(invokeMock).toHaveBeenCalledWith("queue_search_result_download", {
      result: result(),
      mode: "video",
    });
  });

  it("validates download state events before they reach consumers", async () => {
    enableNativeRuntime();
    let handler: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation(async (eventName: string, next: (event: { payload: unknown }) => void) => {
      expect(eventName).toBe(DOWNLOAD_STATE_EVENT);
      handler = next;
      return () => undefined;
    });
    const received: unknown[] = [];
    const errors: Error[] = [];
    await subscribeToDownloadState((value) => received.push(value), (error) => errors.push(error));
    handler?.({ payload: snapshot() });
    handler?.({ payload: { ...snapshot(), debug: "raw" } });
    expect(received).toHaveLength(1);
    expect(errors[0]?.message).toContain("invalid download state event");
  });
});
