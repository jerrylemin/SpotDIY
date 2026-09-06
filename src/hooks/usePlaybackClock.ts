import { useEffect, useRef, useState } from "react";

import type { PlaybackPhase, PlaybackSnapshot } from "../types/domain";

export type PlaybackClockSnapshot = Pick<
  PlaybackSnapshot,
  "revision" | "phase" | "currentTrackId" | "currentSourceId" | "positionMs" | "durationMs"
>;

function monotonicNow(): number {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}

export function interpolatePlaybackPosition(
  positionMs: number,
  elapsedMs: number,
  phase: PlaybackPhase,
  durationMs: number | null,
): number {
  const nextPositionMs = Math.max(0, positionMs) + (phase === "playing" ? Math.max(0, elapsedMs) : 0);
  return Math.max(0, durationMs === null ? nextPositionMs : Math.min(durationMs, nextPositionMs));
}

export function usePlaybackClock(snapshot: PlaybackClockSnapshot): number {
  const [positionMs, setPositionMs] = useState(snapshot.positionMs);
  const durationRef = useRef(snapshot.durationMs);
  const anchorRef = useRef({
    positionMs: snapshot.positionMs,
    receivedAt: monotonicNow(),
    phase: snapshot.phase,
  });

  useEffect(() => {
    durationRef.current = snapshot.durationMs;
  }, [snapshot.durationMs]);

  useEffect(() => {
    anchorRef.current = {
      positionMs: snapshot.positionMs,
      receivedAt: monotonicNow(),
      phase: snapshot.phase,
    };
    setPositionMs(snapshot.positionMs);
  }, [snapshot.currentSourceId, snapshot.currentTrackId, snapshot.phase, snapshot.positionMs, snapshot.revision]);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.requestAnimationFrame !== "function") {
      return;
    }

    let frameId = 0;
    const tick = () => {
      const anchor = anchorRef.current;
      const nextPositionMs = interpolatePlaybackPosition(
        anchor.positionMs,
        monotonicNow() - anchor.receivedAt,
        anchor.phase,
        durationRef.current,
      );
      setPositionMs((previous) => Math.abs(previous - nextPositionMs) < 0.5 ? previous : nextPositionMs);
      frameId = window.requestAnimationFrame(tick);
    };

    frameId = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(frameId);
  }, []);

  return positionMs;
}
