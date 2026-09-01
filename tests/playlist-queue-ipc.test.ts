import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import {
  IpcError,
  createPlaylist,
  getQueueWorkspace,
  listPlaylists,
  moveQueueEntry,
  parseQueueWorkspace,
  saveQueueSnapshot,
  subscribeToQueueState,
} from "../src/services/ipc";
import type { Playlist, QueueSnapshot, QueueWorkspace } from "../src/types/domain";

const playlist: Playlist = {
  id: "playlist-1" as Playlist["id"],
  name: "Evening set",
  kind: "normal",
  parentPlaylistId: null,
  baseParentRevision: null,
  branchStatus: null,
  revision: 0,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  items: [],
};

const workspace: QueueWorkspace = {
  revision: 4,
  current: null,
  upNext: [],
  later: [{
    id: "queue-entry-1" as QueueWorkspace["later"][number]["id"],
    trackId: "track-1" as QueueWorkspace["later"][number]["trackId"],
    requestedSourceId: null,
    section: "later",
    position: 0,
    pinned: false,
    title: "Night Drive",
    artists: ["Luna Max"],
    album: "Afterglow",
  }],
  autoplay: [],
  currentPositionMs: 0,
  repeatMode: "off",
  shuffleEnabled: false,
};

const snapshot: QueueSnapshot = {
  id: "snapshot-1" as QueueSnapshot["id"],
  name: "Evening set",
  currentTrackId: null,
  currentSourceId: null,
  currentPositionMs: 0,
  repeatMode: "off",
  shuffleEnabled: false,
  entryCount: 0,
  currentSnapshotEntryId: null,
  historyOrder: [],
  traversalOrder: [],
  entries: [],
  createdAt: "2026-01-01T00:00:00Z",
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
});

describe("playlist and queue DTO validation", () => {
  it("forwards typed playlist and queue arguments", async () => {
    enableNativeRuntime();
    invokeMock
      .mockResolvedValueOnce([playlist])
      .mockResolvedValueOnce(playlist)
      .mockResolvedValueOnce(workspace)
      .mockResolvedValueOnce(snapshot);

    await expect(listPlaylists()).resolves.toEqual([playlist]);
    await expect(createPlaylist("  Evening set  ")).resolves.toEqual(playlist);
    await expect(moveQueueEntry("queue-entry-1" as QueueWorkspace["later"][number]["id"], "up_next", 0)).resolves.toEqual(workspace);
    await expect(saveQueueSnapshot("Evening set")).resolves.toEqual(snapshot);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "list_playlists");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "create_playlist", { name: "Evening set" });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "move_queue_entry", {
      entryId: "queue-entry-1",
      section: "up_next",
      targetIndex: 0,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, "save_queue_snapshot", { name: "Evening set" });
    for (const [, args] of invokeMock.mock.calls) {
      if (!args) {
        continue;
      }
      expect(args).not.toHaveProperty("path");
      expect(args).not.toHaveProperty("url");
    }
  });

  it("rejects unknown queue fields and malformed playlist responses", async () => {
    enableNativeRuntime();
    expect(() => parseQueueWorkspace({ ...workspace, rawUrl: "https://example.test/audio" })).toThrow();

    invokeMock.mockResolvedValueOnce([{ ...playlist, path: "C:\\Music\\track.wav" }]);
    await expect(listPlaylists()).rejects.toBeInstanceOf(IpcError);
  });
});

describe("queue state events", () => {
  it("reports malformed native events without passing them to the listener", async () => {
    enableNativeRuntime();
    let eventHandler: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation(async (_eventName: string, handler: (event: { payload: unknown }) => void) => {
      eventHandler = handler;
      return () => undefined;
    });
    const received: QueueWorkspace[] = [];
    const errors: Error[] = [];

    const unsubscribe = await subscribeToQueueState(
      (next) => received.push(next),
      (error) => errors.push(error),
    );
    eventHandler?.({ payload: workspace });
    eventHandler?.({ payload: { ...workspace, rawPath: "C:\\Music\\track.wav" } });

    expect(listenMock).toHaveBeenCalledWith("queue://state", expect.any(Function));
    expect(received).toEqual([workspace]);
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("invalid queue state event");
    unsubscribe();
  });

  it("returns the browser preview workspace without native IPC", async () => {
    await expect(getQueueWorkspace()).resolves.toMatchObject({
      revision: 0,
      current: null,
      autoplay: [],
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
