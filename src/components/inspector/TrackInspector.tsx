import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";

import { useTrackInspector } from "../../hooks/useTrackInspector";
import { useAppStatus } from "../../hooks/useAppStatus";
import { usePlayback } from "../../hooks/usePlayback";
import {
  IpcError,
  isTauriRuntime,
  openProviderResult,
  pickDownloadDirectory,
  playSearchResult,
  queueSearchResultDownload,
  queueSourceDownload,
  revealLocalFile,
  setSetting,
} from "../../services/ipc";
import type {
  DownloadMode,
  ProviderKind,
  SearchResult,
  TrackId,
  TrackInspector as TrackInspectorDto,
  TrackInspectorSource,
} from "../../types/domain";
import { deriveSearchResultActions, downloadModesForProvider, downloadReadinessReason, isDownloadFolderReadinessReason, type DownloadReadiness } from "../../features/actions/track-actions";
import { InspectorPanel, type InspectorSection } from "./InspectorPanel";
import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";

interface TrackInspectorProps {
  trackId: TrackId;
  onClose: () => void;
  manageEscape?: boolean;
}

interface SearchResultInspectorProps {
  result: SearchResult;
  onClose: () => void;
  manageEscape?: boolean;
}

function formatDuration(durationMs: number | null): string {
  if (durationMs === null) {
    return "Unavailable";
  }
  const seconds = Math.floor(durationMs / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function formatSampleRate(sampleRateHz: number | null): string | null {
  if (sampleRateHz === null) {
    return null;
  }
  const value = sampleRateHz / 1000;
  return `${Number.isInteger(value) ? value : value.toFixed(1)} kHz`;
}

function providerName(provider: ProviderKind): string {
  switch (provider) {
    case "local":
      return "Local library";
    case "youtube":
      return "YouTube";
    case "soundcloud":
      return "SoundCloud";
    case "spotify":
      return "Spotify";
  }
}

function versionLabel(qualifiers: string[]): string {
  return qualifiers.length > 0 ? qualifiers.join(" · ") : "No version qualifier";
}

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

function previewPlaybackEnabled(): boolean {
  return !isTauriRuntime() && import.meta.env.DEV && import.meta.env.VITE_SPOTDIY_E2E === "1";
}

function qualityFacts(source: TrackInspectorSource): string[] {
  return Array.from(new Set([
    source.quality.codec,
    source.quality.container,
    source.quality.bitrateKbps === null ? null : `${source.quality.bitrateKbps} kbps`,
    formatSampleRate(source.quality.sampleRateHz),
    source.quality.bitDepth === null ? null : `${source.quality.bitDepth}-bit`,
  ].filter((value): value is string => Boolean(value))));
}

function sourceAvailability(source: TrackInspectorSource): string {
  if (!source.available) {
    return source.availabilityDetail ?? "Source unavailable.";
  }
  if (!source.capabilities.playback && source.provider !== "local") {
    return source.availabilityDetail ?? "Provider playback is unavailable.";
  }
  return "Available";
}

function CapabilityList({ source }: { source: TrackInspectorSource }) {
  const capabilities = [
    ["search", source.capabilities.search],
    ["metadata", source.capabilities.metadata],
    ["artwork", source.capabilities.artwork],
    ["playback", source.capabilities.playback],
    ["lyrics", source.capabilities.lyrics],
    ["downloads", source.capabilities.downloads],
  ] as const;
  return (
    <div className="inspector-capability-list" aria-label={`${providerName(source.provider)} capabilities`}>
      {capabilities.map(([name, enabled]) => <span className={enabled ? "inspector-capability-on" : "inspector-capability-off"} key={name}>{name}{enabled ? "" : " · unavailable"}</span>)}
    </div>
  );
}

function SourceCard({
  current,
  downloadMode,
  downloadReadiness,
  onAction,
  source,
}: {
  current: boolean;
  downloadMode: DownloadMode;
  downloadReadiness?: DownloadReadiness;
  onAction: (source: TrackInspectorSource, action: "play" | "play-next" | "queue" | "switch" | "reveal" | "open" | "download" | "lyrics") => void;
  source: TrackInspectorSource;
}) {
  const mpvReady = source.provider === "local" || previewPlaybackEnabled() || downloadReadiness?.mpvStatus === "ready";
  const canPlay = source.available && source.capabilities.playback && mpvReady && (isTauriRuntime() || previewPlaybackEnabled());
  const canReveal = source.provider === "local" && source.available && isTauriRuntime();
  const canOpen = source.provider !== "local" && source.canonicalUrl !== null;
  const downloadModes = downloadModesForProvider(source.provider);
  const sourceDownloadMode = downloadModes.includes(downloadMode) ? downloadMode : "audio";
  const downloadReason = downloadModes.length > 0
    ? downloadReadinessReason(source.provider, sourceDownloadMode, {
      canonicalUrl: source.canonicalUrl,
      nativeRuntime: isTauriRuntime(),
      downloadsAvailable: source.capabilities.downloads,
      downloadReadiness,
    })
    : undefined;
  const canDownload = downloadModes.length > 0 && (downloadReason === undefined || isDownloadFolderReadinessReason(downloadReason));
  const canLyrics = current && source.capabilities.lyrics;
  const playReason = !source.available
    ? sourceAvailability(source)
    : !source.capabilities.playback
      ? sourceAvailability(source)
      : !isTauriRuntime() && !previewPlaybackEnabled()
        ? "Playback controls require the native app."
        : source.provider !== "local" && downloadReadiness?.mpvStatus !== "ready"
          ? "The online playback engine is not ready."
        : undefined;

  return (
    <article className="inspector-source-card" data-provider={source.provider}>
      <div className="inspector-source-heading">
        <div><ProviderBadge kind={source.provider} /><strong>{providerName(source.provider)}</strong></div>
        <span className={`inspector-availability${source.available ? " inspector-availability-available" : " inspector-availability-unavailable"}`}>{source.available ? "Available" : "Unavailable"}</span>
      </div>
      <dl className="inspector-fact-grid">
        <div><dt>Provider item</dt><dd>{source.providerItemId}</dd></div>
        <div><dt>Availability</dt><dd>{sourceAvailability(source)}</dd></div>
        <div><dt>Duration</dt><dd>{formatDuration(source.durationMs)}</dd></div>
        <div><dt>Version</dt><dd>{versionLabel(source.versionQualifiers)}</dd></div>
      </dl>
      {qualityFacts(source).length > 0 ? <div className="inspector-quality-facts">{qualityFacts(source).map((fact) => <span key={fact}>{fact}</span>)}</div> : <p className="inspector-muted">Measured local quality unavailable.</p>}
      <CapabilityList source={source} />
      <div className="inspector-source-actions">
        {current ? (
          <button aria-label="Play now" className="button button-primary button-small icon-only-button" disabled={!canPlay} onClick={() => onAction(source, "play")} title={canPlay ? "Play this source now" : playReason} type="button"><SpotIcon name="play" size={13} /> Play now</button>
        ) : (
          <button aria-label="Switch source" className="button button-primary button-small icon-only-button" disabled={!canPlay} onClick={() => onAction(source, "switch")} title={canPlay ? "Switch the current track to this source" : playReason} type="button"><SpotIcon name="refresh" size={13} /> Switch source</button>
        )}
        <button aria-label="Play next" className="button button-quiet button-small icon-only-button" disabled={!canPlay} onClick={() => onAction(source, "play-next")} title={canPlay ? "Play this source after the current track" : playReason} type="button"><SpotIcon name="next" size={13} /> Play next</button>
        <button aria-label="Add to queue" className="button button-quiet button-small icon-only-button" disabled={!canPlay} onClick={() => onAction(source, "queue")} title={canPlay ? "Add this source to the persistent queue" : playReason} type="button"><SpotIcon name="queue" size={13} /> Add to queue</button>
        <button aria-label="Open location" className="button button-quiet button-small icon-only-button" disabled={!canReveal} onClick={() => onAction(source, "reveal")} title={canReveal ? "Reveal this managed local file" : source.provider === "local" ? "Local file reveal requires the native app and an available file." : "Only local sources have managed file locations."} type="button"><SpotIcon name="folder" size={13} /> Open location</button>
        <button aria-label="Open source" className="button button-quiet button-small icon-only-button" disabled={!canOpen} onClick={() => onAction(source, "open")} title={canOpen ? "Open the validated provider source" : "No validated provider URL is available."} type="button"><SpotIcon name="arrow" size={13} /> Open source</button>
        {downloadModes.length > 0 ? <button aria-label={downloadModes.length === 1 ? "Download audio" : "Download"} className="button button-quiet button-small icon-only-button" disabled={!canDownload} onClick={() => onAction(source, "download")} title={canDownload ? (isDownloadFolderReadinessReason(downloadReason) ? "Choose a download folder, then queue this download" : "Queue this managed provider download") : downloadReason} type="button"><SpotIcon name="download" size={13} /> {downloadModes.length === 1 ? "Download audio" : "Download"}</button> : null}
        <button aria-label="Lyrics" className="button button-quiet button-small icon-only-button" disabled={!canLyrics} onClick={() => onAction(source, "lyrics")} title={canLyrics ? "Open lyrics for the current source" : current ? "This source does not advertise lyrics." : "Play this source first to open synchronized lyrics."} type="button"><SpotIcon name="lyrics" size={13} /> Lyrics</button>
      </div>
    </article>
  );
}

function Overview({ inspector }: { inspector: TrackInspectorDto }) {
  return (
    <div className="inspector-overview">
      <div className="inspector-title-block"><span className="eyebrow">PERSISTED LOCAL TRACK</span><h3>{inspector.title}</h3><p>{inspector.artists.join(" · ") || "Unknown artist"}{inspector.album ? ` · ${inspector.album}` : ""}</p></div>
      <dl className="inspector-fact-grid inspector-fact-grid-wide">
        <div><dt>Track ID</dt><dd>{inspector.trackId}</dd></div>
        <div><dt>Duration</dt><dd>{formatDuration(inspector.durationMs)}</dd></div>
        <div><dt>Version</dt><dd>{versionLabel(inspector.versionQualifiers)}</dd></div>
        <div><dt>Preferred source</dt><dd>{inspector.preferredSourceId ?? "Not selected"}</dd></div>
      </dl>
    </div>
  );
}

function CollectionState({ inspector }: { inspector: TrackInspectorDto }) {
  const state = inspector.collectionState;
  return (
    <div className="inspector-collection-state">
      <div className="inspector-state-row"><span>Liked</span><strong>{state.liked ? "Yes" : "No"}</strong></div>
      <div className="inspector-state-row"><span>Rating</span><strong>{state.rating === null ? "Not rated" : `${state.rating}/5`}</strong></div>
      <div className="inspector-state-row"><span>Inbox</span><strong>{state.inInbox ? "In Inbox" : "Not in Inbox"}</strong></div>
      <div className="inspector-state-row"><span>Tags</span><strong>{state.tags.length > 0 ? state.tags.map((tag) => tag.name).join(" · ") : "No tags"}</strong></div>
      <div className="inspector-state-row"><span>Playlists</span><strong>{state.playlistMemberships.length > 0 ? state.playlistMemberships.map((playlist) => playlist.name).join(" · ") : "No playlist memberships"}</strong></div>
    </div>
  );
}

function QualityState({ inspector, currentSourceId }: { inspector: TrackInspectorDto; currentSourceId: string | null }) {
  const current = inspector.sources.find((source) => source.sourceId === currentSourceId) ?? inspector.sources.find((source) => source.sourceId === inspector.preferredSourceId) ?? null;
  return (
    <div className="inspector-quality-state">
      <p className="inspector-muted">Measured file quality is shown only when the source provides it. Provider capability and file quality are separate facts.</p>
      <div className="inspector-quality-table" role="table" aria-label="Measured source quality">
        <div className="inspector-quality-row inspector-quality-heading" role="row"><span>Source</span><span>Measured facts</span></div>
        {inspector.sources.map((source) => <div className={`inspector-quality-row${source.sourceId === current?.sourceId ? " inspector-quality-current" : ""}`} key={source.sourceId} role="row"><span><ProviderBadge kind={source.provider} />{source.sourceId === current?.sourceId ? "Current" : providerName(source.provider)}</span><span>{qualityFacts(source).join(" · ") || "Quality unavailable"}</span></div>)}
      </div>
    </div>
  );
}

function inspectorSections(inspector: TrackInspectorDto, currentSourceId: string | null, sourceAction: (source: TrackInspectorSource, action: "play" | "play-next" | "queue" | "switch" | "reveal" | "open" | "download" | "lyrics") => void, downloadMode: DownloadMode, onDownloadModeChange: (mode: DownloadMode) => void, downloadReadiness?: DownloadReadiness): InspectorSection[] {
  const hasVideoDownload = inspector.sources.some((source) => source.capabilities.downloads && downloadModesForProvider(source.provider).length > 1);
  return [
    { id: "overview", title: "OVERVIEW", content: <Overview inspector={inspector} /> },
    { id: "sources", title: "SOURCES", content: <div className="inspector-source-list">{hasVideoDownload ? <label className="inspector-download-control">Provider download format<select aria-label="Download mode" onChange={(event) => onDownloadModeChange(event.target.value as DownloadMode)} value={downloadMode}><option value="audio">Audio</option><option value="video">Video</option></select></label> : null}{inspector.sources.map((source) => <SourceCard current={source.sourceId === currentSourceId} downloadMode={downloadMode} downloadReadiness={downloadReadiness} key={source.sourceId} onAction={sourceAction} source={source} />)}</div> },
    { id: "quality", title: "QUALITY", content: <QualityState currentSourceId={currentSourceId} inspector={inspector} /> },
    { id: "collection", title: "COLLECTION", content: <CollectionState inspector={inspector} /> },
    { id: "capabilities", title: "CAPABILITIES", content: <div className="inspector-capability-source-list">{inspector.sources.map((source) => <div className="inspector-capability-source" key={source.sourceId}><div><ProviderBadge kind={source.provider} /><strong>{providerName(source.provider)}</strong></div><CapabilityList source={source} /></div>)}</div> },
  ];
}

export function TrackInspector({ manageEscape = false, onClose, trackId }: TrackInspectorProps) {
  const navigate = useNavigate();
  const playback = usePlayback();
  const appStatus = useAppStatus();
  const query = useTrackInspector(trackId);
  const [actionError, setActionError] = useState<string | null>(null);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("audio");
  const inspector = query.data;
  const currentTrack = playback.snapshot.currentTrackId === trackId;
  const currentSourceId = currentTrack ? playback.snapshot.currentSourceId : null;
  const downloadReadiness = useMemo<DownloadReadiness | undefined>(() => appStatus.data ? {
    ytDlpStatus: appStatus.data.mediaTools.ytDlp.status,
    ffmpegStatus: appStatus.data.mediaTools.ffmpeg.status,
    downloadDirectoryStatus: appStatus.data.downloadDirectoryStatus,
    mpvStatus: appStatus.data.mediaTools.mpv.status,
    spotifyStatus: appStatus.data.providers.find((provider) => provider.kind === "spotify")?.runtimeStatus,
  } : undefined, [appStatus.data]);

  const sourceAction = useCallback(async (source: TrackInspectorSource, action: "play" | "play-next" | "queue" | "switch" | "reveal" | "open" | "download" | "lyrics") => {
    setActionError(null);
    try {
      if (action === "play") {
        await playback.playNow(trackId, source.sourceId);
      } else if (action === "play-next") {
        await playback.playNext(trackId, source.sourceId);
      } else if (action === "queue") {
        await playback.addToQueue(trackId, source.sourceId);
      } else if (action === "switch") {
        await playback.switchSource(trackId, source.sourceId);
      } else if (action === "reveal") {
        await revealLocalFile(source.sourceId);
      } else if (action === "open" && source.canonicalUrl) {
        await openProviderResult(source.provider, source.canonicalUrl);
      } else if (action === "download") {
        const modes = downloadModesForProvider(source.provider);
        const mode = modes.includes(downloadMode) ? downloadMode : "audio";
        const reason = downloadReadinessReason(source.provider, mode, {
          canonicalUrl: source.canonicalUrl,
          nativeRuntime: isTauriRuntime(),
          downloadsAvailable: source.capabilities.downloads,
          downloadReadiness,
        });
        if (isDownloadFolderReadinessReason(reason)) {
          const directory = await pickDownloadDirectory();
          if (!directory) {
            return;
          }
          await setSetting({ key: "downloadsDirectory", value: directory });
        }
        await queueSourceDownload(trackId, source.sourceId, mode);
      } else if (action === "lyrics") {
        navigate({ to: "/lyrics" });
      }
    } catch (error) {
      setActionError(errorMessage(error, "SpotDIY could not complete that inspector action."));
    }
  }, [downloadMode, downloadReadiness, navigate, playback, trackId]);

  const sections = useMemo(() => inspector ? inspectorSections(inspector, currentSourceId, sourceAction, downloadMode, setDownloadMode, downloadReadiness) : [], [currentSourceId, downloadMode, downloadReadiness, inspector, sourceAction]);

  if (query.isLoading) {
    return <InspectorPanel manageEscape={manageEscape} onClose={onClose} sections={[{ id: "loading", title: "OVERVIEW", content: <div className="inspector-pending" role="status"><SpotIcon name="spark" size={17} /> Reading track details…</div> }]} subtitle="Local track" title="Track Inspector" />;
  }
  if (query.isError || !inspector) {
    return <InspectorPanel manageEscape={manageEscape} onClose={onClose} sections={[{ id: "error", title: "OVERVIEW", content: <div className="inspector-error" role="alert"><SpotIcon name="alert" size={17} /><span>{errorMessage(query.error, "That track could not be inspected.")}</span></div> }]} subtitle="Local track" title="Track Inspector" />;
  }

  return (
    <>
      {actionError ? <div className="inspector-floating-message" role="alert">{actionError}</div> : null}
      <InspectorPanel manageEscape={manageEscape} onClose={onClose} sections={sections} subtitle={`${inspector.artists.join(" · ") || "Unknown artist"} · ${inspector.sources.length} sources`} title={inspector.title} />
    </>
  );
}

function searchResultDate(result: SearchResult): string | null {
  if (!result.publishedAt) {
    return null;
  }
  return result.publishedAt.value;
}

export function SearchResultInspector({ manageEscape = false, onClose, result }: SearchResultInspectorProps) {
  const appStatus = useAppStatus();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("audio");
  const downloadModes = downloadModesForProvider(result.provider);
  const downloadReadiness: DownloadReadiness | undefined = appStatus.data ? {
    ytDlpStatus: appStatus.data.mediaTools.ytDlp.status,
    ffmpegStatus: appStatus.data.mediaTools.ffmpeg.status,
    downloadDirectoryStatus: appStatus.data.downloadDirectoryStatus,
    mpvStatus: appStatus.data.mediaTools.mpv.status,
    spotifyStatus: appStatus.data.providers.find((provider) => provider.kind === "spotify")?.runtimeStatus,
  } : undefined;
  const downloadReason = downloadModes.length > 0
    ? downloadReadinessReason(result.provider, downloadMode, {
      canonicalUrl: result.canonicalUrl,
      nativeRuntime: isTauriRuntime(),
      downloadsAvailable: true,
      downloadReadiness,
    })
    : undefined;
  const nativeDownload = downloadModes.length > 0 && (downloadReason === undefined || isDownloadFolderReadinessReason(downloadReason));
  const searchActions = deriveSearchResultActions(result, {
    nativeRuntime: isTauriRuntime(),
    downloadsAvailable: true,
    downloadReadiness,
  });
  const playAction = searchActions.find((action) => action.id === "play");
  const provider = providerName(result.provider);
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (actionError) {
      setError(errorMessage(actionError, "That source action could not be completed."));
    } finally {
      setBusy(false);
    }
  };

  async function queueDownload(mode: DownloadMode) {
    const reason = downloadReadinessReason(result.provider, mode, {
      canonicalUrl: result.canonicalUrl,
      nativeRuntime: isTauriRuntime(),
      downloadsAvailable: true,
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

  const sections: InspectorSection[] = [
    {
      id: "search-overview",
      title: "OVERVIEW",
      content: (
        <div className="inspector-overview">
          <span className="inspector-locality-label">NOT IN LOCAL LIBRARY</span>
          <div className="inspector-title-block"><span className="eyebrow">EPHEMERAL SEARCH RESULT</span><h3>{result.title}</h3><p>{result.artists.join(" · ") || "Unknown artist"}{result.album ? ` · ${result.album}` : ""}</p></div>
          <dl className="inspector-fact-grid inspector-fact-grid-wide">
            <div><dt>Provider</dt><dd><ProviderBadge kind={result.provider} />{provider}</dd></div>
            <div><dt>Duration</dt><dd>{formatDuration(result.durationMs)}</dd></div>
            <div><dt>Published</dt><dd>{searchResultDate(result) ?? "Unavailable"}</dd></div>
            <div><dt>Engagement</dt><dd>{result.engagementCount === null ? "Unavailable" : `${result.engagementCount.toLocaleString()} ${result.engagementKind ?? ""}`}</dd></div>
            <div><dt>Explicit</dt><dd>{result.explicit === null ? "Unavailable" : result.explicit ? "Yes" : "No"}</dd></div>
          </dl>
        </div>
      ),
    },
    {
      id: "search-actions",
      title: "SOURCE ACTIONS",
      content: (
        <div className="inspector-search-actions">
          {error ? <div className="inspector-error" role="alert"><SpotIcon name="alert" size={15} /><span>{error}</span></div> : null}
          <p className="inspector-muted">Online results stay ephemeral until playback starts. Playing a result saves its validated provider source so the native player can load it and the queue can restore it.</p>
          <div className="inspector-source-actions">
            {result.provider !== "spotify" ? <button aria-label="Play online" className="button button-primary button-small icon-only-button" disabled={!playAction?.enabled || busy} onClick={() => void run(() => playSearchResult(result))} title={playAction?.enabled ? "Play this provider result" : playAction?.reason} type="button"><SpotIcon name="play" size={13} /> Play online</button> : null}
            <button aria-label={result.provider === "spotify" ? "Open on Spotify" : "Open source"} className="button button-primary button-small icon-only-button" disabled={!result.canonicalUrl || busy} onClick={() => { if (result.canonicalUrl) void run(() => openProviderResult(result.provider, result.canonicalUrl!)); }} title={result.canonicalUrl ? "Open the validated provider source" : "No validated provider URL is available."} type="button"><SpotIcon name="arrow" size={13} /> {result.provider === "spotify" ? "Open on Spotify" : "Open source"}</button>
            {downloadModes.length > 1 ? <select aria-label="Download mode" disabled={busy || !downloadModes.length} onChange={(event) => setDownloadMode(event.target.value as DownloadMode)} title={downloadReason ?? "Choose the managed download format"} value={downloadMode}>{downloadModes.map((mode) => { const modeReason = downloadReadinessReason(result.provider, mode, { canonicalUrl: result.canonicalUrl, nativeRuntime: isTauriRuntime(), downloadsAvailable: true, downloadReadiness }); return <option disabled={Boolean(modeReason && !isDownloadFolderReadinessReason(modeReason))} key={mode} value={mode}>{mode === "audio" ? "Audio" : "Video"}</option>; })}</select> : null}
            {downloadModes.length > 0 ? <button aria-label={downloadModes.length === 1 ? "Download audio" : "Download"} className="button button-quiet button-small icon-only-button" disabled={!nativeDownload || busy} onClick={() => void run(() => queueDownload(downloadModes.includes(downloadMode) ? downloadMode : "audio"))} title={nativeDownload ? (isDownloadFolderReadinessReason(downloadReason) ? "Choose a download folder, then queue this download" : "Queue a managed provider download") : downloadReason} type="button"><SpotIcon name="download" size={13} /> {downloadModes.length === 1 ? "Download audio" : "Download"}</button> : null}
          </div>
          <div className="inspector-disabled-explanation">{result.provider === "spotify" ? "Spotify audio is source-matched through spotdl; playback still opens Spotify." : playAction?.enabled ? "Online playback uses the validated provider URL." : playAction?.reason}</div>
        </div>
      ),
    },
  ];

  return <InspectorPanel manageEscape={manageEscape} onClose={onClose} sections={sections} subtitle={`${provider} · Search result`} title={result.title} />;
}
