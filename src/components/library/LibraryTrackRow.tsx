import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { ProviderBadge } from "../common/ProviderBadge";
import { ContextActionMenu } from "../common/ContextActionMenu";
import { SpotIcon } from "../icons/SpotIcon";
import { isTauriRuntime } from "../../services/ipc";
import { useUiStore } from "../../stores/ui-store";
import type { LibraryTrack, Playlist, SourceId } from "../../types/domain";

interface LibraryTrackRowProps {
  track: LibraryTrack;
  deletePending: boolean;
  revealPending: boolean;
  onDelete: (track: LibraryTrack) => void;
  onReveal: (sourceId: SourceId) => void;
  onRename: (track: LibraryTrack, name: string) => void;
  onPlayNow: (track: LibraryTrack) => void;
  onPlayNext: (track: LibraryTrack) => void;
  onAddToQueue: (track: LibraryTrack) => void;
  playbackPending: boolean;
  playbackEnabled: boolean;
  current: boolean;
  pinned: boolean;
  playlists: Playlist[];
  playlistPending: boolean;
  onPin: (track: LibraryTrack) => void;
  onPlaylist: (track: LibraryTrack, playlistId: Playlist["id"]) => void;
}

function formatDuration(durationMs: number | null): string | null {
  if (durationMs === null) {
    return null;
  }
  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function formatSampleRate(sampleRateHz: number | null): string | null {
  if (sampleRateHz === null) {
    return null;
  }
  const kilohertz = sampleRateHz / 1000;
  return `${Number.isInteger(kilohertz) ? kilohertz : kilohertz.toFixed(1)} kHz`;
}

function qualityFacts(track: LibraryTrack): string[] {
  return Array.from(new Set([
    track.codec ?? track.container,
    track.bitrateKbps === null ? null : `${track.bitrateKbps} kbps`,
    formatSampleRate(track.sampleRateHz),
    track.bitDepth === null ? null : `${track.bitDepth}-bit`,
    formatDuration(track.durationMs),
  ].filter((fact): fact is string => Boolean(fact))));
}

function statusLabel(track: LibraryTrack): string {
  if (!track.available || track.indexStatus === "missing") {
    return "Unavailable";
  }
  switch (track.indexStatus) {
    case "error":
      return "Metadata issue";
    case "pending":
      return "Pending";
    default:
      return "Indexed";
  }
}

export function LibraryTrackRow({
  track,
  deletePending,
  revealPending,
  onDelete,
  onReveal,
  onRename,
  onPlayNow,
  onPlayNext,
  onAddToQueue,
  playbackPending,
  playbackEnabled,
  current,
  pinned,
  playlists,
  playlistPending,
  onPin,
  onPlaylist,
}: LibraryTrackRowProps) {
  const artworkSource = isTauriRuntime() && track.artworkPath
    ? convertFileSrc(track.artworkPath, "asset")
    : null;
  const [artworkFailed, setArtworkFailed] = useState(false);
  const [playlistMenuOpen, setPlaylistMenuOpen] = useState(false);
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);

  useEffect(() => {
    setArtworkFailed(false);
  }, [artworkSource]);

  const detail = track.availabilityDetail ?? track.statusDetail;
  const canReveal = isTauriRuntime() && track.available && track.indexStatus !== "missing";
  const canDelete = canReveal;
  const canPlay = playbackEnabled && track.available && track.indexStatus === "indexed";
  const facts = qualityFacts(track);
  const rename = () => {
    const filename = track.path.toString().split(/[\\/]/).pop() ?? track.title;
    const currentName = filename.replace(/\.[^.]+$/, "");
    const nextName = window.prompt("Rename file (the extension is preserved):", currentName);
    if (nextName?.trim()) {
      onRename(track, nextName.trim());
    }
  };

  return (
    <article
      className={`library-track-row library-track-${track.indexStatus}${track.available ? "" : " library-track-unavailable"}${current ? " library-track-current" : ""}`}
      data-testid={`library-track-${track.trackId}`}
    >
      <div className="library-track-art" aria-hidden="true">
        {artworkSource && !artworkFailed ? (
          <img alt="" loading="lazy" onError={() => setArtworkFailed(true)} src={artworkSource} />
        ) : (
          <SpotIcon name="library" size={21} />
        )}
      </div>
      <div className="library-track-copy">
        <div className="library-track-title-line">
          <strong title={track.title}>{track.title}</strong>
          <ProviderBadge kind="local" />
          <span className={`library-index-chip library-index-${track.indexStatus}`}>{statusLabel(track)}</span>
        </div>
        <span className="library-track-artists">{track.artists.length > 0 ? track.artists.join(" · ") : "Unknown artist"}</span>
        <span className="library-track-album">{track.album ?? "Album unavailable"}</span>
        {detail ? <span className="library-track-detail">{detail}</span> : null}
      </div>
      <div className="library-track-quality" aria-label="Measured file quality">
        {facts.length > 0 ? facts.map((fact) => <span key={fact}>{fact}</span>) : <span>Quality unavailable</span>}
      </div>
      <ContextActionMenu
        actions={[
          { id: "play", label: "Play now", onSelect: () => onPlayNow(track), disabled: !canPlay || playbackPending, disabledReason: "Track unavailable" },
          { id: "play-next", label: "Play next", onSelect: () => onPlayNext(track), disabled: !canPlay || playbackPending, disabledReason: "Track unavailable" },
          { id: "queue", label: "Add to queue", onSelect: () => onAddToQueue(track), disabled: !canPlay || playbackPending, disabledReason: "Track unavailable" },
          { id: "inspect", label: "Inspect", onSelect: () => openTrackInspector(track.trackId) },
          { id: "reveal", label: "Open location", onSelect: () => onReveal(track.sourceId), disabled: !canReveal || revealPending, disabledReason: "File unavailable" },
          { id: "rename", label: "Rename file", onSelect: rename, disabled: !canReveal || revealPending, disabledReason: "File unavailable" },
          { id: "delete", label: "Delete file", onSelect: () => onDelete(track), danger: true, disabled: !canDelete || deletePending, disabledReason: "File unavailable" },
        ]}
        className="library-track-context-menu"
        label={`Actions for ${track.title}`}
        showMoreButton={false}
      >
      <div className="library-track-actions">
        <button
          aria-label={`Play now ${track.title}`}
          className="player-play-button library-track-play"
          disabled={!canPlay || playbackPending}
          onClick={() => onPlayNow(track)}
          title={canPlay ? "Replace the queue and start this track now" : "This track cannot be played right now"}
          type="button"
        >
          <SpotIcon name="play" size={14} />
        </button>
        <button
          aria-label={`Play next ${track.title}`}
          className="button button-quiet button-small icon-only-button"
          disabled={!canPlay || playbackPending}
          onClick={() => onPlayNext(track)}
          title={canPlay ? "Insert this track immediately after the current track" : "This track cannot be played right now"}
          type="button"
        >
          <SpotIcon name="next" size={14} />
          Play next
        </button>
        <button
          aria-label={`Add ${track.title} to queue`}
          className="button button-quiet button-small icon-only-button"
          disabled={!canPlay || playbackPending}
          onClick={() => onAddToQueue(track)}
          title={canPlay ? "Append this track to the persistent queue" : "This track cannot be played right now"}
          type="button"
        >
          <SpotIcon name="queue" size={14} />
          Add to queue
        </button>
        <button
          aria-label={`Inspect ${track.title}`}
          className="button button-quiet button-small icon-only-button"
          onClick={() => openTrackInspector(track.trackId)}
          type="button"
        >
          <SpotIcon name="info" size={14} />
          Inspect
        </button>
        <button
          aria-label={`Open file location for ${track.title}`}
          className="button button-quiet button-small icon-only-button"
          disabled={!canReveal || revealPending}
          onClick={() => onReveal(track.sourceId)}
          title={canReveal ? "Reveal this file in Explorer" : "The local file is unavailable"}
          type="button"
        >
          <SpotIcon name="folder" size={14} />
          Open location
        </button>
        <button
          aria-label={`Rename ${track.title}`}
          className="button button-quiet button-small icon-only-button"
          disabled={!canReveal || revealPending}
          onClick={rename}
          title={canReveal ? "Rename this music file" : "The local file is unavailable"}
          type="button"
        >
          <SpotIcon name="edit" size={14} />
        </button>
        <button
          aria-label={`Delete ${track.title}`}
          className="button button-quiet button-small playlist-danger"
          disabled={!canDelete || deletePending}
          onClick={() => onDelete(track)}
          title={canDelete ? "Permanently delete this local file" : "The local file is unavailable"}
          type="button"
        >
          <SpotIcon name="trash" size={14} />
          Delete
        </button>
        <div className="library-track-library-actions" aria-label={`Library actions for ${track.title}`}>
          <button
            aria-pressed={pinned}
            aria-label={pinned ? `Unpin ${track.title}` : `Pin ${track.title} to top`}
            className={`button button-quiet button-small icon-only-button${pinned ? " library-pin-active" : ""}`}
            onClick={() => onPin(track)}
            title={pinned ? "Unpin from the top of the library" : "Pin to the top of the library"}
            type="button"
          >
            <SpotIcon name="pin" size={14} />
            {pinned ? "Pinned" : "Pin to top"}
          </button>
          <button
            aria-expanded={playlistMenuOpen}
            aria-label={`Add ${track.title} to a playlist`}
            className="button button-quiet button-small icon-only-button"
            disabled={!isTauriRuntime() || playlistPending || playlists.length === 0}
            onClick={() => setPlaylistMenuOpen((open) => !open)}
            title={playlists.length > 0 ? "Add to playlist" : "Create a playlist first"}
            type="button"
          >
            <SpotIcon name="playlist" size={14} />
            Add playlist
          </button>
          {playlistMenuOpen ? (
            <div aria-label={`Choose a playlist for ${track.title}`} className="library-playlist-menu" role="dialog">
              {playlists.map((playlist) => (
                <button key={playlist.id} disabled={playlistPending} onClick={() => { onPlaylist(track, playlist.id); setPlaylistMenuOpen(false); }} type="button">
                  <SpotIcon name="playlist" size={13} />
                  <span>{playlist.name}</span>
                </button>
              ))}
            </div>
          ) : null}
        </div>
      </div>
      </ContextActionMenu>
    </article>
  );
}
