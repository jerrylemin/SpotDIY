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
import { SmartPlaylistPanel } from "../components/smart/SmartPlaylistPanel";
import { usePlayback } from "../hooks/usePlayback";
import { useUiStore } from "../stores/ui-store";
import {
  createPlaylist,
  createPlaylistBranch,
  deletePlaylist,
  discardPlaylistBranch,
  duplicatePlaylist,
  getBranchChanges,
  getPlaylist,
  IpcError,
  isTauriRuntime,
  listPlaylists,
  mergeBranchChanges,
  playPlaylist,
  queuePlaylist,
  removePlaylistItem,
  renamePlaylist,
  reorderPlaylistItem,
} from "../services/ipc";
import type { BranchChange, Playlist, PlaylistItem, PlaylistId } from "../types/domain";

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

function playlistKindLabel(playlist: Playlist): string {
  if (playlist.kind === "inbox") {
    return "Inbox";
  }
  if (playlist.kind === "branch") {
    return playlist.branchStatus === "merged" ? "Merged branch" : "Branch";
  }
  return "Playlist";
}

function changeKey(change: BranchChange): string {
  switch (change.type) {
    case "add":
      return `add:${change.branchItemId}`;
    case "remove":
      return `remove:${change.baseItemId}`;
    case "move":
      return `move:${change.baseItemId}:${change.targetPosition}`;
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
      <button aria-label={`Remove track ${item.trackId}`} className="queue-entry-action queue-entry-remove" disabled={!editable} onClick={() => onRemove(item)} type="button">Remove</button>
    </div>
    </ContextActionMenu>
  );
}

export function PlaylistsPage() {
  const nativeRuntime = isTauriRuntime();
  const playback = usePlayback();
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [selectedPlaylistId, setSelectedPlaylistId] = useState<PlaylistId | null>(null);
  const selectedPlaylistRef = useRef<PlaylistId | null>(null);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [selectedItemIds, setSelectedItemIds] = useState<Set<string>>(new Set());
  const [branchChanges, setBranchChanges] = useState<BranchChange[]>([]);
  const [selectedChanges, setSelectedChanges] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [actionPending, setActionPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const refresh = useCallback(async (preferredId?: PlaylistId | null) => {
    if (!nativeRuntime) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const next = await listPlaylists();
      setPlaylists(next);
      const nextId = preferredId && next.some((playlist) => playlist.id === preferredId)
        ? preferredId
        : selectedPlaylistRef.current && next.some((playlist) => playlist.id === selectedPlaylistRef.current)
          ? selectedPlaylistRef.current
          : next.find((playlist) => playlist.kind === "normal")?.id ?? next[0]?.id ?? null;
      selectedPlaylistRef.current = nextId;
      setSelectedPlaylistId(nextId);
      if (nextId) {
        const detail = await getPlaylist(nextId);
        setSelectedPlaylist(detail);
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
    if (!selectedPlaylist || selectedPlaylist.kind !== "branch" || selectedPlaylist.branchStatus !== "open") {
      setBranchChanges([]);
      setSelectedChanges(new Set());
      return;
    }
    let active = true;
    void getBranchChanges(selectedPlaylist.id)
      .then((changes) => {
        if (active) {
          setBranchChanges(changes);
          setSelectedChanges(new Set(changes.map(changeKey)));
        }
      })
      .catch((branchError) => {
        if (active) {
          setError(errorMessage(branchError, "SpotDIY could not read branch changes."));
        }
      });
    return () => {
      active = false;
    };
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

  const selectedItems = selectedPlaylist?.items.filter((item) => selectedItemIds.has(item.id)) ?? [];
  const selectedItemIdList = selectedItems.length > 0 ? selectedItems.map((item) => item.id) : selectedPlaylist?.items.map((item) => item.id) ?? [];
  const editablePlaylist = Boolean(nativeRuntime && selectedPlaylist && selectedPlaylist.kind !== "inbox" && selectedPlaylist.branchStatus !== "merged");
  const selectedChangeList = branchChanges.filter((change) => selectedChanges.has(changeKey(change)));

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
        <section className="page-intro"><div><span className="eyebrow">PLAYLISTS</span><h1>Shape the <em>moment.</em></h1><p>Keep playlists simple, branchable, and close to how you actually listen.</p></div><button className="button button-primary" disabled type="button"><SpotIcon name="playlist" size={16} /> New playlist</button></section>
        <SmartPlaylistPanel />
        <EmptyState icon="playlist" eyebrow="NATIVE WORKSPACE" title="Playlists live with your library" description="Open the native SpotDIY app to create durable playlists, manage the Inbox, and make one-shot branches." />
      </div>
    );
  }

  return (
    <div className="page-stack playlists-page">
      <section className="page-intro">
        <div><span className="eyebrow">PLAYLISTS</span><h1>Shape the <em>moment.</em></h1><p>Durable collections with a lightweight Inbox and one-shot branches for decisions you can review.</p></div>
        <button className="button button-primary" disabled={actionPending} onClick={create} type="button"><SpotIcon name="playlist" size={16} /> New playlist</button>
      </section>

      <SmartPlaylistPanel />

      {error ? <div className="library-alert library-alert-error" role="alert"><SpotIcon name="alert" size={16} /><span>{error}</span></div> : null}
      {loading ? <div className="library-pending-state" role="status"><SpotIcon name="spark" size={18} /> Loading playlists…</div> : playlists.length === 0 ? <EmptyState icon="playlist" eyebrow="NO PLAYLISTS YET" title="Your next context belongs here" description="Create a playlist from the native app, then add local tracks from the library." action={<button className="button button-primary" onClick={create} type="button">Create playlist</button>} /> : (
        <section className="playlist-workspace">
          <aside aria-label="Playlist collections" className="playlist-sidebar">
            <div className="section-heading"><div><span className="eyebrow">COLLECTIONS</span><h2>Listening spaces</h2></div><span className="section-note">{playlists.length}</span></div>
            <div className="playlist-nav-list">
              {playlists.map((playlist) => (
                <button className={`playlist-nav-item${playlist.id === selectedPlaylistId ? " playlist-nav-item-active" : ""}`} key={playlist.id} onClick={() => { selectedPlaylistRef.current = playlist.id; setSelectedPlaylistId(playlist.id); void getPlaylist(playlist.id).then(setSelectedPlaylist).catch((loadError) => setError(errorMessage(loadError, "SpotDIY could not read that playlist."))); }} type="button">
                  <SpotIcon name={playlist.kind === "inbox" ? "library" : playlist.kind === "branch" ? "spark" : "playlist"} size={16} />
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
                  <div><span className="eyebrow">{playlistKindLabel(selectedPlaylist)}</span><h2>{selectedPlaylist.name}</h2><p>{selectedPlaylist.items.length} track{selectedPlaylist.items.length === 1 ? "" : "s"} · revision {selectedPlaylist.revision}</p></div>
                  <div className="playlist-detail-actions">
                    <button className="button button-quiet button-small" disabled={!editablePlaylist || actionPending} onClick={() => { const name = window.prompt("Rename playlist:", selectedPlaylist.name); if (name) void runAction(() => renamePlaylist(selectedPlaylist.id, name), "SpotDIY could not rename that playlist."); }} type="button">Rename</button>
                    <button className="button button-quiet button-small" disabled={selectedPlaylist.kind === "inbox" || actionPending} onClick={() => { if (window.confirm(`Duplicate “${selectedPlaylist.name}”?`)) void runAction(() => duplicatePlaylist(selectedPlaylist.id), "SpotDIY could not duplicate that playlist.", null); }} type="button">Duplicate</button>
                    <button className="button button-quiet button-small playlist-danger" disabled={!editablePlaylist || actionPending} onClick={() => { if (window.confirm(`Delete “${selectedPlaylist.name}”?`)) void runAction(() => deletePlaylist(selectedPlaylist.id), "SpotDIY could not delete that playlist.", null); }} type="button"><SpotIcon name="trash" size={13} /> Delete</button>
                  </div>
                </div>

                <div className="playlist-toolbar">
                  <button className="button button-primary button-small" disabled={selectedItemIdList.length === 0 || actionPending} onClick={() => void runAction(() => playPlaylist(selectedPlaylist.id, selectedItemIdList), "SpotDIY could not start that playlist.")} type="button"><SpotIcon name="play" size={13} /> Play {selectedItems.length > 0 ? "selected" : "all"}</button>
                  <button className="button button-quiet button-small" disabled={selectedItemIdList.length === 0 || actionPending} onClick={() => void runAction(() => queuePlaylist(selectedPlaylist.id, selectedItemIdList), "SpotDIY could not add that playlist to the queue.")} type="button"><SpotIcon name="queue" size={13} /> Add to queue</button>
                  <button className="button button-quiet button-small" disabled={selectedPlaylist.kind !== "normal" || actionPending} onClick={() => { const name = window.prompt("Name this one-shot branch:", `${selectedPlaylist.name} — review`); if (name) void runAction(() => createPlaylistBranch(selectedPlaylist.id, name), "SpotDIY could not create that playlist branch.", null); }} type="button"><SpotIcon name="spark" size={13} /> Create branch</button>
                  <span className="playlist-toolbar-note">Select items to target playback; no selection uses the full playlist.</span>
                </div>

                <DndContext onDragEnd={handleItemDragEnd} sensors={sensors}>
                  <SortableContext items={selectedPlaylist.items.map((item) => item.id)} strategy={verticalListSortingStrategy}>
                    <div className="playlist-item-list">
                      {selectedPlaylist.items.length === 0 ? <div className="queue-section-empty">No tracks yet. Add one from Your library.</div> : selectedPlaylist.items.map((item) => <SortablePlaylistItem editable={editablePlaylist} item={item} key={item.id} onInspect={(row) => openTrackInspector(row.trackId)} onPlayNext={(row) => { void playback.playNext(row.trackId, row.requestedSourceId); }} onPlayNow={(row) => { void playback.playNow(row.trackId, row.requestedSourceId); }} onQueue={(row) => { void playback.addToQueue(row.trackId, row.requestedSourceId); }} onRemove={(row) => void runAction(() => removePlaylistItem(selectedPlaylist.id, row.id), "SpotDIY could not remove that playlist item.")} onSelect={(row) => setSelectedItemIds((current) => { const next = new Set(current); if (next.has(row.id)) next.delete(row.id); else next.add(row.id); return next; })} playbackPending={playback.pending} selected={selectedItemIds.has(item.id)} />)}
                    </div>
                  </SortableContext>
                </DndContext>

                {selectedPlaylist.kind === "branch" && selectedPlaylist.branchStatus === "open" ? (
                  <section className="branch-review-card">
                    <div className="section-heading"><div><span className="eyebrow">BRANCH REVIEW</span><h3>Choose changes to merge</h3></div><span className="section-note">base revision {selectedPlaylist.baseParentRevision}</span></div>
                    {branchChanges.length === 0 ? <p className="queue-section-empty">This branch has no changes against its parent.</p> : <div className="branch-change-list">{branchChanges.map((change) => { const key = changeKey(change); return <label className="branch-change-row" key={key}><input checked={selectedChanges.has(key)} onChange={() => setSelectedChanges((current) => { const next = new Set(current); if (next.has(key)) next.delete(key); else next.add(key); return next; })} type="checkbox" /><span>{change.type === "add" ? `Add branch item ${change.branchItemId}` : change.type === "remove" ? `Remove base item ${change.baseItemId}` : `Move base item ${change.baseItemId} to position ${change.targetPosition + 1}`}</span></label>; })}</div>}
                    <div className="branch-review-actions"><button className="button button-primary button-small" disabled={selectedChangeList.length === 0 || actionPending} onClick={() => void runAction(() => mergeBranchChanges(selectedPlaylist.id, selectedChangeList), "SpotDIY could not merge the selected branch changes.", selectedPlaylist.parentPlaylistId)} type="button">Merge selected</button><button className="button button-quiet button-small" disabled={actionPending} onClick={() => { if (window.confirm(`Discard “${selectedPlaylist.name}”?`)) void runAction(() => discardPlaylistBranch(selectedPlaylist.id), "SpotDIY could not discard that branch.", null); }} type="button">Discard branch</button></div>
                  </section>
                ) : null}
              </>
            ) : null}
          </div>
        </section>
      )}
    </div>
  );
}
