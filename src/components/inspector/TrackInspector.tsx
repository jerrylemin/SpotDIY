import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";

import { useTrackInspector } from "../../hooks/useTrackInspector";
import { usePlayback } from "../../hooks/usePlayback";
import {
  IpcError,
  isTauriRuntime,
  openProviderResult,
  queueSourceDownload,
  revealLocalFile,
} from "../../services/ipc";
import type {
  DownloadMode,
  ProviderKind,
  SearchResult,
  TrackId,
  TrackInspector as TrackInspectorDto,
  TrackInspectorSource,
} from "../../types/domain";
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
    return source.availabilityDetail ?? "Online playback is not implemented.";
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
  onAction,
  source,
}: {
  current: boolean;
  onAction: (source: TrackInspectorSource, action: "play" | "play-next" | "queue" | "switch" | "reveal" | "open" | "download" | "lyrics") => void;
  source: TrackInspectorSource;
}) {
  const canPlay = source.available && source.capabilities.playback && (isTauriRuntime() || previewPlaybackEnabled());
  const canReveal = source.provider === "local" && source.available && isTauriRuntime();
  const canOpen = source.provider !== "local" && source.canonicalUrl !== null;
  const canDownload = (source.provider === "youtube" || source.provider === "soundcloud")
    && source.capabilities.downloads
    && source.canonicalUrl !== null
    && isTauriRuntime();
  const canLyrics = current && source.capabilities.lyrics;
  const playReason = !source.available
    ? sourceAvailability(source)
    : !source.capabilities.playback
      ? sourceAvailability(source)
      : !isTauriRuntime() && !previewPlaybackEnabled()
        ? "Playback controls require the native app."
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
          <button className="button button-primary button-small" disabled={!canPlay} onClick={() => onAction(source, "play")} title={canPlay ? "Play this source now" : playReason} type="button"><SpotIcon name="play" size={13} /> Play now</button>
        ) : (
          <button className="button button-primary button-small" disabled={!canPlay} onClick={() => onAction(source, "switch")} title={canPlay ? "Switch the current track to this source" : playReason} type="button">Switch source</button>
        )}
        <button className="button button-quiet button-small" disabled={!canPlay} onClick={() => onAction(source, "play-next")} title={canPlay ? "Play this source after the current track" : playReason} type="button">Play next</button>
        <button className="button button-quiet button-small" disabled={!canPlay} onClick={() => onAction(source, "queue")} title={canPlay ? "Add this source to the persistent queue" : playReason} type="button">Add to queue</button>
        <button className="button button-quiet button-small" disabled={!canReveal} onClick={() => onAction(source, "reveal")} title={canReveal ? "Reveal this managed local file" : source.provider === "local" ? "Local file reveal requires the native app and an available file." : "Only local sources have managed file locations."} type="button">Open location</button>
        <button className="button button-quiet button-small" disabled={!canOpen} onClick={() => onAction(source, "open")} title={canOpen ? "Open the validated provider source" : "No validated provider URL is available."} type="button">Open source</button>
        <button className="button button-quiet button-small" disabled={!canDownload} onClick={() => onAction(source, "download")} title={canDownload ? "Queue this managed provider download" : "Downloads require a supported YouTube or SoundCloud source in the native app."} type="button"><SpotIcon name="download" size={13} /> Download</button>
        <button className="button button-quiet button-small" disabled={!canLyrics} onClick={() => onAction(source, "lyrics")} title={canLyrics ? "Open lyrics for the current source" : current ? "This source does not advertise lyrics." : "Play this source first to open synchronized lyrics."} type="button"><SpotIcon name="lyrics" size={13} /> Lyrics</button>
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

function inspectorSections(inspector: TrackInspectorDto, currentSourceId: string | null, sourceAction: (source: TrackInspectorSource, action: "play" | "play-next" | "queue" | "switch" | "reveal" | "open" | "download" | "lyrics") => void, downloadMode: DownloadMode, onDownloadModeChange: (mode: DownloadMode) => void): InspectorSection[] {
  return [
    { id: "overview", title: "OVERVIEW", content: <Overview inspector={inspector} /> },
    { id: "sources", title: "SOURCES", content: <div className="inspector-source-list"><label className="inspector-download-control">Provider download format<select aria-label="Download mode" onChange={(event) => onDownloadModeChange(event.target.value as DownloadMode)} value={downloadMode}><option value="audio">Audio</option><option value="video">Video</option></select></label>{inspector.sources.map((source) => <SourceCard current={source.sourceId === currentSourceId} key={source.sourceId} onAction={sourceAction} source={source} />)}</div> },
    { id: "quality", title: "QUALITY", content: <QualityState currentSourceId={currentSourceId} inspector={inspector} /> },
    { id: "collection", title: "COLLECTION", content: <CollectionState inspector={inspector} /> },
    { id: "capabilities", title: "CAPABILITIES", content: <div className="inspector-capability-source-list">{inspector.sources.map((source) => <div className="inspector-capability-source" key={source.sourceId}><div><ProviderBadge kind={source.provider} /><strong>{providerName(source.provider)}</strong></div><CapabilityList source={source} /></div>)}</div> },
  ];
}

export function TrackInspector({ manageEscape = false, onClose, trackId }: TrackInspectorProps) {
  const navigate = useNavigate();
  const playback = usePlayback();
  const query = useTrackInspector(trackId);
  const [actionError, setActionError] = useState<string | null>(null);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("audio");
  const inspector = query.data;
  const currentTrack = playback.snapshot.currentTrackId === trackId;
  const currentSourceId = currentTrack ? playback.snapshot.currentSourceId : null;

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
        await queueSourceDownload(trackId, source.sourceId, downloadMode);
      } else if (action === "lyrics") {
        navigate({ to: "/lyrics" });
      }
    } catch (error) {
      setActionError(errorMessage(error, "SpotDIY could not complete that inspector action."));
    }
  }, [downloadMode, navigate, playback, trackId]);

  const sections = useMemo(() => inspector ? inspectorSections(inspector, currentSourceId, sourceAction, downloadMode, setDownloadMode) : [], [currentSourceId, downloadMode, inspector, sourceAction]);

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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("audio");
  const canDownload = (result.provider === "youtube" || result.provider === "soundcloud") && result.canonicalUrl !== null;
  const nativeDownload = canDownload && isTauriRuntime();
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
          <p className="inspector-muted">Online results are ephemeral. Opening this panel does not persist a Track, create a source, or change the local library.</p>
          <div className="inspector-source-actions">
            <button className="button button-primary button-small" disabled={!result.canonicalUrl || busy} onClick={() => { if (result.canonicalUrl) void run(() => openProviderResult(result.provider, result.canonicalUrl!)); }} title={result.canonicalUrl ? "Open the validated provider source" : "No validated provider URL is available."} type="button">{result.provider === "spotify" ? "Open on Spotify" : "Open source"}</button>
            <select aria-label="Download mode" disabled={!nativeDownload || busy} onChange={(event) => setDownloadMode(event.target.value as DownloadMode)} title={nativeDownload ? "Choose the managed download format" : "Downloads require the native SpotDIY app"} value={downloadMode}><option value="audio">Audio</option><option value="video">Video</option></select>
            <button className="button button-quiet button-small" disabled={!nativeDownload || busy} onClick={() => void run(() => import("../../services/ipc").then(({ queueSearchResultDownload }) => queueSearchResultDownload(result, downloadMode)))} title={nativeDownload ? "Queue a managed provider download" : canDownload ? "Downloads require the native SpotDIY desktop runtime." : "This provider does not advertise downloads."} type="button"><SpotIcon name="download" size={13} /> Download</button>
          </div>
          <div className="inspector-disabled-explanation">Online playback is not implemented for search results. Spotify remains metadata-only, and Spotify downloads are not supported.</div>
        </div>
      ),
    },
  ];

  return <InspectorPanel manageEscape={manageEscape} onClose={onClose} sections={sections} subtitle={`${provider} · Search result`} title={result.title} />;
}
