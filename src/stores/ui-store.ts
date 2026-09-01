import { create } from "zustand";

import type { SearchResult, TrackId } from "../types/domain";

export type PlayerMode = "standard" | "mini" | "expanded";
export type InspectorState =
  | { kind: "closed" }
  | { kind: "track"; trackId: TrackId }
  | { kind: "search"; result: SearchResult };

interface UiState {
  commandPaletteOpen: boolean;
  queueDrawerOpen: boolean;
  playerMode: PlayerMode;
  inspector: InspectorState;
  setCommandPaletteOpen: (open: boolean) => void;
  setQueueDrawerOpen: (open: boolean) => void;
  setPlayerMode: (mode: PlayerMode) => void;
  openTrackInspector: (trackId: TrackId) => void;
  openSearchInspector: (result: SearchResult) => void;
  closeInspector: () => void;
  toggleCommandPalette: () => void;
  toggleQueueDrawer: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  commandPaletteOpen: false,
  queueDrawerOpen: false,
  playerMode: "standard",
  inspector: { kind: "closed" },
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setQueueDrawerOpen: (queueDrawerOpen) => set({ queueDrawerOpen }),
  setPlayerMode: (playerMode) => set({ playerMode }),
  openTrackInspector: (trackId) => set({ inspector: { kind: "track", trackId } }),
  openSearchInspector: (result) => set({ inspector: { kind: "search", result } }),
  closeInspector: () => set({ inspector: { kind: "closed" } }),
  toggleCommandPalette: () => set((state) => ({ commandPaletteOpen: !state.commandPaletteOpen })),
  toggleQueueDrawer: () => set((state) => ({ queueDrawerOpen: !state.queueDrawerOpen })),
}));
