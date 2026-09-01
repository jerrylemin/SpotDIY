import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import {
  cancelDownload,
  getDownloadSnapshot,
  isTauriRuntime,
  retryDownload,
  setDownloadConcurrency,
  subscribeToDownloadState,
} from "../services/ipc";
import type { DownloadSnapshot, DownloadTaskId } from "../types/domain";

export const DOWNLOAD_SNAPSHOT_QUERY_KEY = ["download-snapshot"] as const;

export function useDownloadSnapshot() {
  const queryClient = useQueryClient();
  const [eventError, setEventError] = useState<string | null>(null);
  const query = useQuery({
    queryKey: DOWNLOAD_SNAPSHOT_QUERY_KEY,
    queryFn: getDownloadSnapshot,
    retry: 1,
  });

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }
    let mounted = true;
    let unsubscribe: (() => void) | undefined;
    void subscribeToDownloadState(
      (snapshot) => {
        if (mounted) {
          setEventError(null);
          queryClient.setQueryData<DownloadSnapshot>(DOWNLOAD_SNAPSHOT_QUERY_KEY, snapshot);
        }
      },
      (error) => {
        if (mounted) {
          setEventError(error.message);
        }
      },
    ).then((stop) => {
      if (mounted) {
        unsubscribe = stop;
      } else {
        stop();
      }
    }).catch((error: unknown) => {
      if (mounted) {
        setEventError(error instanceof Error ? error.message : "Download updates are unavailable.");
      }
    });

    return () => {
      mounted = false;
      unsubscribe?.();
    };
  }, [queryClient]);

  return { ...query, eventError };
}

function invalidateDownloadSnapshot(queryClient: ReturnType<typeof useQueryClient>) {
  return queryClient.invalidateQueries({ queryKey: DOWNLOAD_SNAPSHOT_QUERY_KEY });
}

export function useCancelDownload() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (taskId: DownloadTaskId) => cancelDownload(taskId),
    onSuccess: () => invalidateDownloadSnapshot(queryClient),
  });
}

export function useRetryDownload() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (taskId: DownloadTaskId) => retryDownload(taskId),
    onSuccess: () => invalidateDownloadSnapshot(queryClient),
  });
}

export function useSetDownloadConcurrency() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (maxConcurrent: number) => setDownloadConcurrency(maxConcurrent),
    onSuccess: (snapshot) => {
      queryClient.setQueryData(DOWNLOAD_SNAPSHOT_QUERY_KEY, snapshot);
    },
  });
}
