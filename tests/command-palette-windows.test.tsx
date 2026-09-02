import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

const usePlaybackMock = vi.hoisted(() => vi.fn());
const useWindowsIntegrationMock = vi.hoisted(() => vi.fn());
const navigateMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/usePlayback", () => ({ usePlayback: usePlaybackMock }));
vi.mock("../src/hooks/useWindowsIntegration", () => ({ useWindowsIntegration: useWindowsIntegrationMock }));
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => navigateMock }));

import { CommandPalette } from "../src/components/shell/CommandPalette";
import { useUiStore } from "../src/stores/ui-store";

const toggleOverlayMock = vi.fn();

afterEach(() => {
  cleanup();
  useUiStore.getState().setCommandPaletteOpen(false);
  usePlaybackMock.mockReset();
  useWindowsIntegrationMock.mockReset();
  toggleOverlayMock.mockReset();
  navigateMock.mockReset();
});

describe("Windows command-palette actions", () => {
  it("exposes native overlay toggles and Show SpotDIY only when integration is available", () => {
    usePlaybackMock.mockReturnValue({
      snapshot: { phase: "idle", currentTrackId: null, queueLength: 0 },
      pending: false,
      togglePlayPause: vi.fn(),
      nextTrack: vi.fn(),
      previousTrack: vi.fn(),
      clearQueue: vi.fn(),
    });
    useWindowsIntegrationMock.mockReturnValue({
      snapshot: { platformSupported: true },
      toggleOverlay: toggleOverlayMock,
      showMain: vi.fn(),
    });
    useUiStore.getState().setCommandPaletteOpen(true);
    render(<CommandPalette />);

    expect(screen.getByText("Toggle Mini Overlay")).toBeVisible();
    expect(screen.getByText("Toggle Edge Overlay")).toBeVisible();
    expect(screen.getByText("Toggle Lyrics Overlay")).toBeVisible();
    expect(screen.getByText("Toggle Gaming Overlay")).toBeVisible();
    expect(screen.getByText("Show SpotDIY")).toBeVisible();
    screen.getByRole("button", { name: /Toggle Mini Overlay/ }).click();
    expect(toggleOverlayMock).toHaveBeenCalledWith("mini");
  });
});
