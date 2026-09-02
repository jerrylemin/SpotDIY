import { activeCueIndex, useLyrics } from "../../hooks/useLyrics";
import { usePlayback } from "../../hooks/usePlayback";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import { SpotIcon } from "../icons/SpotIcon";
import { OverlayFrame } from "./OverlayFrame";

export function LyricsOverlay() {
  const playback = usePlayback();
  const windows = useWindowsIntegration();
  const snapshot = playback.snapshot;
  const lyrics = useLyrics(snapshot.currentTrackId, snapshot.currentSourceId);
  const document = lyrics.data;
  const activeIndex = document ? activeCueIndex(document.cues, snapshot.positionMs) : -1;
  const activeCue = activeIndex >= 0 ? document?.cues[activeIndex] : undefined;
  const previousCue = activeIndex > 0 ? document?.cues[activeIndex - 1] : undefined;
  const nextCue = document && activeIndex + 1 < document.cues.length ? document.cues[activeIndex + 1] : undefined;
  const canTransport = snapshot.currentTrackId !== null || snapshot.queueLength > 0;

  return (
    <OverlayFrame kind="lyrics" title="Lyrics" onClose={() => { void windows.closeOverlay("lyrics"); }}>
      <div className="spot-overlay-lyrics-heading">
        <strong>{snapshot.title ?? "Nothing queued"}</strong>
        <span>{snapshot.artists.join(" · ") || "Choose a track"}</span>
      </div>
      <div className="spot-overlay-lyrics-body">
        {lyrics.isLoading ? <span className="spot-overlay-muted">Reading local lyrics…</span> : document?.syncKind === "instrumental" ? <span className="spot-overlay-muted">Instrumental track · no lyric text is available.</span> : document?.syncKind === "plain" ? <span>{document.plainText ?? "No lyric text is available."}</span> : activeCue ? <>
          {previousCue ? <span className="spot-overlay-lyrics-context">{previousCue.lines.join(" ")}</span> : null}
          <strong>{activeCue.lines.join(" ")}</strong>
          {nextCue ? <span className="spot-overlay-lyrics-context">{nextCue.lines.join(" ")}</span> : null}
        </> : <span className="spot-overlay-muted">No local lyrics are available.</span>}
      </div>
      <div className="spot-overlay-lyrics-controls">
        <button aria-label="Previous track" className="spot-overlay-control" disabled={!canTransport || playback.pending} onClick={() => { void playback.previousTrack(); }} type="button"><SpotIcon name="previous" size={15} /></button>
        <button aria-label={snapshot.phase === "playing" ? "Pause" : "Play"} className="spot-overlay-play" disabled={!canTransport || playback.pending} onClick={() => { void playback.togglePlayPause(); }} type="button"><SpotIcon name={snapshot.phase === "playing" ? "pause" : "play"} size={14} /></button>
        <button aria-label="Next track" className="spot-overlay-control" disabled={!canTransport || playback.pending} onClick={() => { void playback.nextTrack(); }} type="button"><SpotIcon name="next" size={15} /></button>
      </div>
    </OverlayFrame>
  );
}
