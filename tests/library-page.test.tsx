import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const nativeRuntimeMock = vi.hoisted(() => vi.fn());
const pickLibraryFoldersMock = vi.hoisted(() => vi.fn());
const statusResult = vi.hoisted(() => ({ data: undefined as unknown, isLoading: false, isError: false, refetch: vi.fn() }));
const pageResult = vi.hoisted(() => ({ data: undefined as unknown, isLoading: false, isFetching: false, isError: false, error: null as unknown }));
const progressResult = vi.hoisted(() => ({ current: null as unknown }));
const addMutation = vi.hoisted(() => ({ isPending: false, error: null as unknown, mutateAsync: vi.fn() }));
const removeMutation = vi.hoisted(() => ({ isPending: false, error: null as unknown, mutate: vi.fn() }));
const rescanMutation = vi.hoisted(() => ({ isPending: false, error: null as unknown, mutate: vi.fn() }));
const rescanAllMutation = vi.hoisted(() => ({ isPending: false, error: null as unknown, mutate: vi.fn() }));
const revealMutation = vi.hoisted(() => ({ isPending: false, error: null as unknown, mutate: vi.fn() }));
const playbackResult = vi.hoisted(() => ({
  snapshot: {
    revision: 0,
    phase: "idle",
    currentQueueEntryId: null,
    currentTrackId: null,
    currentSourceId: null,
    title: null,
    artists: [],
    album: null,
    artworkPath: null,
    sources: [],
    positionMs: 0,
    durationMs: null,
    volumePercent: 100,
    muted: false,
    repeatMode: "off",
    shuffleEnabled: false,
    queueLength: 0,
    queueIndex: null,
    selectedAudioDevice: "auto",
    backendHealth: { ready: false, connected: false, detail: null, recoveryAction: null },
    recovering: false,
    error: null,
    abLoop: { aMs: null, bMs: null, active: false },
  },
  audioDevices: [],
  hydrated: true,
  initializing: false,
  audioDevicesLoading: false,
  bridgeError: null as string | null,
  pending: false,
  refreshSnapshot: vi.fn(),
  refreshDevices: vi.fn(),
  playNow: vi.fn(),
  addToQueue: vi.fn(),
  playNext: vi.fn(),
  togglePlayPause: vi.fn(),
  nextTrack: vi.fn(),
  previousTrack: vi.fn(),
  seekPlayback: vi.fn(),
  setVolume: vi.fn(),
  setMuted: vi.fn(),
  toggleMuted: vi.fn(),
  cycleRepeatMode: vi.fn(),
  setShuffleEnabled: vi.fn(),
  toggleShuffle: vi.fn(),
  setAudioDevice: vi.fn(),
  switchSource: vi.fn(),
  retryPlaybackBackend: vi.fn(),
  clearQueue: vi.fn(),
  warmAudioDevices: vi.fn(),
}));

vi.mock("../src/services/ipc", () => ({
  isTauriRuntime: nativeRuntimeMock,
  pickLibraryFolders: pickLibraryFoldersMock,
  IpcError: class IpcError extends Error {},
  providerLabel: () => "LOCAL",
}));
vi.mock("../src/hooks/useLibrary", () => ({
  useLibraryStatus: () => statusResult,
  useLibraryPage: () => pageResult,
  useLibraryProgress: () => progressResult.current,
  useAddLibraryFolders: () => addMutation,
  useRemoveLibraryFolder: () => removeMutation,
  useRescanLibraryFolder: () => rescanMutation,
  useRescanAllLibraryFolders: () => rescanAllMutation,
  useRevealLocalFile: () => revealMutation,
}));
vi.mock("../src/hooks/usePlayback", () => ({
  usePlayback: () => playbackResult,
}));

import { LibraryPage } from "../src/pages/LibraryPage";

const folder = {
  id: "folder-1",
  path: "C:\\Music",
  normalizedPathKey: "c:\\music",
  enabled: true,
  status: "complete",
  scanGeneration: 2,
  lastScanStartedAt: "2026-08-30T00:00:00Z",
  lastScanFinishedAt: "2026-08-30T00:01:00Z",
  lastScanError: null,
  fileCount: 1,
  indexedTrackCount: 1,
  createdAt: "2026-08-29T00:00:00Z",
  updatedAt: "2026-08-30T00:01:00Z",
} as const;

const track = {
  trackId: "track-1",
  sourceId: "source-1",
  folderId: folder.id,
  title: "Night Drive",
  artists: ["Luna Max"],
  album: "Afterglow",
  durationMs: 185000,
  path: "C:\\Music\\night-drive.flac",
  available: true,
  availabilityDetail: null,
  indexStatus: "indexed",
  statusDetail: null,
  fileSizeBytes: 1234,
  modifiedAt: "2026-08-30T00:00:00Z",
  codec: "FLAC",
  container: "FLAC",
  bitrateKbps: null,
  sampleRateHz: 44100,
  bitDepth: 16,
  contentFingerprint: "fingerprint",
  artworkCacheKey: null,
  artworkMimeType: null,
  artworkPath: null,
  createdAt: "2026-08-30T00:00:00Z",
  updatedAt: "2026-08-30T00:00:00Z",
} as const;

function setState({ folders = [], indexedTrackCount = 0, availableTrackCount = 0, isScanning = false } = {}) {
  statusResult.data = { folders, indexedTrackCount, availableTrackCount, isScanning };
  statusResult.isLoading = false;
  statusResult.isError = false;
  pageResult.data = {
    items: [],
    page: 0,
    pageSize: 50,
    total: 0,
    hasNext: false,
    sort: "title",
    descending: false,
  };
  pageResult.isLoading = false;
  pageResult.isFetching = false;
  pageResult.isError = false;
  pageResult.error = null;
  progressResult.current = null;
}

afterEach(() => {
  cleanup();
  nativeRuntimeMock.mockReset();
  nativeRuntimeMock.mockReturnValue(true);
  pickLibraryFoldersMock.mockReset();
  statusResult.data = undefined;
  statusResult.isLoading = false;
  statusResult.isError = false;
  pageResult.data = undefined;
  pageResult.isLoading = false;
  pageResult.isFetching = false;
  pageResult.isError = false;
  pageResult.error = null;
  progressResult.current = null;
  addMutation.isPending = false;
  addMutation.error = null;
  removeMutation.isPending = false;
  removeMutation.error = null;
  rescanMutation.isPending = false;
  rescanMutation.error = null;
  rescanAllMutation.isPending = false;
  rescanAllMutation.error = null;
  revealMutation.isPending = false;
  revealMutation.error = null;
  addMutation.mutateAsync.mockReset();
  removeMutation.mutate.mockReset();
  rescanMutation.mutate.mockReset();
  rescanAllMutation.mutate.mockReset();
  revealMutation.mutate.mockReset();
  playbackResult.refreshSnapshot.mockReset();
  playbackResult.refreshDevices.mockReset();
  playbackResult.playNow.mockReset();
  playbackResult.addToQueue.mockReset();
  playbackResult.playNext.mockReset();
  playbackResult.togglePlayPause.mockReset();
  playbackResult.nextTrack.mockReset();
  playbackResult.previousTrack.mockReset();
  playbackResult.seekPlayback.mockReset();
  playbackResult.setVolume.mockReset();
  playbackResult.setMuted.mockReset();
  playbackResult.toggleMuted.mockReset();
  playbackResult.cycleRepeatMode.mockReset();
  playbackResult.setShuffleEnabled.mockReset();
  playbackResult.toggleShuffle.mockReset();
  playbackResult.setAudioDevice.mockReset();
  playbackResult.switchSource.mockReset();
  playbackResult.retryPlaybackBackend.mockReset();
  playbackResult.clearQueue.mockReset();
  playbackResult.warmAudioDevices.mockReset();
  statusResult.refetch.mockReset();
  vi.restoreAllMocks();
});

describe("LibraryPage", () => {
  it("explains the browser preview without inventing folders or tracks", () => {
    nativeRuntimeMock.mockReturnValue(false);
    setState();

    render(<LibraryPage />);

    expect(screen.getByText("No music folders connected")).toBeInTheDocument();
    expect(screen.getByText(/browser preview cannot access your music folders/i)).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /add folder/i }).every((button) => (button as HTMLButtonElement).disabled)).toBe(true);
    expect(screen.queryByText("Night Drive")).not.toBeInTheDocument();
  });

  it("shows active scan progress and measured track quality", () => {
    setState({ folders: [folder], indexedTrackCount: 1, availableTrackCount: 1, isScanning: true });
    pageResult.data = { ...pageResult.data, items: [track], total: 1 };
    progressResult.current = {
      folderId: folder.id,
      status: "scanning",
      currentFile: "C:\\Music\\night-drive.flac",
      processed: 1,
      candidates: 3,
      summary: null,
      startedAt: null,
      finishedAt: null,
      error: null,
    };

    render(<LibraryPage />);

    expect(screen.getByText(/indexing local files/i)).toBeInTheDocument();
    expect(screen.getByText(/1 of 3 files checked/i)).toBeInTheDocument();
    expect(screen.getByText("Night Drive")).toBeInTheDocument();
    expect(screen.getByText("FLAC")).toBeInTheDocument();
    expect(screen.getByText("44.1 kHz")).toBeInTheDocument();
    expect(screen.getByText("16-bit")).toBeInTheDocument();
    expect(screen.getByText("3:05")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Play now Night Drive" })).toBeEnabled();
    expect(screen.getByTestId("library-track-track-1").querySelector("img")).toBeNull();
  });

  it("supports real pagination and folder actions", async () => {
    setState({ folders: [folder], indexedTrackCount: 51, availableTrackCount: 1 });
    pageResult.data = { ...pageResult.data, items: [track], total: 51, hasNext: true };

    render(<LibraryPage />);

    expect(screen.getByText("Showing 1–1 of 51")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
    await userEvent.click(screen.getByRole("button", { name: "Rescan" }));
    await userEvent.click(screen.getByRole("button", { name: /open file location/i }));

    expect(rescanMutation.mutate).toHaveBeenCalledWith(folder.id);
    expect(revealMutation.mutate).toHaveBeenCalledWith(track.sourceId);
  });

  it("sends the selected folders to native IPC", async () => {
    setState();
    pickLibraryFoldersMock.mockResolvedValueOnce(["D:\\Archive"]);
    addMutation.mutateAsync.mockResolvedValueOnce([]);

    render(<LibraryPage />);
    await userEvent.click(screen.getByRole("button", { name: "Add folder" }));
    await waitFor(() => expect(addMutation.mutateAsync).toHaveBeenCalledWith(["D:\\Archive"]));
  });

  it("keeps unavailable files visible and disables their reveal action", () => {
    setState({ folders: [folder], indexedTrackCount: 1, availableTrackCount: 0 });
    pageResult.data = {
      ...pageResult.data,
      items: [{ ...track, available: false, indexStatus: "missing", availabilityDetail: "File was not found during the last scan" }],
      total: 1,
    };

    render(<LibraryPage />);

    expect(screen.getByText("Unavailable")).toBeInTheDocument();
    expect(screen.getByText("File was not found during the last scan")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /play now/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /open file location/i })).toBeDisabled();
  });

  it("confirms folder removal and leaves the file system action explicit", async () => {
    setState({ folders: [folder], indexedTrackCount: 1, availableTrackCount: 1 });
    const confirmMock = vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<LibraryPage />);
    await userEvent.click(screen.getByRole("button", { name: "Remove C:\\Music" }));

    expect(confirmMock).toHaveBeenCalledWith(expect.stringMatching(/files will remain untouched/i));
    expect(removeMutation.mutate).toHaveBeenCalledWith(folder.id);
  });
});
