import { useMemo, useState } from "react";

import { usePlayback } from "../../hooks/usePlayback";
import { isTauriRuntime, openProviderResult, pickDownloadDirectory, playSearchResult, queueSearchResultDownload, revealLocalFile, setSetting } from "../../services/ipc";
import { useUiStore } from "../../stores/ui-store";
import type { DownloadMode, SearchResult, SourceCapabilities } from "../../types/domain";
import { deriveSearchResultActions, downloadModesForResult, downloadReadinessReason, isDownloadFolderReadinessReason, type DownloadReadiness, type SearchResultActionId } from "../../features/actions/track-actions";
import { ContextActionMenu } from "../common/ContextActionMenu";
import { SpotIcon } from "../icons/SpotIcon";

interface SearchResultCardProps {
  result: SearchResult;
  capabilities?: SourceCapabilities;
  downloadReadiness?: DownloadReadiness;
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

export function SearchResultCard({ capabilities, downloadReadiness, result }: SearchResultCardProps) {
  const playback = usePlayback();
  const nativeRuntime = isTauriRuntime();
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const openSearchInspector = useUiStore((state) => state.openSearchInspector);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("audio");
  const localPlayable = result.provider === "local" && result.localTrackId !== null;
  const actions = useMemo(() => deriveSearchResultActions(result, { downloadReadiness, downloadsAvailable: capabilities?.downloads, nativeRuntime }), [capabilities?.downloads, downloadReadiness, nativeRuntime, result]);
  const action = (id: SearchResultActionId) => actions.find((item) => item.id === id);
  const duration = durationLabel(result.durationMs);
  const downloadAction = action("download");
  const openSourceAction = action("open-source");
  const downloadModes = downloadAction?.downloadModes ?? downloadModesForResult(result);
  const selectedDownloadReason = downloadModes.length > 0
    ? downloadReadinessReason(result.provider, downloadMode, {
      canonicalUrl: result.canonicalUrl,
      nativeRuntime,
      downloadsAvailable: capabilities?.downloads,
      downloadReadiness,
    })
    : undefined;
  const playAction = action("play");
  const downloadEnabled = Boolean(downloadAction?.enabled && (selectedDownloadReason === undefined || isDownloadFolderReadinessReason(selectedDownloadReason)));

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

  async function queueDownload(mode: DownloadMode) {
    const reason = downloadReadinessReason(result.provider, mode, {
      canonicalUrl: result.canonicalUrl,
      nativeRuntime,
      downloadsAvailable: capabilities?.downloads,
      downloadReadiness,
    });
    if (isDownloadFolderReadinessReason(reason)) {
      const directory = await pickDownloadDirectory();
      if (!directory) {
        return;
      }
      await setSetting({ key: "downloadsDirectory", value: directory });
    }
    await queueSearchResultDownload(result, mode);
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
          if (item.id === "play" && !localPlayable) void runAction(() => playSearchResult(result));
          if (item.id === "play-next" && localPlayable && localTrackId) void runAction(() => playback.playNext(localTrackId, result.localSourceId));
          if (item.id === "queue" && localPlayable && localTrackId) void runAction(() => playback.addToQueue(localTrackId, result.localSourceId));
          if (item.id === "open-location" && result.localSourceId) void runAction(() => revealLocalFile(result.localSourceId!));
          if (item.id === "open-source" && result.canonicalUrl) void runAction(() => openProviderResult(result.provider, result.canonicalUrl!));
          if (item.id === "download" && result.canonicalUrl) void runAction(() => queueDownload(downloadMode));
        },
      }))}
      className="search-result-context-menu"
      label={`Actions for ${result.title}`}
      showMoreButton={false}
    >
      <article className="search-result-card">
        <div className="search-result-main">
          <div className="search-result-heading"><strong title={result.title}>{result.title}</strong>{result.explicit ? <span className="search-result-explicit">E</span> : null}</div>
          <span className="search-result-artists">{result.artists.length > 0 ? result.artists.join(", ") : "Unknown artist"}</span>
          <span className="search-result-album">{result.album ?? "Single"}{duration ? ` · ${duration}` : ""}</span>
          {result.publishedAt ? <span className="search-result-date">{result.publishedAt.value}</span> : null}
          {actionError ? <span className="search-result-error" role="alert">{actionError}</span> : null}
        </div>
        <div className="search-result-actions">
          {localPlayable ? (
            <>
              <button aria-label="Play now" className="button button-small icon-only-button search-result-play" disabled={busy} onClick={() => void runAction(() => playback.playNow(result.localTrackId!, result.localSourceId))} title={`Play ${result.title} now`} type="button"><SpotIcon name="play" size={13} /></button>
              <button aria-label="Queue" className="button button-small icon-only-button" disabled={busy} onClick={() => void runAction(() => playback.addToQueue(result.localTrackId!, result.localSourceId))} title={`Add ${result.title} to queue`} type="button"><SpotIcon name="queue" size={13} /></button>
              <button aria-label="Play next" className="button button-small icon-only-button" disabled={busy} onClick={() => void runAction(() => playback.playNext(result.localTrackId!, result.localSourceId))} title={`Play ${result.title} next`} type="button"><SpotIcon name="next" size={13} /></button>
              <button aria-label="Inspect" className="button button-small icon-only-button" disabled={busy} onClick={inspect} title={`Inspect ${result.title}`} type="button"><SpotIcon name="info" size={13} /></button>
              {result.localSourceId ? <button aria-label="Show file" className="button button-small icon-only-button search-result-reveal" disabled={busy || !action("open-location")?.enabled} onClick={() => void runAction(() => revealLocalFile(result.localSourceId!))} title={action("open-location")?.reason ?? `Show ${result.title} file`} type="button"><SpotIcon name="folder" size={13} /></button> : null}
            </>
          ) : (
            <>
              <button aria-label="Play online" className="button button-small icon-only-button search-result-play" disabled={busy || !playAction?.enabled} onClick={() => void runAction(() => playSearchResult(result))} title={playAction?.enabled ? `Play ${result.title}` : playAction?.reason} type="button"><SpotIcon name="play" size={13} /></button>
              <button aria-label={result.provider === "spotify" ? "Open on Spotify" : "Open source"} className="button button-small icon-only-button" disabled={busy || !openSourceAction?.enabled} onClick={() => { if (result.canonicalUrl) void runAction(() => openProviderResult(result.provider, result.canonicalUrl!)); }} title={openSourceAction?.enabled ? (result.provider === "spotify" ? "Open on Spotify" : `Open ${result.title} source`) : openSourceAction?.reason} type="button"><SpotIcon name="arrow" size={13} /></button>
              <button aria-label="Inspect" className="button button-small icon-only-button" disabled={busy} onClick={inspect} title={`Inspect ${result.title}`} type="button"><SpotIcon name="info" size={13} /></button>
              {downloadAction?.enabled || downloadAction?.reason ? <div className="search-result-download">{downloadModes.length > 1 ? <select aria-label={`Download mode for ${result.title}`} disabled={busy || !downloadAction.enabled} onChange={(event) => setDownloadMode(event.target.value as DownloadMode)} title={selectedDownloadReason ?? downloadAction.reason} value={downloadMode}>{downloadModes.map((mode) => { const modeReason = downloadReadinessReason(result.provider, mode, { canonicalUrl: result.canonicalUrl, nativeRuntime, downloadsAvailable: capabilities?.downloads, downloadReadiness }); return <option disabled={Boolean(modeReason && !isDownloadFolderReadinessReason(modeReason))} key={mode} value={mode}>{mode === "audio" ? "Audio" : "Video"}</option>; })}</select> : null}<button aria-label={downloadModes.length === 1 ? "Download audio" : "Download"} className="button button-small icon-only-button" disabled={busy || !downloadEnabled} onClick={() => void runAction(() => queueDownload(downloadMode))} title={downloadEnabled ? (isDownloadFolderReadinessReason(selectedDownloadReason) ? "Choose a download folder, then queue this download" : `Download ${result.title}`) : selectedDownloadReason ?? downloadAction.reason} type="button"><SpotIcon name="download" size={13} /></button></div> : null}
            </>
          )}
        </div>
      </article>
    </ContextActionMenu>
  );
}
