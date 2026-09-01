import { create } from "zustand";

interface UiState {
  commandPaletteOpen: boolean;
  queueDrawerOpen: boolean;
  playerExpanded: boolean;
  setCommandPaletteOpen: (open: boolean) => void;
  setQueueDrawerOpen: (open: boolean) => void;
  setPlayerExpanded: (expanded: boolean) => void;
  toggleCommandPalette: () => void;
  toggleQueueDrawer: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  commandPaletteOpen: false,
  queueDrawerOpen: false,
  playerExpanded: false,
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setQueueDrawerOpen: (queueDrawerOpen) => set({ queueDrawerOpen }),
  setPlayerExpanded: (playerExpanded) => set({ playerExpanded }),
  toggleCommandPalette: () => set((state) => ({ commandPaletteOpen: !state.commandPaletteOpen })),
  toggleQueueDrawer: () => set((state) => ({ queueDrawerOpen: !state.queueDrawerOpen })),
}));
