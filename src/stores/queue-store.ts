import { create } from "zustand";

import type { QueueWorkspace } from "../types/domain";

export function emptyQueueWorkspace(): QueueWorkspace {
  return {
    revision: 0,
    current: null,
    upNext: [],
    later: [],
    autoplay: [],
    currentPositionMs: 0,
    repeatMode: "off",
    shuffleEnabled: false,
  };
}

interface QueueStoreState {
  workspace: QueueWorkspace;
  hydrated: boolean;
  initializing: boolean;
  error: string | null;
  setWorkspace: (workspace: QueueWorkspace) => void;
  setInitializing: (initializing: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

export const useQueueStore = create<QueueStoreState>((set) => ({
  workspace: emptyQueueWorkspace(),
  hydrated: false,
  initializing: false,
  error: null,
  setWorkspace: (workspace) => set((state) => (
    workspace.revision >= state.workspace.revision
      ? { workspace, hydrated: true, error: null }
      : state
  )),
  setInitializing: (initializing) => set({ initializing }),
  setError: (error) => set({ error }),
  reset: () => set({ workspace: emptyQueueWorkspace(), hydrated: false, initializing: false, error: null }),
}));
