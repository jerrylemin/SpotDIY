import { useCallback, useEffect } from "react";

import {
  clearPlaybackQueue,
  enqueueTrack,
  getAudioDevices,
  getPlaybackSnapshot,
  nextTrack,
  playTrack,
  playTrackNext,
  previousTrack,
  retryPlaybackBackend,
  seekPlayback,
  setAudioDevice,
  setPlaybackMuted,
  setPlaybackVolume,
  setRepeatMode,
  setShuffleEnabled,
  subscribeToPlaybackState,
  switchPlaybackSource,
  togglePlayPause,
  IpcError,
} from "../services/ipc";
import { usePlayerStore } from "../stores/player-store";
import type { RepeatMode, SourceId, TrackId } from "../types/domain";

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

let bridgeConsumers = 0;
let bridgeUnsubscribe: (() => void) | null = null;
let bridgeInitialization: Promise<void> | null = null;

async function refreshPlaybackSnapshot() {
  const store = usePlayerStore.getState();
  store.setInitializing(true);
  try {
    store.setSnapshot(await getPlaybackSnapshot());
    store.setBridgeError(null);
  } catch (error) {
    store.setBridgeError(errorMessage(error, "SpotDIY could not read the playback state."));
  } finally {
    store.setInitializing(false);
  }
}

async function ensurePlaybackBridge() {
  if (bridgeInitialization) {
    return bridgeInitialization;
  }

  bridgeInitialization = (async () => {
    await refreshPlaybackSnapshot();
    if (!bridgeUnsubscribe) {
      bridgeUnsubscribe = await subscribeToPlaybackState((snapshot) => {
        usePlayerStore.getState().setSnapshot(snapshot);
      });
    }
  })()
    .catch((error) => {
      usePlayerStore.getState().setBridgeError(errorMessage(error, "SpotDIY could not subscribe to playback updates."));
    })
    .finally(() => {
      bridgeInitialization = null;
    });

  return bridgeInitialization;
}

function releasePlaybackBridge() {
  if (bridgeConsumers > 0) {
    return;
  }
  bridgeUnsubscribe?.();
  bridgeUnsubscribe = null;
}

export function usePlayback() {
  const snapshot = usePlayerStore((state) => state.snapshot);
  const audioDevices = usePlayerStore((state) => state.audioDevices);
  const hydrated = usePlayerStore((state) => state.hydrated);
  const initializing = usePlayerStore((state) => state.initializing);
  const audioDevicesLoading = usePlayerStore((state) => state.audioDevicesLoading);
  const pendingCount = usePlayerStore((state) => state.pendingCount);
  const bridgeError = usePlayerStore((state) => state.bridgeError);

  useEffect(() => {
    bridgeConsumers += 1;
    void ensurePlaybackBridge();
    return () => {
      bridgeConsumers = Math.max(0, bridgeConsumers - 1);
      releasePlaybackBridge();
    };
  }, []);

  const runSnapshotAction = useCallback(
    async (action: () => Promise<typeof snapshot>, fallbackMessage: string) => {
      const store = usePlayerStore.getState();
      store.beginAction();
      try {
        const next = await action();
        store.setSnapshot(next);
        store.setBridgeError(null);
        return next;
      } catch (error) {
        store.setBridgeError(errorMessage(error, fallbackMessage));
        throw error;
      } finally {
        store.endAction();
      }
    },
    [],
  );

  const runSideEffect = useCallback(
    async <T,>(action: () => Promise<T>, fallbackMessage: string) => {
      const store = usePlayerStore.getState();
      store.beginAction();
      try {
        const result = await action();
        store.setBridgeError(null);
        return result;
      } catch (error) {
        store.setBridgeError(errorMessage(error, fallbackMessage));
        throw error;
      } finally {
        store.endAction();
      }
    },
    [],
  );

  const refreshDevices = useCallback(async () => {
    const store = usePlayerStore.getState();
    store.setAudioDevicesLoading(true);
    try {
      store.setAudioDevices(await getAudioDevices());
      store.setBridgeError(null);
    } catch (error) {
      store.setBridgeError(errorMessage(error, "SpotDIY could not read the playback audio devices."));
      throw error;
    } finally {
      store.setAudioDevicesLoading(false);
    }
  }, []);

  const cycleRepeatMode = useCallback(async () => {
    const nextRepeatMode: RepeatMode = snapshot.repeatMode === "off"
      ? "one"
      : snapshot.repeatMode === "one"
        ? "all"
        : "off";
    return runSnapshotAction(
      () => setRepeatMode(nextRepeatMode),
      "SpotDIY could not update repeat mode.",
    );
  }, [runSnapshotAction, snapshot.repeatMode]);

  const playNow = useCallback((trackId: TrackId, sourceId: SourceId | null) => runSnapshotAction(
    () => playTrack({ trackId, sourceId }),
    "SpotDIY could not start playback for that track.",
  ), [runSnapshotAction]);

  const addToQueue = useCallback((trackId: TrackId, sourceId: SourceId | null) => runSnapshotAction(
    () => enqueueTrack({ trackId, sourceId }),
    "SpotDIY could not add that track to the queue.",
  ), [runSnapshotAction]);

  const playNext = useCallback((trackId: TrackId, sourceId: SourceId | null) => runSnapshotAction(
    () => playTrackNext({ trackId, sourceId }),
    "SpotDIY could not queue that track to play next.",
  ), [runSnapshotAction]);

  const toggleMuted = useCallback(() => runSnapshotAction(
    () => setPlaybackMuted(!snapshot.muted),
    "SpotDIY could not update the mute state.",
  ), [runSnapshotAction, snapshot.muted]);

  const toggleShuffle = useCallback(() => runSnapshotAction(
    () => setShuffleEnabled(!snapshot.shuffleEnabled),
    "SpotDIY could not update shuffle mode.",
  ), [runSnapshotAction, snapshot.shuffleEnabled]);

  const switchSource = useCallback((trackId: TrackId, sourceId: SourceId) => runSnapshotAction(
    () => switchPlaybackSource({ trackId, sourceId }),
    "SpotDIY could not switch playback sources.",
  ), [runSnapshotAction]);

  return {
    snapshot,
    audioDevices,
    hydrated,
    initializing,
    audioDevicesLoading,
    bridgeError,
    pending: pendingCount > 0,
    refreshSnapshot: useCallback(() => runSnapshotAction(
      getPlaybackSnapshot,
      "SpotDIY could not refresh the playback state.",
    ), [runSnapshotAction]),
    refreshDevices,
    playNow,
    addToQueue,
    playNext,
    togglePlayPause: useCallback(() => runSnapshotAction(
      togglePlayPause,
      "SpotDIY could not toggle playback.",
    ), [runSnapshotAction]),
    nextTrack: useCallback(() => runSnapshotAction(
      nextTrack,
      "SpotDIY could not skip to the next track.",
    ), [runSnapshotAction]),
    previousTrack: useCallback(() => runSnapshotAction(
      previousTrack,
      "SpotDIY could not return to the previous track.",
    ), [runSnapshotAction]),
    seekPlayback: useCallback((positionMs: number) => runSnapshotAction(
      () => seekPlayback(positionMs),
      "SpotDIY could not seek within the current track.",
    ), [runSnapshotAction]),
    setVolume: useCallback((volumePercent: number) => runSnapshotAction(
      () => setPlaybackVolume(volumePercent),
      "SpotDIY could not update the playback volume.",
    ), [runSnapshotAction]),
    setMuted: useCallback((muted: boolean) => runSnapshotAction(
      () => setPlaybackMuted(muted),
      "SpotDIY could not update the mute state.",
    ), [runSnapshotAction]),
    toggleMuted,
    cycleRepeatMode,
    setShuffleEnabled: useCallback((enabled: boolean) => runSnapshotAction(
      () => setShuffleEnabled(enabled),
      "SpotDIY could not update shuffle mode.",
    ), [runSnapshotAction]),
    toggleShuffle,
    setAudioDevice: useCallback((name: string) => runSnapshotAction(
      () => setAudioDevice(name),
      "SpotDIY could not switch the audio device.",
    ), [runSnapshotAction]),
    switchSource,
    retryPlaybackBackend: useCallback(() => runSnapshotAction(
      retryPlaybackBackend,
      "SpotDIY could not retry the playback backend.",
    ), [runSnapshotAction]),
    clearQueue: useCallback(() => runSnapshotAction(
      clearPlaybackQueue,
      "SpotDIY could not clear the playback queue.",
    ), [runSnapshotAction]),
    warmAudioDevices: useCallback(() => runSideEffect(
      async () => {
        if (audioDevices.length === 0 && !audioDevicesLoading) {
          await refreshDevices();
        }
      },
      "SpotDIY could not read the playback audio devices.",
    ), [audioDevices.length, audioDevicesLoading, refreshDevices, runSideEffect]),
  };
}
