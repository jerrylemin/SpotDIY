import { useEffect, useRef } from "react";

import { activeCueIndex } from "../../hooks/useLyrics";
import type { LyricsDocument } from "../../types/domain";

interface LyricsPanelProps {
  document: LyricsDocument;
  positionMs: number;
  onSeek: (positionMs: number) => void;
}

function formatCueTime(positionMs: number): string {
  const totalSeconds = Math.floor(positionMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

export function LyricsPanel({ document, positionMs, onSeek }: LyricsPanelProps) {
  const activeIndex = activeCueIndex(document.cues, positionMs);
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
  }, [activeIndex]);

  if (document.syncKind === "instrumental") {
    return <div className="lyrics-instrumental">Instrumental track · no lyric text is available.</div>;
  }

  if (document.syncKind === "plain") {
    return <div className="lyrics-plain-text">{document.plainText ?? "No lyric text is available."}</div>;
  }

  return (
    <div aria-label="Timed lyrics" className="lyrics-cue-list">
      {document.cues.map((cue, index) => (
        <button
          aria-current={index === activeIndex ? "true" : undefined}
          className={`lyrics-cue${index === activeIndex ? " lyrics-cue-active" : ""}`}
          key={`${cue.startMs}-${index}`}
          onClick={() => onSeek(cue.startMs)}
          ref={(element) => { cueRefs.current[index] = element; }}
          type="button"
        >
          <span className="lyrics-cue-time">{formatCueTime(cue.startMs)}</span>
          {cue.lines.map((line, lineIndex) => <span className="lyrics-cue-line" key={`${line}-${lineIndex}`}>{line || " "}</span>)}
        </button>
      ))}
    </div>
  );
}
