import { useMemo, useState } from "react";

import { usePlayback } from "../../hooks/usePlayback";
import { isTauriRuntime, openProviderResult, queueSearchResultDownload, revealLocalFile } from "../../services/ipc";
import { useUiStore } from "../../stores/ui-store";
import type { DownloadMode, SearchResult, SourceCapabilities } from "../../types/domain";
import { deriveSearchResultActions, type SearchResultActionId } from "../../features/actions/track-actions";
import { ContextActionMenu } from "../common/ContextActionMenu";
import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";

interface SearchResultCardProps {
  result: SearchResult;
  capabilities?: SourceCapabilities;
}

function durationLabel(durationMs: number | null): string | null {
  if (durationMs === null) return null;
  const totalSeconds = Math.floor(durationMs / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

function resultErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  return "That action could not be completed.";
}

export function SearchResultCard({ capabilities, result }: SearchResultCardProps) {
  const playback = usePlayback();
  const nativeRuntime = isTauriRuntime();
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const openSearchInspector = useUiStore((state) => state.openSearchInspector);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("audio");
  const localPlayable = result.provider === "local" && result.localTrackId !== null;
  const actions = useMemo(() => deriveSearchResultActions(result, { downloadsAvailable: capabilities?.downloads, nativeRuntime }), [capabilities?.downloads, nativeRuntime, result]);
  const action = (id: SearchResultActionId) => actions.find((item) => item.id === id);
  const duration = durationLabel(result.durationMs);
  const downloadAction = action("download");
  const openSourceAction = action("open-source");

  async function runAction(run: () => Promise<unknown>) {
    setBusy(true);
    setActionError(null);
    try {
      await run();
    } catch (error) {
      setActionError(resultErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const localTrackId = result.localTrackId;
  const inspect = () => {
    if (localPlayable && localTrackId) {
      openTrackInspector(localTrackId);
    } else {
      openSearchInspector(result);
    }
  };

  return (
    <ContextActionMenu
      actions={actions.map((item) => ({
        id: item.id,
        label: item.label,
        disabled: !item.enabled || busy,
        disabledReason: item.reason,
        onSelect: () => {
          if (item.id === "inspect") inspect();
          if (item.id === "play" && localPlayable && localTrackId) void runAction(() => playback.playNow(localTrackId, result.localSourceId));
          if (item.id === "play-next" && localPlayable && localTrackId) void runAction(() => playback.playNext(localTrackId, result.localSourceId));
          if (item.id === "queue" && localPlayable && localTrackId) void runAction(() => playback.addToQueue(localTrackId, result.localSourceId));
          if (item.id === "open-location" && result.localSourceId) void runAction(() => revealLocalFile(result.localSourceId!));
          if (item.id === "open-source" && result.canonicalUrl) void runAction(() => openProviderResult(result.provider, result.canonicalUrl!));
          if (item.id === "download" && result.canonicalUrl) void runAction(() => queueSearchResultDownload(result, downloadMode));
        },
      }))}
      className="search-result-context-menu"
      label={`Actions for ${result.title}`}
    >
      <article className="search-result-card">
        <div className="search-result-art">
          {result.artworkUrl ? <img alt={`${result.title} artwork`} src={result.artworkUrl} /> : <SpotIcon name="library" size={18} />}
        </div>
        <div className="search-result-main">
          <div className="search-result-heading"><ProviderBadge kind={result.provider} /><strong title={result.title}>{result.title}</strong>{result.explicit ? <span className="search-result-explicit">E</span> : null}</div>
          <span className="search-result-artists">{result.artists.length > 0 ? result.artists.join(", ") : "Unknown artist"}</span>
          <span className="search-result-album">{result.album ?? "Single"}{duration ? ` · ${duration}` : ""}</span>
          {result.publishedAt ? <span className="search-result-date">{result.publishedAt.value}</span> : null}
          {actionError ? <span className="search-result-error" role="alert">{actionError}</span> : null}
        </div>
        <div className="search-result-actions">
          {localPlayable ? (
            <>
              <button className="button button-small search-result-play" disabled={busy} onClick={() => void runAction(() => playback.playNow(result.localTrackId!, result.localSourceId))} type="button"><SpotIcon name="play" size={13} /> Play now</button>
              <button className="button button-small" disabled={busy} onClick={() => void runAction(() => playback.addToQueue(result.localTrackId!, result.localSourceId))} type="button">Queue</button>
              <button className="button button-small" disabled={busy} onClick={() => void runAction(() => playback.playNext(result.localTrackId!, result.localSourceId))} type="button">Play next</button>
              <button className="button button-small" disabled={busy} onClick={inspect} type="button"><SpotIcon name="info" size={13} /> Inspect</button>
              {result.localSourceId ? <button className="text-link search-result-reveal" disabled={busy || !action("open-location")?.enabled} onClick={() => void runAction(() => revealLocalFile(result.localSourceId!))} title={action("open-location")?.reason} type="button">Show file</button> : null}
            </>
          ) : (
            <>
              <button className="button button-small" disabled={busy || !openSourceAction?.enabled} onClick={() => { if (result.canonicalUrl) void runAction(() => openProviderResult(result.provider, result.canonicalUrl!)); }} title={openSourceAction?.enabled ? "Open the validated provider source" : openSourceAction?.reason} type="button">{result.provider === "spotify" ? "Open on Spotify" : "Open source"}</button>
              <button className="button button-small" onClick={inspect} type="button"><SpotIcon name="info" size={13} /> Inspect</button>
              {downloadAction?.enabled || downloadAction?.reason ? <div className="search-result-download"><select aria-label={`Download mode for ${result.title}`} disabled={busy || !downloadAction.enabled} onChange={(event) => setDownloadMode(event.target.value as DownloadMode)} title={downloadAction.reason} value={downloadMode}><option value="audio">Audio</option><option value="video">Video</option></select><button className="button button-small" disabled={busy || !downloadAction.enabled} onClick={() => void runAction(() => queueSearchResultDownload(result, downloadMode))} title={downloadAction.enabled ? "Queue this provider download" : downloadAction.reason} type="button"><SpotIcon name="download" size={13} /> Download</button></div> : null}
              <span className="search-result-capability-note">{result.provider === "spotify" ? "Metadata only · Spotify downloads are not supported" : "Online playback is not implemented"}</span>
            </>
          )}
        </div>
      </article>
    </ContextActionMenu>
  );
}
