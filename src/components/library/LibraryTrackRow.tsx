import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";
import { isTauriRuntime } from "../../services/ipc";
import type { LibraryTrack, SourceId } from "../../types/domain";

interface LibraryTrackRowProps {
  track: LibraryTrack;
  revealPending: boolean;
  onReveal: (sourceId: SourceId) => void;
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
  return [
    track.codec ?? track.container,
    track.bitrateKbps === null ? null : `${track.bitrateKbps} kbps`,
    formatSampleRate(track.sampleRateHz),
    track.bitDepth === null ? null : `${track.bitDepth}-bit`,
    formatDuration(track.durationMs),
  ].filter((fact): fact is string => Boolean(fact));
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

export function LibraryTrackRow({ track, revealPending, onReveal }: LibraryTrackRowProps) {
  const artworkSource = isTauriRuntime() && track.artworkPath
    ? convertFileSrc(track.artworkPath, "asset")
    : null;
  const [artworkFailed, setArtworkFailed] = useState(false);

  useEffect(() => {
    setArtworkFailed(false);
  }, [artworkSource]);

  const detail = track.availabilityDetail ?? track.statusDetail;
  const canReveal = track.available && track.indexStatus !== "missing";
  const facts = qualityFacts(track);

  return (
    <article
      className={`library-track-row library-track-${track.indexStatus}${track.available ? "" : " library-track-unavailable"}`}
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
      <div className="library-track-actions">
        <button
          aria-label={`Play ${track.title}`}
          className="player-play-button library-track-play"
          disabled
          title="Playback arrives in Plan 04"
          type="button"
        >
          <SpotIcon name="play" size={14} />
        </button>
        <button
          aria-label={`Open file location for ${track.title}`}
          className="button button-quiet button-small"
          disabled={!canReveal || revealPending}
          onClick={() => onReveal(track.sourceId)}
          title={canReveal ? "Reveal this file in Explorer" : "The local file is unavailable"}
          type="button"
        >
          <SpotIcon name="arrow" size={14} />
          Open location
        </button>
      </div>
    </article>
  );
}
