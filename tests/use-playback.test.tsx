import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const getPlaybackSnapshotMock = vi.hoisted(() => vi.fn());
const subscribeToPlaybackStateMock = vi.hoisted(() => vi.fn());

vi.mock("../src/services/ipc", () => ({
  IpcError: class IpcError extends Error {},
  clearPlaybackQueue: vi.fn(),
  enqueueTrack: vi.fn(),
  getAudioDevices: vi.fn(),
  getPlaybackSnapshot: getPlaybackSnapshotMock,
  nextTrack: vi.fn(),
  playTrack: vi.fn(),
  playTrackNext: vi.fn(),
  previousTrack: vi.fn(),
  retryPlaybackBackend: vi.fn(),
  seekPlayback: vi.fn(),
  setAudioDevice: vi.fn(),
  setPlaybackMuted: vi.fn(),
  setPlaybackVolume: vi.fn(),
  setRepeatMode: vi.fn(),
  setShuffleEnabled: vi.fn(),
  subscribeToPlaybackState: subscribeToPlaybackStateMock,
  switchPlaybackSource: vi.fn(),
  togglePlayPause: vi.fn(),
}));

import { usePlayback } from "../src/hooks/usePlayback";
import { usePlayerStore } from "../src/stores/player-store";

afterEach(() => {
  getPlaybackSnapshotMock.mockReset();
  subscribeToPlaybackStateMock.mockReset();
  usePlayerStore.getState().reset();
});

describe("usePlayback bridge initialization", () => {
  it("subscribes before fetching the initial snapshot", async () => {
    const calls: string[] = [];
    const snapshot = usePlayerStore.getState().snapshot;
    subscribeToPlaybackStateMock.mockImplementation(async () => {
      calls.push("subscribe");
      return () => undefined;
    });
    getPlaybackSnapshotMock.mockImplementation(async () => {
      calls.push("snapshot");
      return { ...snapshot, revision: 1 };
    });

    const { unmount } = renderHook(() => usePlayback());

    await waitFor(() => expect(getPlaybackSnapshotMock).toHaveBeenCalledOnce());
    expect(calls).toEqual(["subscribe", "snapshot"]);
    unmount();
  });

  it("re-subscribes when a remount overlaps initial snapshot loading", async () => {
    let resolveFirstSnapshot: ((value: ReturnType<typeof usePlayerStore.getState>["snapshot"]) => void) | undefined;
    const firstSnapshot = new Promise<ReturnType<typeof usePlayerStore.getState>["snapshot"]>((resolve) => {
      resolveFirstSnapshot = resolve;
    });
    const snapshot = usePlayerStore.getState().snapshot;
    const unsubscribe = vi.fn();
    subscribeToPlaybackStateMock
      .mockResolvedValueOnce(unsubscribe)
      .mockResolvedValueOnce(vi.fn());
    getPlaybackSnapshotMock
      .mockReturnValueOnce(firstSnapshot)
      .mockResolvedValueOnce({ ...snapshot, revision: 2 });

    const first = renderHook(() => usePlayback());
    await waitFor(() => expect(getPlaybackSnapshotMock).toHaveBeenCalledOnce());

    first.unmount();
    const second = renderHook(() => usePlayback());
    expect(unsubscribe).toHaveBeenCalledOnce();

    resolveFirstSnapshot?.({ ...snapshot, revision: 1 });
    await waitFor(() => expect(subscribeToPlaybackStateMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(getPlaybackSnapshotMock).toHaveBeenCalledTimes(2));
    second.unmount();
  });
});
