import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useCallback, useEffect, useRef, useState } from "react";

import { EmptyState } from "../components/common/EmptyState";
import { ContextActionMenu } from "../components/common/ContextActionMenu";
import { SpotIcon } from "../components/icons/SpotIcon";
import { usePlayback } from "../hooks/usePlayback";
import { useUiStore } from "../stores/ui-store";
import {
  createPlaylist,
  deletePlaylist,
  getPlaylist,
  IpcError,
  isTauriRuntime,
  listPlaylists,
  playPlaylist,
  queuePlaylist,
  removePlaylistItem,
  renamePlaylist,
  reorderPlaylistItem,
} from "../services/ipc";
import type { Playlist, PlaylistItem, PlaylistId } from "../types/domain";

const PINNED_PLAYLISTS_KEY = "spotdiy.playlists.pinned";

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

function readPinnedPlaylistIds(): Set<string> {
  try {
    const raw = window.localStorage.getItem(PINNED_PLAYLISTS_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string") : []);
  } catch {
    return new Set();
  }
}

function writePinnedPlaylistIds(ids: Set<string>): void {
  try {
    window.localStorage.setItem(PINNED_PLAYLISTS_KEY, JSON.stringify([...ids]));
  } catch {
    // Pinning is a convenience preference; storage failure must not block playlists.
  }
}

interface SortablePlaylistItemProps {
  item: PlaylistItem;
  selected: boolean;
  editable: boolean;
  onSelect: (item: PlaylistItem) => void;
  onRemove: (item: PlaylistItem) => void;
  onPlayNow: (item: PlaylistItem) => void;
  onPlayNext: (item: PlaylistItem) => void;
  onQueue: (item: PlaylistItem) => void;
  onInspect: (item: PlaylistItem) => void;
  playbackPending: boolean;
}

function SortablePlaylistItem({ item, selected, editable, onSelect, onRemove, onPlayNow, onPlayNext, onQueue, onInspect, playbackPending }: SortablePlaylistItemProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
  } = useSortable({ id: item.id, data: { index: item.position } });

  return (
    <ContextActionMenu
      actions={[
        { id: "play", label: "Play now", onSelect: () => onPlayNow(item), disabled: playbackPending, disabledReason: "Playback is busy" },
        { id: "play-next", label: "Play next", onSelect: () => onPlayNext(item), disabled: playbackPending, disabledReason: "Playback is busy" },
        { id: "queue", label: "Add to queue", onSelect: () => onQueue(item), disabled: playbackPending, disabledReason: "Playback is busy" },
        { id: "inspect", label: "Inspect", onSelect: () => onInspect(item) },
      ]}
      className="playlist-item-context-menu"
      label={`Actions for track ${item.trackId}`}
    >
      <div className={`playlist-item-row${selected ? " playlist-item-row-selected" : ""}`} ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition }}>
        <input aria-label={`Select track ${item.trackId}`} checked={selected} disabled={!editable} onChange={() => onSelect(item)} type="checkbox" />
        <button aria-label={`Drag track ${item.trackId}`} className="playlist-drag-handle" disabled={!editable} ref={setActivatorNodeRef} type="button" {...attributes} {...listeners}>⋮⋮</button>
        <span className="playlist-item-position">{item.position + 1}</span>
        <div className="playlist-item-copy">
          <strong>{item.trackId}</strong>
          <span>{item.requestedSourceId ? `Requested source ${item.requestedSourceId}` : "Source resolved at playback"}</span>
        </div>
        <button aria-label={`Delete track ${item.trackId}`} className="queue-entry-action queue-entry-remove" disabled={!editable} onClick={() => onRemove(item)} type="button"><SpotIcon name="trash" size={13} /> Delete</button>
      </div>
    </ContextActionMenu>
  );
}

export function PlaylistsPage() {
  const nativeRuntime = isTauriRuntime();
  const playback = usePlayback();
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [pinnedPlaylistIds, setPinnedPlaylistIds] = useState<Set<string>>(() => readPinnedPlaylistIds());
  const [selectedPlaylistId, setSelectedPlaylistId] = useState<PlaylistId | null>(null);
  const selectedPlaylistRef = useRef<PlaylistId | null>(null);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [selectedItemIds, setSelectedItemIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [actionPending, setActionPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const orderedPlaylists = [...playlists].sort((left, right) => {
    const leftPinned = pinnedPlaylistIds.has(left.id) ? 0 : 1;
    const rightPinned = pinnedPlaylistIds.has(right.id) ? 0 : 1;
    return leftPinned - rightPinned || left.name.localeCompare(right.name);
  });

  const refresh = useCallback(async (preferredId?: PlaylistId | null) => {
    if (!nativeRuntime) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const next = (await listPlaylists()).filter((playlist) => playlist.kind === "normal");
      const validIds = new Set<string>(next.map((playlist) => playlist.id));
      setPinnedPlaylistIds((current) => {
        const filtered = new Set([...current].filter((id) => validIds.has(id)));
        writePinnedPlaylistIds(filtered);
        return filtered;
      });
      setPlaylists(next);
      const nextId = preferredId && next.some((playlist) => playlist.id === preferredId)
        ? preferredId
        : selectedPlaylistRef.current && next.some((playlist) => playlist.id === selectedPlaylistRef.current)
          ? selectedPlaylistRef.current
          : next[0]?.id ?? null;
      selectedPlaylistRef.current = nextId;
      setSelectedPlaylistId(nextId);
      if (nextId) {
        setSelectedPlaylist(await getPlaylist(nextId));
      } else {
        setSelectedPlaylist(null);
      }
      setError(null);
    } catch (refreshError) {
      setError(errorMessage(refreshError, "SpotDIY could not read playlists."));
    } finally {
      setLoading(false);
    }
  }, [nativeRuntime]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    setSelectedItemIds(new Set());
  }, [selectedPlaylist]);

  const runAction = async (action: () => Promise<unknown>, fallback: string, preferredId = selectedPlaylistId) => {
    setActionPending(true);
    setError(null);
    try {
      await action();
      await refresh(preferredId);
    } catch (actionError) {
      setError(errorMessage(actionError, fallback));
    } finally {
      setActionPending(false);
    }
  };

  const create = () => {
    const name = window.prompt("Name the new playlist:", "New playlist");
    if (name) {
      void runAction(() => createPlaylist(name), "SpotDIY could not create that playlist.", null);
    }
  };

  const togglePinned = (playlistId: PlaylistId) => {
    setPinnedPlaylistIds((current) => {
      const next = new Set(current);
      if (next.has(playlistId)) {
        next.delete(playlistId);
      } else {
        next.add(playlistId);
      }
      writePinnedPlaylistIds(next);
      return next;
    });
  };

  const selectedItems = selectedPlaylist?.items.filter((item) => selectedItemIds.has(item.id)) ?? [];
  const selectedItemIdList = selectedItems.length > 0 ? selectedItems.map((item) => item.id) : selectedPlaylist?.items.map((item) => item.id) ?? [];
  const editablePlaylist = Boolean(nativeRuntime && selectedPlaylist);

  const handleItemDragEnd = (event: DragEndEvent) => {
    if (!selectedPlaylist || !editablePlaylist || !event.over) {
      return;
    }
    const item = selectedPlaylist.items.find((candidate) => candidate.id === event.active.id);
    const over = selectedPlaylist.items.find((candidate) => candidate.id === event.over?.id);
    if (!item || !over || item.position === over.position) {
      return;
    }
    void runAction(
      () => reorderPlaylistItem(selectedPlaylist.id, item.id, over.position),
      "SpotDIY could not reorder that playlist.",
    );
  };

  if (!nativeRuntime) {
    return (
      <div className="page-stack">
        <section className="page-intro"><div><span className="eyebrow">PLAYLISTS</span><h1>Shape the <em>moment.</em></h1><p>Keep your playlists focused: add tracks, reorder them, and play them anywhere.</p></div><button className="button button-primary" disabled type="button"><SpotIcon name="playlist" size={16} /> New playlist</button></section>
        <EmptyState icon="playlist" eyebrow="NATIVE WORKSPACE" title="Playlists live with your library" description="Open the native SpotDIY app to create, edit, pin, and play playlists." />
      </div>
    );
  }

  return (
    <div className="page-stack playlists-page">
      <section className="page-intro">
        <div><span className="eyebrow">PLAYLISTS</span><h1>Shape the <em>moment.</em></h1><p>Create simple playlists for the tracks you want to keep together.</p></div>
        <button className="button button-primary" disabled={actionPending} onClick={create} type="button"><SpotIcon name="playlist" size={16} /> New playlist</button>
      </section>

      {error ? <div className="library-alert library-alert-error" role="alert"><SpotIcon name="alert" size={16} /><span>{error}</span></div> : null}
      {loading ? <div className="library-pending-state" role="status"><SpotIcon name="spark" size={18} /> Loading playlists…</div> : orderedPlaylists.length === 0 ? <EmptyState icon="playlist" eyebrow="NO PLAYLISTS YET" title="Start with one playlist" description="Create a playlist, then use Add playlist on any library track to fill it." action={<button className="button button-primary" onClick={create} type="button">Create playlist</button>} /> : (
        <section className="playlist-workspace">
          <aside aria-label="Playlists" className="playlist-sidebar">
            <div className="section-heading"><div><span className="eyebrow">PLAYLISTS</span><h2>Your spaces</h2></div><span className="section-note">{orderedPlaylists.length}</span></div>
            <div className="playlist-nav-list">
              {orderedPlaylists.map((playlist) => (
                <button className={`playlist-nav-item${playlist.id === selectedPlaylistId ? " playlist-nav-item-active" : ""}`} key={playlist.id} onClick={() => { selectedPlaylistRef.current = playlist.id; setSelectedPlaylistId(playlist.id); void getPlaylist(playlist.id).then(setSelectedPlaylist).catch((loadError) => setError(errorMessage(loadError, "SpotDIY could not read that playlist."))); }} type="button">
                  <SpotIcon name={pinnedPlaylistIds.has(playlist.id) ? "pin" : "playlist"} size={16} />
                  <span>{playlist.name}</span>
                  <small>{playlist.items.length}</small>
                </button>
              ))}
            </div>
          </aside>

          <div className="playlist-detail">
            {selectedPlaylist ? (
              <>
                <div className="playlist-detail-header">
                  <div><span className="eyebrow">PLAYLIST</span><h2>{selectedPlaylist.name}</h2><p>{selectedPlaylist.items.length} track{selectedPlaylist.items.length === 1 ? "" : "s"} · revision {selectedPlaylist.revision}</p></div>
                  <div className="playlist-detail-actions">
                    <button aria-pressed={pinnedPlaylistIds.has(selectedPlaylist.id)} className="button button-quiet button-small" onClick={() => togglePinned(selectedPlaylist.id)} type="button"><SpotIcon name="pin" size={13} /> {pinnedPlaylistIds.has(selectedPlaylist.id) ? "Unpin" : "Pin"}</button>
                    <button className="button button-quiet button-small" disabled={!editablePlaylist || actionPending} onClick={() => { const name = window.prompt("Rename playlist:", selectedPlaylist.name); if (name) void runAction(() => renamePlaylist(selectedPlaylist.id, name), "SpotDIY could not rename that playlist."); }} type="button">Rename</button>
                    <button className="button button-quiet button-small playlist-danger" disabled={!editablePlaylist || actionPending} onClick={() => { if (window.confirm(`Delete “${selectedPlaylist.name}”?`)) void runAction(() => deletePlaylist(selectedPlaylist.id), "SpotDIY could not delete that playlist.", null); }} type="button"><SpotIcon name="trash" size={13} /> Delete</button>
                  </div>
                </div>

                <div className="playlist-toolbar">
                  <button className="button button-primary button-small" disabled={selectedItemIdList.length === 0 || actionPending} onClick={() => void runAction(() => playPlaylist(selectedPlaylist.id, selectedItemIdList), "SpotDIY could not start that playlist.")} type="button"><SpotIcon name="play" size={13} /> Play {selectedItems.length > 0 ? "selected" : "all"}</button>
                  <button className="button button-quiet button-small" disabled={selectedItemIdList.length === 0 || actionPending} onClick={() => void runAction(() => queuePlaylist(selectedPlaylist.id, selectedItemIdList), "SpotDIY could not add that playlist to the queue.")} type="button"><SpotIcon name="queue" size={13} /> Add to queue</button>
                  <span className="playlist-toolbar-note">Select tracks to target playback; no selection uses the full playlist.</span>
                </div>

                <DndContext onDragEnd={handleItemDragEnd} sensors={sensors}>
                  <SortableContext items={selectedPlaylist.items.map((item) => item.id)} strategy={verticalListSortingStrategy}>
                    <div className="playlist-item-list">
                      {selectedPlaylist.items.length === 0 ? <div className="queue-section-empty">No tracks yet. Add one from Your library.</div> : selectedPlaylist.items.map((item) => <SortablePlaylistItem editable={editablePlaylist} item={item} key={item.id} onInspect={(row) => openTrackInspector(row.trackId)} onPlayNext={(row) => { void playback.playNext(row.trackId, row.requestedSourceId); }} onPlayNow={(row) => { void playback.playNow(row.trackId, row.requestedSourceId); }} onQueue={(row) => { void playback.addToQueue(row.trackId, row.requestedSourceId); }} onRemove={(row) => void runAction(() => removePlaylistItem(selectedPlaylist.id, row.id), "SpotDIY could not remove that playlist item.")} onSelect={(row) => setSelectedItemIds((current) => { const next = new Set(current); if (next.has(row.id)) next.delete(row.id); else next.add(row.id); return next; })} playbackPending={playback.pending} selected={selectedItemIds.has(item.id)} />)}
                    </div>
                  </SortableContext>
                </DndContext>
              </>
            ) : null}
          </div>
        </section>
      )}
    </div>
  );
}
