import { useState } from "react";

import { openProviderResult, revealLocalFile } from "../../services/ipc";
import { usePlayback } from "../../hooks/usePlayback";
import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";
import type { SearchResult } from "../../types/domain";

interface SearchResultCardProps {
  result: SearchResult;
}

function durationLabel(durationMs: number | null): string | null {
  if (durationMs === null) {
    return null;
  }
  const totalSeconds = Math.floor(durationMs / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

function resultErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "That action could not be completed.";
}

export function SearchResultCard({ result }: SearchResultCardProps) {
  const playback = usePlayback();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const localPlayable = result.provider === "local" && result.localTrackId !== null;
  const duration = durationLabel(result.durationMs);

  async function runAction(action: () => Promise<unknown>) {
    setBusy(true);
    setActionError(null);
    try {
      await action();
    } catch (error) {
      setActionError(resultErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className="search-result-card">
      <div className="search-result-art">
        {result.artworkUrl ? <img alt={`${result.title} artwork`} src={result.artworkUrl} /> : <SpotIcon name="library" size={18} />}
      </div>
      <div className="search-result-main">
        <div className="search-result-heading">
          <ProviderBadge kind={result.provider} />
          <strong title={result.title}>{result.title}</strong>
          {result.explicit ? <span className="search-result-explicit">E</span> : null}
        </div>
        <span className="search-result-artists">{result.artists.length > 0 ? result.artists.join(", ") : "Unknown artist"}</span>
        <span className="search-result-album">{result.album ?? "Single"}{duration ? ` · ${duration}` : ""}</span>
        {result.publishedAt ? <span className="search-result-date">{result.publishedAt.value}</span> : null}
        {actionError ? <span className="search-result-error" role="alert">{actionError}</span> : null}
      </div>
      <div className="search-result-actions">
        {localPlayable ? (
          <>
            <button className="button button-small search-result-play" disabled={busy} onClick={() => runAction(() => playback.playNow(result.localTrackId!, result.localSourceId))} type="button"><SpotIcon name="play" size={13} /> Play now</button>
            <button className="button button-small" disabled={busy} onClick={() => runAction(() => playback.addToQueue(result.localTrackId!, result.localSourceId))} type="button">Queue</button>
            <button className="button button-small" disabled={busy} onClick={() => runAction(() => playback.playNext(result.localTrackId!, result.localSourceId))} type="button">Play next</button>
            {result.localSourceId ? <button className="text-link search-result-reveal" disabled={busy} onClick={() => runAction(() => revealLocalFile(result.localSourceId!))} type="button">Show file</button> : null}
          </>
        ) : result.canonicalUrl ? (
          <button className="button button-small" disabled={busy} onClick={() => runAction(() => openProviderResult(result.provider, result.canonicalUrl!))} type="button">{result.provider === "spotify" ? "Open on Spotify" : "Open source"}</button>
        ) : null}
      </div>
    </article>
  );
}
