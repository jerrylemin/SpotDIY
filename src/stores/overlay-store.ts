import { create } from "zustand";

import type { WindowsIntegrationSnapshot } from "../types/domain";

interface OverlayState {
  snapshot: WindowsIntegrationSnapshot | null;
  setSnapshot: (snapshot: WindowsIntegrationSnapshot) => void;
}

export const useOverlayStore = create<OverlayState>((set) => ({
  snapshot: null,
  setSnapshot: (snapshot) => set((state) => (
    state.snapshot !== null && snapshot.revision < state.snapshot.revision
      ? state
      : { snapshot }
  )),
}));
