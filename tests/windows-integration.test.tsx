import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

const usePlaybackMock = vi.hoisted(() => vi.fn());
const useWindowsIntegrationMock = vi.hoisted(() => vi.fn());
const useLyricsMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/usePlayback", () => ({ usePlayback: usePlaybackMock }));
vi.mock("../src/hooks/useWindowsIntegration", () => ({ useWindowsIntegration: useWindowsIntegrationMock }));
vi.mock("../src/hooks/useLyrics", () => ({
  activeCueIndex: (cues: Array<{ startMs: number }>, positionMs: number) => cues.reduce((active, cue, index) => cue.startMs <= positionMs ? index : active, -1),
  useLyrics: useLyricsMock,
}));

import { EdgeOverlay } from "../src/components/overlay/EdgeOverlay";
import { GamingOverlay } from "../src/components/overlay/GamingOverlay";
import { LyricsOverlay } from "../src/components/overlay/LyricsOverlay";
import { MiniOverlay } from "../src/components/overlay/MiniOverlay";

const playback = {
  snapshot: {
    revision: 4,
    phase: "playing" as const,
    currentQueueEntryId: "queue-1",
    currentTrackId: "track-1",
    currentSourceId: "source-1",
    title: "Night Drive",
    artists: ["Luna Max"],
    album: "Afterglow",
    artworkPath: null,
    sources: [],
    positionMs: 1_000,
    durationMs: 10_000,
    volumePercent: 72,
    muted: false,
    repeatMode: "off" as const,
    shuffleEnabled: false,
    queueLength: 1,
    queueIndex: 0,
    selectedAudioDevice: "auto",
    backendHealth: { ready: true, connected: true, detail: null, recoveryAction: null },
    recovering: false,
    error: null,
    abLoop: { aMs: null, bMs: null, active: false },
  },
  pending: false,
  nextTrack: vi.fn(),
  previousTrack: vi.fn(),
  togglePlayPause: vi.fn(),
  setVolume: vi.fn(),
};

const windows = {
  snapshot: {
    revision: 2,
    platformSupported: true,
    trayStatus: "ready" as const,
    trayDetail: null,
    smtcStatus: "ready" as const,
    smtcDetail: null,
    globalShortcutsEnabled: false,
    shortcutStatuses: [],
    overlays: [
      { kind: "mini" as const, status: "open" as const, detail: null },
      { kind: "edge" as const, status: "open" as const, detail: null },
      { kind: "lyrics" as const, status: "open" as const, detail: null },
      { kind: "gaming" as const, status: "open" as const, detail: null },
    ],
    gamingClickThrough: false,
    outputProfiles: [],
  },
  loading: false,
  error: null,
  closeOverlay: vi.fn(),
  setGamingClickThrough: vi.fn(),
};

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{children}</QueryClientProvider>;
}

afterEach(() => {
  cleanup();
  usePlaybackMock.mockReset();
  useWindowsIntegrationMock.mockReset();
  useLyricsMock.mockReset();
  playback.nextTrack.mockReset();
  playback.previousTrack.mockReset();
  playback.togglePlayPause.mockReset();
  playback.setVolume.mockReset();
  windows.closeOverlay.mockReset();
  windows.setGamingClickThrough.mockReset();
});

describe("Windows overlay surfaces", () => {
  it("renders the Mini, Edge, Lyrics, and Gaming surfaces from shared playback state", () => {
    usePlaybackMock.mockReturnValue(playback);
    useWindowsIntegrationMock.mockReturnValue(windows);
    useLyricsMock.mockReturnValue({
      data: {
        syncKind: "timed",
        cues: [{ startMs: 0, lines: ["First line"] }, { startMs: 900, lines: ["Active line"] }, { startMs: 2_000, lines: ["Next line"] }],
        plainText: null,
      },
      isLoading: false,
    });

    const mini = render(<MiniOverlay />, { wrapper });
    expect(screen.getByRole("region", { name: "Mini overlay" })).toHaveTextContent("Night Drive");
    mini.unmount();

    const edge = render(<EdgeOverlay />, { wrapper });
    expect(screen.getByRole("region", { name: "Edge overlay" })).toHaveTextContent("Luna Max");
    edge.unmount();

    const lyrics = render(<LyricsOverlay />, { wrapper });
    expect(screen.getByRole("region", { name: "Lyrics overlay" })).toHaveTextContent("Active line");
    lyrics.unmount();

    render(<GamingOverlay />, { wrapper });
    expect(screen.getByRole("region", { name: "Gaming overlay" })).toHaveTextContent("Interactive");
  });

  it("keeps Gaming click-through explicit and routed through the Windows service", async () => {
    usePlaybackMock.mockReturnValue(playback);
    useWindowsIntegrationMock.mockReturnValue(windows);
    useLyricsMock.mockReturnValue({ data: null, isLoading: false });
    render(<GamingOverlay />, { wrapper });

    expect(screen.getByText(/windowed or borderless games/)).toBeVisible();
    await screen.getByRole("button", { name: "Enable click-through" }).click();
    expect(windows.setGamingClickThrough).toHaveBeenCalledWith(true);
  });
});
