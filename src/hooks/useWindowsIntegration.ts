import { useCallback, useEffect, useState } from "react";

import {
  applyOutputProfile,
  closeOverlay,
  createOutputProfile,
  deleteOutputProfile,
  getWindowsIntegrationSnapshot,
  isTauriRuntime,
  listOutputProfiles,
  openOverlay,
  resetGlobalShortcuts,
  setGamingClickThrough,
  setGlobalShortcutsEnabled,
  setWindowsIntegrationSettings,
  subscribeToWindowsIntegrationState,
  toggleOverlay,
  updateGlobalShortcut,
  updateOutputProfile,
  IpcError,
} from "../services/ipc";
import { useOverlayStore } from "../stores/overlay-store";
import type {
  GlobalShortcutBinding,
  OutputProfile,
  OverlayKind,
  WindowsIntegrationSnapshot,
  WindowsIntegrationSettings,
} from "../types/domain";

function errorMessage(error: unknown): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "Windows integration could not be updated.";
}

export function useWindowsIntegration() {
  const snapshot = useOverlayStore((state) => state.snapshot);
  const setSnapshot = useOverlayStore((state) => state.setSnapshot);
  const [loading, setLoading] = useState(snapshot === null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    setLoading(useOverlayStore.getState().snapshot === null);
    void subscribeToWindowsIntegrationState(
      (next) => {
        if (active) {
          setSnapshot(next);
        }
      },
      (eventError) => {
        if (active) {
          setError(eventError.message);
        }
      },
    ).then((stop) => {
      if (active) {
        unsubscribe = stop;
      } else {
        stop();
      }
    }).catch((eventError: unknown) => {
      if (active) {
        setError(errorMessage(eventError));
      }
    });
    void getWindowsIntegrationSnapshot().then((next) => {
      if (active) {
        setSnapshot(next);
        setLoading(false);
      }
    }).catch((loadError: unknown) => {
      if (active) {
        setError(errorMessage(loadError));
        setLoading(false);
      }
    });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [setSnapshot]);

  const run = useCallback(async <T,>(action: () => Promise<T>) => {
    setError(null);
    try {
      return await action();
    } catch (actionError) {
      setError(errorMessage(actionError));
      throw actionError;
    }
  }, []);

  const withSnapshot = useCallback((action: () => Promise<WindowsIntegrationSnapshot>) => run(async () => {
    const next = await action();
    setSnapshot(next);
    return next;
  }), [run, setSnapshot]);

  const showMain = useCallback(async () => {
    if (typeof window === "undefined" || !isTauriRuntime()) {
      return;
    }
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const currentWindow = getCurrentWindow();
    await currentWindow.show();
    await currentWindow.setFocus();
  }, []);

  return {
    snapshot,
    loading,
    error,
    clearError: useCallback(() => setError(null), []),
    showMain,
    setWindowsIntegrationSettings: (settings: WindowsIntegrationSettings) => withSnapshot(() => setWindowsIntegrationSettings(settings)),
    setGlobalShortcutsEnabled: (enabled: boolean) => withSnapshot(() => setGlobalShortcutsEnabled(enabled)),
    updateGlobalShortcut: (binding: GlobalShortcutBinding) => withSnapshot(() => updateGlobalShortcut(binding)),
    resetGlobalShortcuts: () => withSnapshot(resetGlobalShortcuts),
    openOverlay: (kind: OverlayKind) => withSnapshot(() => openOverlay(kind)),
    closeOverlay: (kind: OverlayKind) => withSnapshot(() => closeOverlay(kind)),
    toggleOverlay: (kind: OverlayKind) => withSnapshot(() => toggleOverlay(kind)),
    setGamingClickThrough: (enabled: boolean) => withSnapshot(() => setGamingClickThrough(enabled)),
    listOutputProfiles,
    createOutputProfile: (name: string) => withSnapshot(() => createOutputProfile(name)),
    updateOutputProfile: (profile: OutputProfile) => withSnapshot(() => updateOutputProfile(profile)),
    deleteOutputProfile: (id: string) => withSnapshot(() => deleteOutputProfile(id)),
    applyOutputProfile,
  };
}
