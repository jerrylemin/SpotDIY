import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const queueDownloadMock = vi.hoisted(() => vi.fn());
const isTauriRuntimeMock = vi.hoisted(() => vi.fn(() => true));

vi.mock("../src/services/ipc", () => ({
  isTauriRuntime: isTauriRuntimeMock,
  openProviderResult: vi.fn(),
  providerLabel: (kind: string) => kind,
  queueSearchResultDownload: queueDownloadMock,
  revealLocalFile: vi.fn(),
}));
vi.mock("../src/hooks/usePlayback", () => ({
  usePlayback: () => ({
    playNow: vi.fn(),
    addToQueue: vi.fn(),
    playNext: vi.fn(),
  }),
}));

import { SearchResultCard } from "../src/components/search/SearchResultCard";
import type { SearchResult } from "../src/types/domain";

const providerResult: SearchResult = {
  provider: "youtube",
  entityKind: "track",
  providerItemId: "video-1",
  canonicalUrl: "https://www.youtube.com/watch?v=video-1",
  title: "Provider fixture",
  artists: ["Fixture artist"],
  album: null,
  durationMs: 10_000,
  artworkUrl: null,
  publishedAt: null,
  engagementCount: null,
  engagementKind: null,
  explicit: null,
  localTrackId: null,
  localSourceId: null,
  originalRank: 1,
};

afterEach(() => {
  queueDownloadMock.mockReset();
  isTauriRuntimeMock.mockReturnValue(true);
});

describe("provider search download action", () => {
  it("offers audio/video modes only for supported provider tracks", async () => {
    queueDownloadMock.mockResolvedValueOnce({});
    render(<SearchResultCard result={providerResult} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Download mode for Provider fixture" }), { target: { value: "video" } });
    fireEvent.click(screen.getByRole("button", { name: /Download/ }));
    await waitFor(() => expect(queueDownloadMock).toHaveBeenCalledWith(providerResult, "video"));
  });
});
