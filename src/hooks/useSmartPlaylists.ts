import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  createSmartPlaylist,
  deleteSmartPlaylist,
  getSmartPlaylist,
  listSmartPlaylists,
  previewSmartPlaylist,
  updateSmartPlaylist,
} from "../services/ipc";
import type { SmartPlaylistId, SmartPlaylistInput } from "../types/domain";

export const SMART_PLAYLISTS_QUERY_KEY = ["smart-playlists"] as const;

export function smartPlaylistQueryKey(playlistId: SmartPlaylistId) {
  return ["smart-playlist", playlistId] as const;
}

export function smartPlaylistPreviewQueryKey(playlistId: SmartPlaylistId, page = 0, pageSize = 20) {
  return ["smart-playlist-preview", playlistId, page, pageSize] as const;
}

export function useSmartPlaylists() {
  return useQuery({
    queryKey: SMART_PLAYLISTS_QUERY_KEY,
    queryFn: listSmartPlaylists,
    retry: 1,
  });
}

export function useSmartPlaylist(playlistId: SmartPlaylistId | null) {
  return useQuery({
    queryKey: playlistId ? smartPlaylistQueryKey(playlistId) : ["smart-playlist", "none"],
    queryFn: () => getSmartPlaylist(playlistId as SmartPlaylistId),
    enabled: playlistId !== null,
    retry: 1,
  });
}

export function useSmartPlaylistPreview(playlistId: SmartPlaylistId | null, page = 0, pageSize = 20) {
  return useQuery({
    queryKey: playlistId ? smartPlaylistPreviewQueryKey(playlistId, page, pageSize) : ["smart-playlist-preview", "none"],
    queryFn: () => previewSmartPlaylist(playlistId as SmartPlaylistId, page, pageSize),
    enabled: playlistId !== null,
    retry: 1,
  });
}

export function useSmartPlaylistActions() {
  const queryClient = useQueryClient();
  const invalidate = async () => {
    await queryClient.invalidateQueries({ queryKey: SMART_PLAYLISTS_QUERY_KEY });
  };
  const create = useMutation({ mutationFn: createSmartPlaylist, onSuccess: invalidate });
  const update = useMutation({
    mutationFn: ({ playlistId, input }: { playlistId: SmartPlaylistId; input: SmartPlaylistInput }) => updateSmartPlaylist(playlistId, input),
    onSuccess: invalidate,
  });
  const remove = useMutation({ mutationFn: deleteSmartPlaylist, onSuccess: invalidate });
  return { create, update, remove };
}
