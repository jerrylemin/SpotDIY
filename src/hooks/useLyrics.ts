import { useCallback, useEffect, useState } from "react";
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
  LyricsWord,
  ManualLyricsMode,
  SourceId,
  TrackId,
} from "../types/domain";

export const LYRICS_QUERY_KEY = ["lyrics"] as const;
export const BOOKMARKS_QUERY_KEY = ["bookmarks"] as const;
export const AB_LOOP_PRESETS_QUERY_KEY = ["ab-loop-presets"] as const;
export const LYRICS_OFFSET_STEP_MS = 250;
export const MAX_LYRICS_OFFSET_MS = 5_000;

const LYRICS_OFFSET_GRANULARITY_MS = 50;
const LYRICS_OFFSET_STORAGE_PREFIX = "spotdiy:lyrics-offset:";
const LYRICS_OFFSET_EVENT = "spotdiy:lyrics-offset-changed";
const automaticLookupKeys = new Set<string>();

function lyricsOffsetStorageKey(trackId: TrackId): string {
  return `${LYRICS_OFFSET_STORAGE_PREFIX}${trackId}`;
}

export function clampLyricsOffset(offsetMs: number): number {
  if (!Number.isFinite(offsetMs)) {
    return 0;
  }
  const rounded = Math.round(offsetMs / LYRICS_OFFSET_GRANULARITY_MS) * LYRICS_OFFSET_GRANULARITY_MS;
  return Math.min(MAX_LYRICS_OFFSET_MS, Math.max(-MAX_LYRICS_OFFSET_MS, rounded));
}

export function formatLyricsOffset(offsetMs: number): string {
  const normalized = clampLyricsOffset(offsetMs);
  return normalized === 0 ? "0 ms" : `${normalized > 0 ? "+" : ""}${normalized} ms`;
}

export function readLyricsOffset(trackId: TrackId | null): number {
  if (trackId === null || typeof window === "undefined") {
    return 0;
  }
  try {
    return clampLyricsOffset(Number(window.localStorage.getItem(lyricsOffsetStorageKey(trackId)) ?? 0));
  } catch {
    return 0;
  }
}

export function useLyricsOffset(trackId: TrackId | null) {
  const [offsetMs, setOffsetMs] = useState(() => readLyricsOffset(trackId));

  useEffect(() => {
    setOffsetMs(readLyricsOffset(trackId));
  }, [trackId]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    const synchronize = () => setOffsetMs(readLyricsOffset(trackId));
    window.addEventListener(LYRICS_OFFSET_EVENT, synchronize);
    return () => window.removeEventListener(LYRICS_OFFSET_EVENT, synchronize);
  }, [trackId]);

  const setOffset = useCallback((nextOffsetMs: number) => {
    if (trackId === null || typeof window === "undefined") {
      return;
    }
    const next = clampLyricsOffset(nextOffsetMs);
    try {
      window.localStorage.setItem(lyricsOffsetStorageKey(trackId), String(next));
    } catch {
      // The sync control remains useful when storage is unavailable.
    }
    setOffsetMs(next);
    if (typeof window !== "undefined") {
      window.dispatchEvent(new Event(LYRICS_OFFSET_EVENT));
    }
  }, [trackId]);

  const nudge = useCallback((deltaMs: number) => {
    setOffset(offsetMs + deltaMs);
  }, [offsetMs, setOffset]);
  const reset = useCallback(() => setOffset(0), [setOffset]);

  return {
    offsetMs,
    setOffset,
    nudge,
    reset,
  };
}

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

export function activeWordIndex(words: LyricsWord[], positionMs: number): number {
  let low = 0;
  let high = words.length - 1;
  let active = -1;

  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    if (words[middle].startMs <= positionMs) {
      active = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }

  return active;
}

export const DEFAULT_CUE_DURATION_MS = 4_000;

export function cueEndMs(
  cues: LyricsDocument["cues"],
  index: number,
  durationMs: number | null = null,
): number | null {
  const cue = cues[index];
  if (!cue) {
    return null;
  }
  const nextStartMs = cues[index + 1]?.startMs;
  const fallbackEndMs = durationMs ?? cue.startMs + DEFAULT_CUE_DURATION_MS;
  return Math.max(cue.startMs + 1, nextStartMs ?? fallbackEndMs);
}

export function cueProgress(
  cues: LyricsDocument["cues"],
  index: number,
  positionMs: number,
  durationMs: number | null = null,
): number {
  const cue = cues[index];
  const endMs = cueEndMs(cues, index, durationMs);
  if (!cue || endMs === null || endMs <= cue.startMs) {
    return 0;
  }
  return Math.min(1, Math.max(0, (positionMs - cue.startMs) / (endMs - cue.startMs)));
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

  const autoLookupKey = trackId === null ? null : `${trackId}:${sourceId ?? ""}`;
  useEffect(() => {
    if (
      autoLookupKey === null
      || !query.isSuccess
      || query.data !== null
      || automaticLookupKeys.has(autoLookupKey)
    ) {
      return;
    }
    automaticLookupKeys.add(autoLookupKey);
    void findBest.mutateAsync().catch(() => {
      // Automatic lookup is best effort; the page still exposes an explicit retry.
    });
  }, [autoLookupKey, findBest, query.data, query.isSuccess]);

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
