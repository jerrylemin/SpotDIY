import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

const usePlaybackMock = vi.hoisted(() => vi.fn());
const useWindowsIntegrationMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/usePlayback", () => ({ usePlayback: usePlaybackMock }));
vi.mock("../src/hooks/useWindowsIntegration", () => ({ useWindowsIntegration: useWindowsIntegrationMock }));

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
  seekPlayback: vi.fn(),
  setVolume: vi.fn(),
  toggleMuted: vi.fn(),
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
    ],
    outputProfiles: [],
  },
  loading: false,
  error: null,
  closeOverlay: vi.fn(),
};

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{children}</QueryClientProvider>;
}

afterEach(() => {
  cleanup();
  usePlaybackMock.mockReset();
  useWindowsIntegrationMock.mockReset();
  playback.nextTrack.mockReset();
  playback.previousTrack.mockReset();
  playback.togglePlayPause.mockReset();
  playback.seekPlayback.mockReset();
  playback.setVolume.mockReset();
  playback.toggleMuted.mockReset();
  windows.closeOverlay.mockReset();
});

describe("Windows overlay surfaces", () => {
  it("renders the only mini surface with seek and volume controls", () => {
    usePlaybackMock.mockReturnValue(playback);
    useWindowsIntegrationMock.mockReturnValue(windows);

    render(<MiniOverlay />, { wrapper });
    const region = screen.getByRole("region", { name: "Mini overlay" });
    expect(region).toHaveTextContent("Night Drive");
    expect(region).not.toHaveTextContent("SpotDIY");
    expect(screen.getByRole("slider", { name: "Seek within current track" })).toBeVisible();
    expect(screen.getByRole("slider", { name: "Playback volume" })).toHaveValue("72");
  });
});
