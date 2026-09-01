import { create } from "zustand";

import type { PlaybackAudioDevice, PlaybackSnapshot } from "../types/domain";

function emptyPlaybackSnapshot(): PlaybackSnapshot {
  return {
    revision: 0,
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
    queueLength: 0,
    queueIndex: null,
    selectedAudioDevice: "auto",
    backendHealth: {
      ready: false,
      connected: false,
      detail: null,
      recoveryAction: null,
    },
    recovering: false,
    error: null,
    abLoop: {
      aMs: null,
      bMs: null,
      active: false,
    },
  };
}

interface PlayerStoreState {
  snapshot: PlaybackSnapshot;
  audioDevices: PlaybackAudioDevice[];
  hydrated: boolean;
  initializing: boolean;
  audioDevicesLoading: boolean;
  pendingCount: number;
  bridgeError: string | null;
  setSnapshot: (snapshot: PlaybackSnapshot) => void;
  setAudioDevices: (audioDevices: PlaybackAudioDevice[]) => void;
  setHydrated: (hydrated: boolean) => void;
  setInitializing: (initializing: boolean) => void;
  setAudioDevicesLoading: (loading: boolean) => void;
  setBridgeError: (bridgeError: string | null) => void;
  beginAction: () => void;
  endAction: () => void;
  reset: () => void;
}

export const usePlayerStore = create<PlayerStoreState>((set) => ({
  snapshot: emptyPlaybackSnapshot(),
  audioDevices: [],
  hydrated: false,
  initializing: false,
  audioDevicesLoading: false,
  pendingCount: 0,
  bridgeError: null,
  setSnapshot: (snapshot) => set((state) => (
    snapshot.revision >= state.snapshot.revision
      ? { snapshot, hydrated: true, bridgeError: null }
      : state
  )),
  setAudioDevices: (audioDevices) => set({ audioDevices }),
  setHydrated: (hydrated) => set({ hydrated }),
  setInitializing: (initializing) => set({ initializing }),
  setAudioDevicesLoading: (audioDevicesLoading) => set({ audioDevicesLoading }),
  setBridgeError: (bridgeError) => set({ bridgeError }),
  beginAction: () => set((state) => ({ pendingCount: state.pendingCount + 1 })),
  endAction: () => set((state) => ({ pendingCount: Math.max(0, state.pendingCount - 1) })),
  reset: () => set({
    snapshot: emptyPlaybackSnapshot(),
    audioDevices: [],
    hydrated: false,
    initializing: false,
    audioDevicesLoading: false,
    pendingCount: 0,
    bridgeError: null,
  }),
}));
