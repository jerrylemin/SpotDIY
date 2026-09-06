import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

const queueDownloadMock = vi.hoisted(() => vi.fn());
const playSearchResultMock = vi.hoisted(() => vi.fn());
const pickDownloadDirectoryMock = vi.hoisted(() => vi.fn());
const setSettingMock = vi.hoisted(() => vi.fn());
const isTauriRuntimeMock = vi.hoisted(() => vi.fn(() => true));

vi.mock("../src/services/ipc", () => ({
  isTauriRuntime: isTauriRuntimeMock,
  openProviderResult: vi.fn(),
  pickDownloadDirectory: pickDownloadDirectoryMock,
  playSearchResult: playSearchResultMock,
  providerLabel: (kind: string) => kind,
  queueSearchResultDownload: queueDownloadMock,
  revealLocalFile: vi.fn(),
  setSetting: setSettingMock,
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
import type { DownloadReadiness } from "../src/features/actions/track-actions";

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
  cleanup();
  queueDownloadMock.mockReset();
  playSearchResultMock.mockReset();
  pickDownloadDirectoryMock.mockReset();
  setSettingMock.mockReset();
  isTauriRuntimeMock.mockReturnValue(true);
});

describe("provider search download action", () => {
  it("offers audio/video modes only for supported provider tracks", async () => {
    queueDownloadMock.mockResolvedValueOnce({});
    const readiness: DownloadReadiness = {
      ytDlpStatus: "ready",
      ffmpegStatus: "ready",
      mpvStatus: "ready",
      downloadDirectoryStatus: "ready",
    };
    render(<SearchResultCard
      capabilities={{ search: true, playback: false, metadata: true, artwork: true, lyrics: false, downloads: true, popularity: true, releaseDate: false, lyricsMetadata: false }}
      downloadReadiness={readiness}
      result={providerResult}
    />);

    fireEvent.change(screen.getByRole("combobox", { name: "Download mode for Provider fixture" }), { target: { value: "video" } });
    fireEvent.click(screen.getByRole("button", { name: /Download/ }));
    await waitFor(() => expect(queueDownloadMock).toHaveBeenCalledWith(providerResult, "video"));
  });

  it("plays a YouTube result through the native playback command", async () => {
    playSearchResultMock.mockResolvedValueOnce({});
    render(<SearchResultCard
      capabilities={{ search: true, playback: true, metadata: true, artwork: true, lyrics: false, downloads: true, popularity: true, releaseDate: false, lyricsMetadata: false }}
      downloadReadiness={{ ytDlpStatus: "ready", ffmpegStatus: "ready", mpvStatus: "ready", downloadDirectoryStatus: "ready" }}
      result={providerResult}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Play online" }));
    await waitFor(() => expect(playSearchResultMock).toHaveBeenCalledWith(providerResult));
  });

  it("opens the folder picker before queueing when the download folder is missing", async () => {
    pickDownloadDirectoryMock.mockResolvedValueOnce("C:\\Downloads");
    setSettingMock.mockResolvedValueOnce({});
    queueDownloadMock.mockResolvedValueOnce({});
    render(<SearchResultCard
      capabilities={{ search: true, playback: true, metadata: true, artwork: true, lyrics: false, downloads: true, popularity: true, releaseDate: false, lyricsMetadata: false }}
      downloadReadiness={{ ytDlpStatus: "ready", ffmpegStatus: "ready", mpvStatus: "ready", downloadDirectoryStatus: "missing" }}
      result={providerResult}
    />);

    const download = screen.getByRole("button", { name: "Download" });
    expect(download).toBeEnabled();
    fireEvent.click(download);
    await waitFor(() => expect(setSettingMock).toHaveBeenCalledWith({ key: "downloadsDirectory", value: "C:\\Downloads" }));
    expect(queueDownloadMock).toHaveBeenCalledWith(providerResult, "audio");
  });

  it("renders SoundCloud as one audio action with the native reason when unavailable", () => {
    const soundcloud = { ...providerResult, provider: "soundcloud" as const, canonicalUrl: "https://soundcloud.com/artist/fixture" };
    render(<SearchResultCard
      capabilities={{ search: true, playback: false, metadata: true, artwork: true, lyrics: false, downloads: true, popularity: true, releaseDate: false, lyricsMetadata: false }}
      downloadReadiness={{ ytDlpStatus: "missing", ffmpegStatus: "ready", downloadDirectoryStatus: "ready" }}
      result={soundcloud}
    />);

    expect(screen.queryByRole("combobox", { name: `Download mode for ${soundcloud.title}` })).not.toBeInTheDocument();
    const button = screen.getByRole("button", { name: "Download audio" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("title", "yt-dlp is not available.");
  });
});
