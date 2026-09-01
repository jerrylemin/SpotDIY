import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const resetSearchMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/usePlayback", () => ({
  usePlayback: () => ({
    snapshot: {
      currentTrackId: null,
      currentSourceId: null,
      title: null,
      artists: [],
      album: null,
      positionMs: 0,
      durationMs: null,
      abLoop: { aMs: null, bMs: null, active: false },
    },
    pending: false,
  }),
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ children, ...props }: { children: unknown; to: string }) => <a href={props.to} {...props}>{children}</a>,
}));
vi.mock("../src/hooks/useLyrics", () => ({
  useLyrics: () => ({
    data: null,
    isLoading: false,
    error: null,
    searchOnline: { data: [], isPending: false, error: null, reset: resetSearchMock },
    findBest: { isPending: false, error: null },
    selectCandidate: { isPending: false, error: null },
    saveManual: { isPending: false, isError: false, mutateAsync: vi.fn() },
    removeManual: { isPending: false, mutateAsync: vi.fn() },
    importFile: { isPending: false, mutateAsync: vi.fn() },
    clearCache: { isPending: false, mutateAsync: vi.fn() },
  }),
  useBookmarks: () => ({
    data: [],
    isLoading: false,
    create: { isPending: false, mutateAsync: vi.fn() },
    update: { isPending: false, mutateAsync: vi.fn() },
    remove: { isPending: false, mutateAsync: vi.fn() },
  }),
  useAbLoopPresets: () => ({
    data: [],
    save: { isPending: false, mutateAsync: vi.fn() },
    remove: { isPending: false, mutateAsync: vi.fn() },
  }),
}));

import { LyricsPage } from "../src/pages/LyricsPage";

afterEach(() => {
  cleanup();
  resetSearchMock.mockReset();
});

describe("LyricsPage", () => {
  it("shows a truthful no-current-track state", () => {
    render(<LyricsPage />);

    expect(screen.getByText("Choose a track first")).toBeInTheDocument();
    expect(screen.getByText("Lyrics, bookmarks, and A/B controls become available when a track is playing or queued as the current selection.")).toBeInTheDocument();
  });
});
