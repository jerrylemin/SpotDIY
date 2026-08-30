import { listen } from "@tauri-apps/api/event";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import {
  LIBRARY_PROGRESS_EVENT,
  addLibraryFolders,
  getLibraryPage,
  getLibraryStatus,
  isTauriRuntime,
  parseScanProgress,
  removeLibraryFolder,
  rescanAllLibraryFolders,
  rescanLibraryFolder,
  revealLocalFile,
} from "../services/ipc";
import type {
  LibraryPageRequest,
  ScanProgress,
  SourceId,
} from "../types/domain";

export const LIBRARY_STATUS_QUERY_KEY = ["library-status"] as const;
export const LIBRARY_PAGE_QUERY_KEY = ["library-page"] as const;

export function libraryPageQueryKey(request: LibraryPageRequest) {
  return [...LIBRARY_PAGE_QUERY_KEY, request] as const;
}

function invalidateLibraryQueries(queryClient: ReturnType<typeof useQueryClient>) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: LIBRARY_STATUS_QUERY_KEY }),
    queryClient.invalidateQueries({ queryKey: LIBRARY_PAGE_QUERY_KEY }),
  ]);
}

export function useLibraryStatus() {
  return useQuery({
    queryKey: LIBRARY_STATUS_QUERY_KEY,
    queryFn: getLibraryStatus,
    retry: 1,
  });
}

export function useLibraryPage(request: LibraryPageRequest) {
  return useQuery({
    queryKey: libraryPageQueryKey(request),
    queryFn: () => getLibraryPage(request),
    placeholderData: keepPreviousData,
    retry: 1,
  });
}

export function useAddLibraryFolders() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: addLibraryFolders,
    onSuccess: () => invalidateLibraryQueries(queryClient),
  });
}

export function useRemoveLibraryFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: removeLibraryFolder,
    onSuccess: () => invalidateLibraryQueries(queryClient),
  });
}

export function useRescanLibraryFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: rescanLibraryFolder,
    onSuccess: () => invalidateLibraryQueries(queryClient),
  });
}

export function useRescanAllLibraryFolders() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: rescanAllLibraryFolders,
    onSuccess: () => invalidateLibraryQueries(queryClient),
  });
}

export function useRevealLocalFile() {
  return useMutation({ mutationFn: (sourceId: SourceId) => revealLocalFile(sourceId) });
}

export function useLibraryProgress(): ScanProgress | null {
  const queryClient = useQueryClient();
  const [progress, setProgress] = useState<ScanProgress | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | undefined;
    const handleProgress = (event: { payload: unknown }) => {
      if (!active) {
        return;
      }
      try {
        const next = parseScanProgress(event.payload);
        setProgress(next);
        const refetchType = next.status === "scanning" ? "none" : "active";
        void Promise.all([
          queryClient.invalidateQueries({ queryKey: LIBRARY_STATUS_QUERY_KEY, refetchType }),
          queryClient.invalidateQueries({ queryKey: LIBRARY_PAGE_QUERY_KEY, refetchType }),
        ]);
      } catch {
        // Native events are untrusted input; an invalid payload must not break the page.
      }
    };

    void listen<unknown>(LIBRARY_PROGRESS_EVENT, handleProgress)
      .then((stopListening) => {
        if (active) {
          unlisten = stopListening;
        } else {
          stopListening();
        }
      })
      .catch(() => {
        // The native event bridge may be unavailable while the web preview is mounting.
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [queryClient]);

  return progress;
}
