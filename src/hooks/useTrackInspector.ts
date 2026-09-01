import { useQuery } from "@tanstack/react-query";

import { getTrackInspector } from "../services/ipc";
import type { TrackId } from "../types/domain";

export const TRACK_INSPECTOR_QUERY_KEY = ["track-inspector"] as const;

export function trackInspectorQueryKey(trackId: TrackId | null) {
  return [...TRACK_INSPECTOR_QUERY_KEY, trackId] as const;
}

export function useTrackInspector(trackId: TrackId | null, enabled = true) {
  return useQuery({
    queryKey: trackInspectorQueryKey(trackId),
    queryFn: () => trackId === null ? Promise.resolve(null) : getTrackInspector(trackId),
    enabled: enabled && trackId !== null,
    staleTime: 15_000,
    retry: 1,
  });
}
