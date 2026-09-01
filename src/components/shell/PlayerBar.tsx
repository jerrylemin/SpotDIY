import { convertFileSrc } from "@tauri-apps/api/core";
import { Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { isTauriRuntime } from "../../services/ipc";
import { usePlayback } from "../../hooks/usePlayback";
import { useBookmarks } from "../../hooks/useLyrics";
import { useUiStore } from "../../stores/ui-store";
import { AudioDeviceMenu } from "../player/AudioDeviceMenu";
import { MiniPlayer } from "../player/MiniPlayer";
import { NowPlayingPanel } from "../player/NowPlayingPanel";
import { PlaybackControls } from "../player/PlaybackControls";
import { SourceSwitcher } from "../player/SourceSwitcher";
import { ProgressControl } from "../player/ProgressControl";
import { VolumeControl } from "../player/VolumeControl";
import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";

function phaseCaption(phase: ReturnType<typeof usePlayback>["snapshot"]["phase"]): string {
  switch (phase) {
    case "loading": return "Loading track";
    case "playing": return "Now playing";
    case "paused": return "Paused";
    case "seeking": return "Seeking";
    case "ended": return "Playback ended";
    case "recovering": return "Recovering playback";
    case "failed": return "Playback unavailable";
    case "shuttingDown": return "Shutting down";
    default: return "Nothing queued";
  }
}

function StandardPlayerBar() {
  const playback = usePlayback();
  const bookmarks = useBookmarks(playback.snapshot.currentTrackId);
  const queueDrawerOpen = useUiStore((state) => state.queueDrawerOpen);
  const setQueueDrawerOpen = useUiStore((state) => state.setQueueDrawerOpen);
  const setPlayerMode = useUiStore((state) => state.setPlayerMode);
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const [artworkFailed, setArtworkFailed] = useState(false);
  const currentSource = playback.snapshot.sources.find((source) => source.sourceId === playback.snapshot.currentSourceId) ?? null;
  const sourceLabel = currentSource?.label ?? "—";
  const browserPreviewIdle = !isTauriRuntime() && !import.meta.env.VITE_SPOTDIY_E2E;
  const artworkSource = playback.snapshot.artworkPath
    ? isTauriRuntime()
      ? convertFileSrc(playback.snapshot.artworkPath, "asset")
      : playback.snapshot.artworkPath
    : null;
  const hasCurrentTrack = playback.snapshot.currentTrackId !== null;
  const failureMessage = playback.snapshot.error?.summary ?? playback.bridgeError;
  const canRetry = Boolean(playback.snapshot.error?.retryable || playback.bridgeError || playback.snapshot.recovering);
  const toolMissing = playback.snapshot.error?.code === "toolMissing";

  useEffect(() => setArtworkFailed(false), [artworkSource]);

  return (
    <footer aria-label="Now playing" className="player-bar">
      <div className="player-track-panel">
        <button
          aria-label={hasCurrentTrack ? `Inspect ${playback.snapshot.title ?? "current track"}` : "Inspect current track"}
          className="player-art-placeholder"
          disabled={!hasCurrentTrack}
          onClick={() => { if (playback.snapshot.currentTrackId) openTrackInspector(playback.snapshot.currentTrackId); }}
          type="button"
        >
          {artworkSource && !artworkFailed ? <img alt="" onError={() => setArtworkFailed(true)} src={artworkSource} /> : <SpotIcon name={hasCurrentTrack ? "library" : "play"} size={20} />}
        </button>
        <div className="player-track-copy">
          <div className="player-track-heading">
            <span className={`player-phase-chip player-phase-${playback.snapshot.phase}`}>{phaseCaption(playback.snapshot.phase)}</span>
            {currentSource ? <ProviderBadge kind={currentSource.provider} /> : null}
          </div>
          <strong title={playback.snapshot.title ?? undefined}>{playback.snapshot.title ?? (browserPreviewIdle ? "Native playback only" : "Nothing queued")}</strong>
          <span>{hasCurrentTrack ? playback.snapshot.artists.join(" · ") || "Unknown artist" : browserPreviewIdle ? "Open the desktop app to control local playback." : "Choose a local track to start listening."}</span>
          {playback.snapshot.album ? <small>{playback.snapshot.album}</small> : null}
          {failureMessage ? (
            <div className="player-inline-alert" role={playback.snapshot.phase === "failed" ? "alert" : "status"}>
              <SpotIcon name="alert" size={14} />
              {toolMissing ? <span><strong>Player engine unavailable</strong> Install mpv to play local music</span> : <span>{failureMessage}</span>}
              {canRetry ? <button className="player-meta-action" onClick={() => { void playback.retryPlaybackBackend(); }} type="button">Retry Player Engine</button> : null}
            </div>
          ) : playback.snapshot.recovering || playback.snapshot.backendHealth.detail ? (
            <div className="player-inline-alert player-inline-status" role="status">
              <SpotIcon name="refresh" size={14} />
              <span>{playback.snapshot.backendHealth.detail ?? "SpotDIY is recovering playback."}</span>
              {canRetry ? <button className="player-meta-action" onClick={() => { void playback.retryPlaybackBackend(); }} type="button">Retry Player Engine</button> : null}
            </div>
          ) : null}
        </div>
      </div>

      <div className="player-main">
        <PlaybackControls
          disabled={playback.initializing}
          onClearQueue={() => { void playback.clearQueue(); }}
          onCycleRepeat={() => { void playback.cycleRepeatMode(); }}
          onNext={() => { void playback.nextTrack(); }}
          onPrevious={() => { void playback.previousTrack(); }}
          onTogglePlayPause={() => { void playback.togglePlayPause(); }}
          onToggleShuffle={() => { void playback.toggleShuffle(); }}
          pending={playback.pending}
          snapshot={playback.snapshot}
        />
        <ProgressControl
          abLoop={playback.snapshot.abLoop}
          bookmarkPositions={bookmarks.data?.map((bookmark) => bookmark.positionMs)}
          disabled={!hasCurrentTrack || playback.snapshot.phase === "failed" || playback.snapshot.phase === "recovering"}
          durationMs={playback.snapshot.durationMs}
          onSeek={(positionMs) => { void playback.seekPlayback(positionMs); }}
          pending={playback.pending}
          positionMs={playback.snapshot.positionMs}
        />
      </div>

      <div className="player-right">
        <div className="player-source-row">
          <span className="player-source-label" title={currentSource?.availabilityDetail ?? undefined}>SOURCE <strong>{sourceLabel}</strong></span>
          <SourceSwitcher
            currentSourceId={playback.snapshot.currentSourceId}
            disabled={!hasCurrentTrack || playback.pending}
            onSwitch={(sourceId) => { if (playback.snapshot.currentTrackId) void playback.switchSource(playback.snapshot.currentTrackId, sourceId); }}
            sources={playback.snapshot.sources}
          />
        </div>
        <div className="player-shell-actions">
          <button aria-label="Open expanded now playing" className="icon-button" onClick={() => setPlayerMode("expanded")} title="Open expanded now playing" type="button"><SpotIcon name="expand" size={16} /></button>
          <button aria-label="Open mini player" className="icon-button" onClick={() => setPlayerMode("mini")} title="Open mini player" type="button"><SpotIcon name="collapse" size={16} /></button>
          <button aria-expanded={queueDrawerOpen} aria-label="Open queue" className={`button button-quiet button-small player-queue-toggle${queueDrawerOpen ? " player-queue-toggle-active" : ""}`} onClick={() => setQueueDrawerOpen(!queueDrawerOpen)} type="button"><SpotIcon name="queue" size={14} /> Queue {playback.snapshot.queueLength > 0 ? `· ${playback.snapshot.queueLength}` : ""}</button>
          <Link aria-label="Open lyrics" className="button button-quiet button-small player-lyrics-link" to="/lyrics"><SpotIcon name="lyrics" size={14} /> Lyrics</Link>
        </div>
        <AudioDeviceMenu devices={playback.audioDevices} disabled={playback.snapshot.phase === "failed" || playback.snapshot.phase === "recovering"} loading={playback.audioDevicesLoading} onOpen={() => { void playback.warmAudioDevices(); }} onSelect={(name) => { void playback.setAudioDevice(name); }} selectedDeviceName={playback.snapshot.selectedAudioDevice} />
        <VolumeControl disabled={playback.snapshot.phase === "failed" || playback.snapshot.phase === "recovering"} muted={playback.snapshot.muted} onSetVolume={(volumePercent) => { void playback.setVolume(volumePercent); }} onToggleMuted={() => { void playback.toggleMuted(); }} pending={playback.pending} volumePercent={playback.snapshot.volumePercent} />
      </div>
    </footer>
  );
}

export function PlayerBar() {
  const mode = useUiStore((state) => state.playerMode);
  if (mode === "mini") return <MiniPlayer />;
  if (mode === "expanded") return <NowPlayingPanel />;
  return <StandardPlayerBar />;
}
