import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { cancelPreview, getPreviewState, startPreview } from "../services/ipc";
import type { TrackId } from "../types/domain";

export const PREVIEW_QUERY_KEY = ["preview-state"] as const;

export function usePreview() {
  const queryClient = useQueryClient();
  const state = useQuery({
    queryKey: PREVIEW_QUERY_KEY,
    queryFn: getPreviewState,
    retry: 1,
    refetchInterval: (query) => query.state.data?.phase === "playing" ? 1_000 : false,
  });
  const start = useMutation({
    mutationFn: (trackId: TrackId) => startPreview(trackId),
    onSuccess: (next) => queryClient.setQueryData(PREVIEW_QUERY_KEY, next),
  });
  const cancel = useMutation({
    mutationFn: cancelPreview,
    onSuccess: (next) => queryClient.setQueryData(PREVIEW_QUERY_KEY, next),
  });
  return { state, start, cancel };
}
