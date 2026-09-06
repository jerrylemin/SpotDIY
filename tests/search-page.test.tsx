import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

const useAppStatusMock = vi.hoisted(() => vi.fn());
const useSearchMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/useAppStatus", () => ({ useAppStatus: useAppStatusMock }));
vi.mock("../src/hooks/useSearch", () => ({
  searchProviderOrder: (lens: string) => lens === "spotify" ? ["spotify"] : lens === "youtube" ? ["youtube"] : lens === "soundcloud" ? ["soundcloud"] : ["local", "youtube", "soundcloud", "spotify"],
  useSearch: useSearchMock,
}));
vi.mock("../src/components/search/ProviderSearchSection", () => ({
  ProviderSearchSection: ({ section }: { section: { provider: string; state: string } }) => <article data-provider={section.provider}>{section.provider}:{section.state}</article>,
}));
vi.mock("@tanstack/react-router", () => ({ Link: ({ children }: { children: unknown }) => <a href="/library">{children}</a> }));

import { SearchPage } from "../src/pages/SearchPage";

const provider = (kind: "local" | "youtube" | "soundcloud" | "spotify", detail: string) => ({
  kind,
  label: detail,
  configured: kind === "local",
  available: kind === "local",
  runtimeStatus: kind === "spotify" ? "ready" as const : kind === "local" ? "ready" as const : "missing" as const,
  capabilities: {
    search: true,
    playback: kind === "local",
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: kind === "youtube" || kind === "soundcloud" || kind === "spotify",
    popularity: kind !== "local" && kind !== "spotify",
    releaseDate: false,
    lyricsMetadata: false,
  },
  detail,
});

const mediaTools = {
  ytDlp: { status: "ready" as const, version: "2026.08.19", detail: null },
  ffmpeg: { status: "ready" as const, version: "7.0.0", detail: null },
  mpv: { status: "ready" as const, version: "0.41.0", detail: null },
};

afterEach(() => {
  cleanup();
  useAppStatusMock.mockReset();
  useSearchMock.mockReset();
});

describe("SearchPage", () => {
  it("keeps provider order fixed and shows independent states", async () => {
    const user = userEvent.setup();
    useAppStatusMock.mockReturnValue({ data: { version: "0.1.0", runtime: "tauri", storageMode: "standard", firstRun: false, tracksIndexed: 0, musicFolders: [], downloadsDirectory: "C:\\Downloads", downloadDirectoryStatus: "ready", mediaTools, providers: [
      provider("local", "Local library"),
      provider("youtube", "YouTube"),
      provider("soundcloud", "SoundCloud"),
      provider("spotify", "Spotify"),
    ] }, isError: false });
    useSearchMock.mockReturnValue({
      sections: {
        local: { provider: "local", state: "ready", results: [], error: null },
        youtube: { provider: "youtube", state: "failed", results: [], error: { code: "unavailable", detail: "yt-dlp missing", retryAfterSeconds: null } },
        soundcloud: { provider: "soundcloud", state: "loading", results: [], error: null },
        spotify: { provider: "spotify", state: "loading", results: [], error: null },
      },
      activeSearchId: "search-1",
      isSearching: true,
      isDebouncing: false,
      error: null,
      cancel: vi.fn(),
      clear: vi.fn(),
      retry: vi.fn(),
    });

    render(<SearchPage />);
    await user.type(screen.getByRole("textbox", { name: "Search music" }), "signal");
    const sections = screen.getAllByRole("article");
    expect(sections.map((section) => section.getAttribute("data-provider"))).toEqual(["local", "youtube", "soundcloud", "spotify"]);
    expect(screen.getByText("local:ready")).toBeInTheDocument();
    expect(screen.getByText("youtube:failed")).toBeInTheDocument();
    expect(screen.getByText("soundcloud:loading")).toBeInTheDocument();
    expect(screen.getByText("spotify:loading")).toBeInTheDocument();
  });

  it("isolates Spotify to the Spotify lens", async () => {
    const user = userEvent.setup();
    useAppStatusMock.mockReturnValue({ data: { version: "0.1.0", runtime: "tauri", storageMode: "standard", firstRun: false, tracksIndexed: 0, musicFolders: [], downloadsDirectory: "C:\\Downloads", downloadDirectoryStatus: "ready", mediaTools, providers: [provider("spotify", "Spotify")] }, isError: false });
    useSearchMock.mockReturnValue({ sections: { spotify: { provider: "spotify", state: "ready", results: [], error: null } }, activeSearchId: null, isSearching: false, isDebouncing: false, error: null, cancel: vi.fn(), clear: vi.fn(), retry: vi.fn() });

    render(<SearchPage />);
    await user.type(screen.getByRole("textbox", { name: "Search music" }), "signal");
    await user.click(screen.getByRole("tab", { name: "SPOTIFY" }));
    expect(screen.getByText("spotify:ready")).toBeInTheDocument();
    expect(screen.queryByText("local:failed")).not.toBeInTheDocument();
  });
});
