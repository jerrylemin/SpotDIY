import { create } from "zustand";

interface UiState {
  commandPaletteOpen: boolean;
  playerExpanded: boolean;
  setCommandPaletteOpen: (open: boolean) => void;
  setPlayerExpanded: (expanded: boolean) => void;
  toggleCommandPalette: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  commandPaletteOpen: false,
  playerExpanded: false,
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setPlayerExpanded: (playerExpanded) => set({ playerExpanded }),
  toggleCommandPalette: () => set((state) => ({ commandPaletteOpen: !state.commandPaletteOpen })),
}));
