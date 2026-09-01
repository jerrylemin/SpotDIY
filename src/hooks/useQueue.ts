import { useCallback, useEffect } from "react";

import {
  clearQueueSection,
  deleteQueueSnapshot,
  getQueueWorkspace,
  IpcError,
  listQueueSnapshots,
  moveQueueEntry,
  removeQueueEntry,
  restoreQueueSnapshot,
  saveQueueSnapshot,
  setQueueEntryPinned,
  subscribeToQueueState,
} from "../services/ipc";
import { useQueueStore } from "../stores/queue-store";
import type {
  QueueEntryId,
  QueueSection,
  QueueSnapshot,
  QueueSnapshotId,
  QueueSnapshotSummary,
  QueueWorkspace,
} from "../types/domain";

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

async function refreshQueueWorkspace() {
  const store = useQueueStore.getState();
  store.setInitializing(true);
  try {
    store.setWorkspace(await getQueueWorkspace());
  } catch (error) {
    store.setError(errorMessage(error, "SpotDIY could not read the queue workspace."));
  } finally {
    store.setInitializing(false);
  }
}

async function ensureQueueBridge() {
  if (bridgeInitialization) {
    await bridgeInitialization;
    return;
  }

  bridgeInitialization = (async () => {
    const unsubscribe = await subscribeToQueueState(
      (workspace) => useQueueStore.getState().setWorkspace(workspace),
      (error) => useQueueStore.getState().setError(error.message),
    );
    if (bridgeConsumers === 0) {
      unsubscribe();
      return;
    }
    bridgeUnsubscribe = unsubscribe;
    await refreshQueueWorkspace();
  })()
    .catch((error) => {
      useQueueStore.getState().setError(errorMessage(error, "SpotDIY could not subscribe to queue updates."));
    })
    .finally(() => {
      bridgeInitialization = null;
    });

  await bridgeInitialization;
}

function releaseQueueBridge() {
  if (bridgeConsumers > 0) {
    return;
  }
  bridgeUnsubscribe?.();
  bridgeUnsubscribe = null;
}

export function useQueue() {
  const workspace = useQueueStore((state) => state.workspace);
  const hydrated = useQueueStore((state) => state.hydrated);
  const initializing = useQueueStore((state) => state.initializing);
  const error = useQueueStore((state) => state.error);

  useEffect(() => {
    bridgeConsumers += 1;
    void ensureQueueBridge();
    return () => {
      bridgeConsumers = Math.max(0, bridgeConsumers - 1);
      releaseQueueBridge();
    };
  }, []);

  const runCommand = useCallback(
    async <T,>(action: () => Promise<T>, fallback: string): Promise<T> => {
      try {
        const result = await action();
        useQueueStore.getState().setError(null);
        return result;
      } catch (actionError) {
        useQueueStore.getState().setError(errorMessage(actionError, fallback));
        throw actionError;
      }
    },
    [],
  );
  const runWorkspaceAction = useCallback(
    async (action: () => Promise<QueueWorkspace>, fallback: string) => {
      const next = await runCommand(action, fallback);
      useQueueStore.getState().setWorkspace(next);
      return next;
    },
    [runCommand],
  );

  return {
    workspace,
    hydrated,
    initializing,
    error,
    refresh: useCallback(() => refreshQueueWorkspace(), []),
    moveEntry: useCallback((entryId: QueueEntryId, section: QueueSection, targetIndex: number) => runWorkspaceAction(
      () => moveQueueEntry(entryId, section, targetIndex),
      "SpotDIY could not move that queue entry.",
    ), [runWorkspaceAction]),
    removeEntry: useCallback((entryId: QueueEntryId) => runWorkspaceAction(
      () => removeQueueEntry(entryId),
      "SpotDIY could not remove that queue entry.",
    ), [runWorkspaceAction]),
    setEntryPinned: useCallback((entryId: QueueEntryId, pinned: boolean) => runWorkspaceAction(
      () => setQueueEntryPinned(entryId, pinned),
      "SpotDIY could not update that queue pin.",
    ), [runWorkspaceAction]),
    clearSection: useCallback((section: QueueSection) => runWorkspaceAction(
      () => clearQueueSection(section),
      "SpotDIY could not clear that queue section.",
    ), [runWorkspaceAction]),
    saveSnapshot: useCallback((name: string) => runCommand<QueueSnapshot>(
      () => saveQueueSnapshot(name),
      "SpotDIY could not save the queue snapshot.",
    ), [runCommand]),
    listSnapshots: useCallback(() => runCommand<QueueSnapshotSummary[]>(
      listQueueSnapshots,
      "SpotDIY could not read queue snapshots.",
    ), [runCommand]),
    restoreSnapshot: useCallback((snapshotId: QueueSnapshotId) => runCommand(
      async () => {
        const result = await restoreQueueSnapshot(snapshotId);
        await refreshQueueWorkspace();
        return result;
      },
      "SpotDIY could not restore the queue snapshot.",
    ), [runCommand]),
    deleteSnapshot: useCallback((snapshotId: QueueSnapshotId) => runCommand<QueueSnapshotSummary[]>(
      () => deleteQueueSnapshot(snapshotId),
      "SpotDIY could not delete the queue snapshot.",
    ), [runCommand]),
  };
}
