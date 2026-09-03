import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  getAnalyticsOverview,
  getListeningHeatmap,
  getListeningSessionHistory,
  getTasteTimeline,
  getTimeMachineDay,
  getTopArtists,
  getTopTracks,
  listListeningSessions,
  reopenListeningSessionAsQueue,
  reopenTimeMachineDayAsQueue,
  setListeningSessionLabel,
} from "../services/ipc";
import type { ListeningSessionId } from "../types/domain";

export const ANALYTICS_OVERVIEW_QUERY_KEY = ["analytics-overview"] as const;
export const ANALYTICS_HEATMAP_QUERY_KEY = ["analytics-heatmap"] as const;
export const ANALYTICS_TOP_TRACKS_QUERY_KEY = ["analytics-top-tracks"] as const;
export const ANALYTICS_TOP_ARTISTS_QUERY_KEY = ["analytics-top-artists"] as const;
export const ANALYTICS_TIMELINE_QUERY_KEY = ["analytics-timeline"] as const;
export const ANALYTICS_SESSIONS_QUERY_KEY = ["analytics-sessions"] as const;

export function analyticsSessionHistoryQueryKey(sessionId: ListeningSessionId) {
  return ["analytics-session-history", sessionId] as const;
}

export function analyticsDayQueryKey(localDate: string) {
  return ["analytics-day", localDate] as const;
}

export function useAnalyticsOverview() {
  return useQuery({
    queryKey: ANALYTICS_OVERVIEW_QUERY_KEY,
    queryFn: getAnalyticsOverview,
    retry: 1,
  });
}

export function useListeningHeatmap() {
  return useQuery({
    queryKey: ANALYTICS_HEATMAP_QUERY_KEY,
    queryFn: getListeningHeatmap,
    retry: 1,
  });
}

export function useTopTracks(limit = 10) {
  return useQuery({
    queryKey: [...ANALYTICS_TOP_TRACKS_QUERY_KEY, limit],
    queryFn: () => getTopTracks(limit),
    retry: 1,
  });
}

export function useTopArtists(limit = 10) {
  return useQuery({
    queryKey: [...ANALYTICS_TOP_ARTISTS_QUERY_KEY, limit],
    queryFn: () => getTopArtists(limit),
    retry: 1,
  });
}

export function useTasteTimeline() {
  return useQuery({
    queryKey: ANALYTICS_TIMELINE_QUERY_KEY,
    queryFn: getTasteTimeline,
    retry: 1,
  });
}

export function useListeningSessions(page = 0, pageSize = 20) {
  return useQuery({
    queryKey: [...ANALYTICS_SESSIONS_QUERY_KEY, page, pageSize],
    queryFn: () => listListeningSessions(page, pageSize),
    placeholderData: keepPreviousData,
    retry: 1,
  });
}

export function useListeningSessionHistory(sessionId: ListeningSessionId | null) {
  return useQuery({
    queryKey: sessionId ? analyticsSessionHistoryQueryKey(sessionId) : ["analytics-session-history", "none"],
    queryFn: () => getListeningSessionHistory(sessionId as ListeningSessionId),
    enabled: sessionId !== null,
    retry: 1,
  });
}

export function useTimeMachineDay(localDate: string) {
  return useQuery({
    queryKey: analyticsDayQueryKey(localDate),
    queryFn: () => getTimeMachineDay(localDate),
    enabled: /^\d{4}-\d{2}-\d{2}$/.test(localDate),
    retry: 1,
  });
}

export function useAnalyticsActions() {
  const queryClient = useQueryClient();
  const invalidate = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ANALYTICS_OVERVIEW_QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ANALYTICS_HEATMAP_QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ANALYTICS_TOP_TRACKS_QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ANALYTICS_TOP_ARTISTS_QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ANALYTICS_TIMELINE_QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ANALYTICS_SESSIONS_QUERY_KEY }),
    ]);
  };

  const labelSession = useMutation({
    mutationFn: ({ sessionId, label }: { sessionId: ListeningSessionId; label: string | null }) => setListeningSessionLabel(sessionId, label),
    onSuccess: invalidate,
  });
  const reopenSession = useMutation({ mutationFn: reopenListeningSessionAsQueue, onSuccess: invalidate });
  const reopenDay = useMutation({ mutationFn: reopenTimeMachineDayAsQueue, onSuccess: invalidate });

  return { labelSession, reopenSession, reopenDay };
}
