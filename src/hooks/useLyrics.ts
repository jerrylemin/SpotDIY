import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  clearCachedLrclib,
  applyAbLoopPreset,
  createBookmark,
  deleteBookmark,
  deleteManualLyrics,
  deleteAbLoopPreset,
  findLrclibBest,
  getLyrics,
  listAbLoopPresets,
  listBookmarks,
  pickAndImportLyricsFile,
  saveAbLoopPreset,
  saveManualLyrics,
  searchLrclib,
  selectLrclibCandidate,
  updateBookmark,
} from "../services/ipc";
import type {
  AbLoopPreset,
  AbLoopPresetId,
  Bookmark,
  BookmarkId,
  LyricsCandidate,
  LyricsDocument,
  ManualLyricsMode,
  SourceId,
  TrackId,
} from "../types/domain";

export const LYRICS_QUERY_KEY = ["lyrics"] as const;
export const BOOKMARKS_QUERY_KEY = ["bookmarks"] as const;
export const AB_LOOP_PRESETS_QUERY_KEY = ["ab-loop-presets"] as const;

export function lyricsQueryKey(trackId: TrackId | null, sourceId: SourceId | null) {
  return [...LYRICS_QUERY_KEY, trackId, sourceId] as const;
}

export function bookmarksQueryKey(trackId: TrackId | null) {
  return [...BOOKMARKS_QUERY_KEY, trackId] as const;
}

export function abLoopPresetsQueryKey(trackId: TrackId | null) {
  return [...AB_LOOP_PRESETS_QUERY_KEY, trackId] as const;
}

export function activeCueIndex(cues: LyricsDocument["cues"], positionMs: number): number {
  let low = 0;
  let high = cues.length - 1;
  let active = -1;

  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    if (cues[middle].startMs <= positionMs) {
      active = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }

  return active;
}

export function useLyrics(trackId: TrackId | null, sourceId: SourceId | null) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: lyricsQueryKey(trackId, sourceId),
    queryFn: () => trackId === null ? Promise.resolve(null) : getLyrics(trackId, sourceId),
    enabled: trackId !== null,
    retry: 1,
  });

  const invalidate = () => queryClient.invalidateQueries({ queryKey: LYRICS_QUERY_KEY });
  const saveManual = useMutation({
    mutationFn: ({ mode, text }: { mode: ManualLyricsMode; text: string }) => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return saveManualLyrics(trackId, mode, text);
    },
    onSuccess: invalidate,
  });
  const removeManual = useMutation({
    mutationFn: () => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return deleteManualLyrics(trackId);
    },
    onSuccess: invalidate,
  });
  const importFile = useMutation({
    mutationFn: () => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return pickAndImportLyricsFile(trackId);
    },
    onSuccess: invalidate,
  });
  const findBest = useMutation({
    mutationFn: () => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return findLrclibBest(trackId);
    },
    onSuccess: invalidate,
  });
  const searchOnline = useMutation<LyricsCandidate[], Error>({
    mutationFn: () => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return searchLrclib(trackId);
    },
  });
  const selectCandidate = useMutation({
    mutationFn: (providerRecordId: number) => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return selectLrclibCandidate(trackId, providerRecordId);
    },
    onSuccess: invalidate,
  });
  const clearCache = useMutation({
    mutationFn: () => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return clearCachedLrclib(trackId);
    },
    onSuccess: invalidate,
  });

  return {
    ...query,
    saveManual,
    removeManual,
    importFile,
    findBest,
    searchOnline,
    selectCandidate,
    clearCache,
  };
}

export function useBookmarks(trackId: TrackId | null) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: bookmarksQueryKey(trackId),
    queryFn: () => trackId === null ? Promise.resolve<Bookmark[]>([]) : listBookmarks(trackId),
    enabled: trackId !== null,
    retry: 1,
  });
  const invalidate = () => queryClient.invalidateQueries({ queryKey: BOOKMARKS_QUERY_KEY });
  const create = useMutation({
    mutationFn: ({ positionMs, note }: { positionMs: number; note: string }) => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return createBookmark(trackId, positionMs, note);
    },
    onSuccess: invalidate,
  });
  const update = useMutation({
    mutationFn: ({ bookmarkId, positionMs, note }: { bookmarkId: BookmarkId; positionMs: number; note: string }) => updateBookmark(bookmarkId, positionMs, note),
    onSuccess: invalidate,
  });
  const remove = useMutation({
    mutationFn: (bookmarkId: BookmarkId) => deleteBookmark(bookmarkId),
    onSuccess: invalidate,
  });
  return { ...query, create, update, remove };
}

export function useAbLoopPresets(trackId: TrackId | null) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: abLoopPresetsQueryKey(trackId),
    queryFn: () => trackId === null ? Promise.resolve<AbLoopPreset[]>([]) : listAbLoopPresets(trackId),
    enabled: trackId !== null,
    retry: 1,
  });
  const invalidate = () => queryClient.invalidateQueries({ queryKey: AB_LOOP_PRESETS_QUERY_KEY });
  const save = useMutation({
    mutationFn: ({ name }: { name: string }) => {
      if (trackId === null) {
        return Promise.reject(new Error("There is no current track."));
      }
      return saveAbLoopPreset(trackId, name);
    },
    onSuccess: invalidate,
  });
  const apply = useMutation({
    mutationFn: (presetId: AbLoopPresetId) => applyAbLoopPreset(presetId),
  });
  const remove = useMutation({
    mutationFn: (presetId: AbLoopPresetId) => deleteAbLoopPreset(presetId),
    onSuccess: invalidate,
  });
  return { ...query, save, apply, remove };
}
