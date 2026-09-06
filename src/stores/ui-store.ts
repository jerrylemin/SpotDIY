import { create } from "zustand";

import type { ProviderKind, ProviderSearchSection, SearchLens, SearchResult, SearchSortDirection, SearchSortField, TrackId } from "../types/domain";

export type PlayerMode = "standard" | "mini" | "expanded";
export type InspectorState =
  | { kind: "closed" }
  | { kind: "track"; trackId: TrackId }
  | { kind: "search"; result: SearchResult };

export interface SearchWorkspaceState {
  query: string;
  lens: SearchLens;
  sortField: SearchSortField;
  sortDirection: SearchSortDirection;
  sections: Partial<Record<ProviderKind, ProviderSearchSection>>;
  searchKey: string;
  completed: boolean;
}

const initialSearchWorkspace: SearchWorkspaceState = {
  query: "",
  lens: "all",
  sortField: "relevance",
  sortDirection: "descending",
  sections: {},
  searchKey: "",
  completed: false,
};

interface UiState {
  commandPaletteOpen: boolean;
  queueDrawerOpen: boolean;
  playerMode: PlayerMode;
  inspector: InspectorState;
  searchWorkspace: SearchWorkspaceState;
  setCommandPaletteOpen: (open: boolean) => void;
  setQueueDrawerOpen: (open: boolean) => void;
  setPlayerMode: (mode: PlayerMode) => void;
  openTrackInspector: (trackId: TrackId) => void;
  openSearchInspector: (result: SearchResult) => void;
  closeInspector: () => void;
  toggleCommandPalette: () => void;
  toggleQueueDrawer: () => void;
  setSearchWorkspace: (workspace: Partial<SearchWorkspaceState>) => void;
}

export const useUiStore = create<UiState>((set) => ({
  commandPaletteOpen: false,
  queueDrawerOpen: false,
  playerMode: "standard",
  inspector: { kind: "closed" },
  searchWorkspace: initialSearchWorkspace,
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setQueueDrawerOpen: (queueDrawerOpen) => set({ queueDrawerOpen }),
  setPlayerMode: (playerMode) => set({ playerMode }),
  openTrackInspector: (trackId) => set({ inspector: { kind: "track", trackId } }),
  openSearchInspector: (result) => set({ inspector: { kind: "search", result } }),
  closeInspector: () => set({ inspector: { kind: "closed" } }),
  toggleCommandPalette: () => set((state) => ({ commandPaletteOpen: !state.commandPaletteOpen })),
  toggleQueueDrawer: () => set((state) => ({ queueDrawerOpen: !state.queueDrawerOpen })),
  setSearchWorkspace: (workspace) => set((state) => ({ searchWorkspace: { ...state.searchWorkspace, ...workspace } })),
}));
