import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { ProviderBadge } from "../common/ProviderBadge";
import { AudioDeviceMenu } from "../player/AudioDeviceMenu";
import { PlaybackControls } from "../player/PlaybackControls";
import { ProgressControl } from "../player/ProgressControl";
import { VolumeControl } from "../player/VolumeControl";
import { SpotIcon } from "../icons/SpotIcon";
import { isTauriRuntime } from "../../services/ipc";
import { usePlayback } from "../../hooks/usePlayback";
import { useUiStore } from "../../stores/ui-store";

function phaseCaption(phase: ReturnType<typeof usePlayback>["snapshot"]["phase"]): string {
  switch (phase) {
    case "loading":
      return "Loading track";
    case "playing":
      return "Now playing";
    case "paused":
      return "Paused";
    case "seeking":
      return "Seeking";
    case "ended":
      return "Playback ended";
    case "recovering":
      return "Recovering playback";
    case "failed":
      return "Playback unavailable";
    case "shuttingDown":
      return "Shutting down";
    default:
      return "Nothing queued";
  }
}

export function PlayerBar() {
  const playback = usePlayback();
  const queueDrawerOpen = useUiStore((state) => state.queueDrawerOpen);
  const setQueueDrawerOpen = useUiStore((state) => state.setQueueDrawerOpen);
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

  useEffect(() => {
    setArtworkFailed(false);
  }, [artworkSource]);

  return (
    <footer className="player-bar" aria-label="Now playing">
      <div className="player-track-panel">
        <div className="player-art-placeholder">
          {artworkSource && !artworkFailed ? (
            <img alt="" onError={() => setArtworkFailed(true)} src={artworkSource} />
          ) : (
            <SpotIcon name={hasCurrentTrack ? "library" : "play"} size={20} />
          )}
        </div>
        <div className="player-track-copy">
          <div className="player-track-heading">
            <span className={`player-phase-chip player-phase-${playback.snapshot.phase}`}>{phaseCaption(playback.snapshot.phase)}</span>
            {currentSource ? <ProviderBadge kind={currentSource.provider} /> : null}
          </div>
          <strong title={playback.snapshot.title ?? undefined}>
            {playback.snapshot.title ?? (browserPreviewIdle ? "Native playback only" : "Nothing queued")}
          </strong>
          <span>
            {hasCurrentTrack
              ? playback.snapshot.artists.length > 0
                ? playback.snapshot.artists.join(" · ")
                : "Unknown artist"
              : browserPreviewIdle
                ? "Open the desktop app to control local playback."
                : "Choose a local track to start listening."}
          </span>
          {playback.snapshot.album ? <small>{playback.snapshot.album}</small> : null}
          {failureMessage ? (
            <div className="player-inline-alert" role={playback.snapshot.phase === "failed" ? "alert" : "status"}>
              <SpotIcon name="alert" size={14} />
              {toolMissing ? (
                <span><strong>Player engine unavailable</strong> Install mpv to play local music</span>
              ) : <span>{failureMessage}</span>}
              {canRetry ? (
                <button className="player-meta-action" onClick={() => { void playback.retryPlaybackBackend(); }} type="button">
                  Retry Player Engine
                </button>
              ) : null}
            </div>
          ) : playback.snapshot.recovering || playback.snapshot.backendHealth.detail ? (
            <div className="player-inline-alert player-inline-status" role="status">
              <SpotIcon name="refresh" size={14} />
              <span>{playback.snapshot.backendHealth.detail ?? "SpotDIY is recovering playback."}</span>
              {canRetry ? (
                <button className="player-meta-action" onClick={() => { void playback.retryPlaybackBackend(); }} type="button">
                  Retry Player Engine
                </button>
              ) : null}
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
          disabled={!hasCurrentTrack || playback.snapshot.phase === "failed" || playback.snapshot.phase === "recovering"}
          durationMs={playback.snapshot.durationMs}
          onSeek={(positionMs) => { void playback.seekPlayback(positionMs); }}
          pending={playback.pending}
          positionMs={playback.snapshot.positionMs}
        />
      </div>

      <div className="player-right">
        <span className="player-source-label" title={currentSource?.availabilityDetail ?? undefined}>SOURCE <strong>{sourceLabel}</strong></span>
        <button
          aria-expanded={queueDrawerOpen}
          aria-label="Open queue"
          className={`button button-quiet button-small player-queue-toggle${queueDrawerOpen ? " player-queue-toggle-active" : ""}`}
          onClick={() => setQueueDrawerOpen(!queueDrawerOpen)}
          type="button"
        >
          <SpotIcon name="queue" size={14} />
          Queue {playback.snapshot.queueLength > 0 ? `· ${playback.snapshot.queueLength}` : ""}
        </button>
        <AudioDeviceMenu
          devices={playback.audioDevices}
          disabled={playback.snapshot.phase === "failed" || playback.snapshot.phase === "recovering"}
          loading={playback.audioDevicesLoading}
          onOpen={() => { void playback.warmAudioDevices(); }}
          onSelect={(name) => { void playback.setAudioDevice(name); }}
          selectedDeviceName={playback.snapshot.selectedAudioDevice}
        />
        <VolumeControl
          disabled={playback.snapshot.phase === "failed" || playback.snapshot.phase === "recovering"}
          muted={playback.snapshot.muted}
          onSetVolume={(volumePercent) => { void playback.setVolume(volumePercent); }}
          onToggleMuted={() => { void playback.toggleMuted(); }}
          pending={playback.pending}
          volumePercent={playback.snapshot.volumePercent}
        />
      </div>
    </footer>
  );
}
