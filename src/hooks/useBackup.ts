import { useCallback, useEffect, useState } from "react";

import {
  cancelSpotdiyImport,
  commitSpotdiyImport,
  exportSpotdiyBackup,
  getPendingImportPreview,
  getStorageStatus,
  IpcError,
  pickAndPrepareSpotdiyImport,
  prepareStorageModeSwitch,
} from "../services/ipc";
import type {
  ImportPreview,
  SpotDiyExportOptions,
  StorageMode,
  StorageModeSwitchResult,
  StorageStatus,
} from "../types/domain";

function errorMessage(error: unknown): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "SpotDIY could not update backup or storage settings.";
}

export function useBackup() {
  const [storage, setStorage] = useState<StorageStatus | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [nextStorage, nextPreview] = await Promise.all([
      getStorageStatus(),
      getPendingImportPreview(),
    ]);
    setStorage(nextStorage);
    setPreview(nextPreview);
  }, []);

  useEffect(() => {
    let active = true;
    void refresh().catch((loadError: unknown) => {
      if (active) {
        setError(errorMessage(loadError));
      }
    }).finally(() => {
      if (active) {
        setLoading(false);
      }
    });
    return () => {
      active = false;
    };
  }, [refresh]);

  const run = useCallback(async <T,>(action: () => Promise<T>): Promise<T> => {
    setBusy(true);
    setError(null);
    try {
      return await action();
    } catch (actionError) {
      setError(errorMessage(actionError));
      throw actionError;
    } finally {
      setBusy(false);
    }
  }, []);

  const exportBackup = useCallback((options: SpotDiyExportOptions) => run(async () => {
    await exportSpotdiyBackup(options);
  }), [run]);

  const prepareImport = useCallback(() => run(async () => {
    const nextPreview = await pickAndPrepareSpotdiyImport();
    setPreview(nextPreview);
    return nextPreview;
  }), [run]);

  const commitImport = useCallback((importId: string) => run(async () => {
    const result = await commitSpotdiyImport(importId);
    setPreview(result.preview);
    await refresh();
    return result;
  }), [refresh, run]);

  const cancelImport = useCallback((importId: string) => run(async () => {
    await cancelSpotdiyImport(importId);
    setPreview(null);
    await refresh();
  }), [refresh, run]);

  const switchMode = useCallback((mode: StorageMode): Promise<StorageModeSwitchResult> => run(async () => {
    const result = await prepareStorageModeSwitch(mode);
    await refresh();
    return result;
  }), [refresh, run]);

  return {
    storage,
    preview,
    loading,
    busy,
    error,
    clearError: useCallback(() => setError(null), []),
    refresh,
    exportBackup,
    prepareImport,
    commitImport,
    cancelImport,
    switchMode,
  };
}
