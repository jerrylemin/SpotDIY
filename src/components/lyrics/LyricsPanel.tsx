import { useEffect, useRef } from "react";

import { activeCueIndex, activeWordIndex, cueProgress } from "../../hooks/useLyrics";
import type { LyricsDocument } from "../../types/domain";

interface LyricsPanelProps {
  document: LyricsDocument;
  durationMs?: number | null;
  lyricOffsetMs?: number;
  positionMs: number;
  onSeek: (positionMs: number) => void;
}

function formatCueTime(positionMs: number): string {
  const totalSeconds = Math.floor(positionMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

export function LyricsPanel({ document, durationMs = null, lyricOffsetMs = 0, positionMs, onSeek }: LyricsPanelProps) {
  const effectivePositionMs = Math.max(0, positionMs + lyricOffsetMs);
  const activeIndex = activeCueIndex(document.cues, effectivePositionMs);
  const activeWord = activeIndex >= 0
    ? activeWordIndex(document.cues[activeIndex].words ?? [], effectivePositionMs)
    : -1;
  const cueRefs = useRef<Array<HTMLButtonElement | null>>([]);

  useEffect(() => {
    if (activeIndex < 0) {
      return;
    }
    const activeCue = cueRefs.current[activeIndex];
    if (!activeCue || typeof window === "undefined") {
      return;
    }
    if (typeof activeCue.scrollIntoView !== "function") {
      return;
    }
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    activeCue.scrollIntoView({ behavior: reducedMotion ? "auto" : "smooth", block: "center" });
  }, [activeIndex, document.source, document.trackId]);

  if (document.syncKind === "instrumental") {
    return <div className="lyrics-instrumental">Instrumental track · no lyric text is available.</div>;
  }

  if (document.syncKind === "plain") {
    return <div className="lyrics-plain-text">{document.plainText ?? "No lyric text is available."}</div>;
  }

  return (
    <div aria-label="Timed lyrics" className="lyrics-cue-list">
      {document.cues.map((cue, index) => {
        const active = index === activeIndex;
        const words = cue.words ?? [];
        const progress = active ? cueProgress(document.cues, index, effectivePositionMs, durationMs) : 0;
        return (
          <button
            aria-current={active ? "true" : undefined}
            className={`lyrics-cue${active ? " lyrics-cue-active" : ""}`}
            key={`${cue.startMs}-${index}`}
            onClick={() => onSeek(Math.max(0, cue.startMs - lyricOffsetMs))}
            ref={(element) => { cueRefs.current[index] = element; }}
            type="button"
          >
            <span className="lyrics-cue-time">{formatCueTime(cue.startMs)}</span>
            {words.length > 0 ? (
              <span aria-label={cue.lines.join(" ")} className="lyrics-cue-word-line">
                {words.map((word, wordIndex) => (
                  <span className={`lyrics-word${active && wordIndex <= activeWord ? " lyrics-word-active" : ""}`} key={`${word.startMs}-${wordIndex}`}>
                    {word.text}{wordIndex < words.length - 1 ? " " : ""}
                  </span>
                ))}
              </span>
            ) : cue.lines.map((line, lineIndex) => <span className="lyrics-cue-line" key={`${line}-${lineIndex}`}>{line || " "}</span>)}
            <span aria-hidden="true" className="lyrics-cue-progress"><span style={{ width: `${progress * 100}%` }} /></span>
          </button>
        );
      })}
    </div>
  );
}
