import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";

const useDownloadSnapshotMock = vi.hoisted(() => vi.fn());
const cancelMutation = vi.hoisted(() => ({ isPending: false, mutateAsync: vi.fn() }));
const retryMutation = vi.hoisted(() => ({ isPending: false, mutateAsync: vi.fn() }));
const concurrencyMutation = vi.hoisted(() => ({ isPending: false, mutateAsync: vi.fn() }));
const isTauriRuntimeMock = vi.hoisted(() => vi.fn(() => true));
const pickDownloadDirectoryMock = vi.hoisted(() => vi.fn());
const setSettingMock = vi.hoisted(() => vi.fn());
const openDownloadLocationMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/useDownloads", () => ({
  DOWNLOAD_SNAPSHOT_QUERY_KEY: ["download-snapshot"],
  useDownloadSnapshot: useDownloadSnapshotMock,
  useCancelDownload: () => cancelMutation,
  useRetryDownload: () => retryMutation,
  useSetDownloadConcurrency: () => concurrencyMutation,
}));
vi.mock("../src/services/ipc", () => ({
  IpcError: class IpcError extends Error {},
  isTauriRuntime: isTauriRuntimeMock,
  openDownloadLocation: openDownloadLocationMock,
  pickDownloadDirectory: pickDownloadDirectoryMock,
  providerLabel: (kind: string) => kind,
  setSetting: setSettingMock,
}));

import { DownloadsPage } from "../src/pages/DownloadsPage";

function snapshot() {
  return {
    revision: 1,
    tasks: [{
      id: "download-1",
      providerKind: "youtube",
      providerItemId: "video-1",
      canonicalUrl: "https://www.youtube.com/watch?v=video-1",
      targetTrackId: null,
      targetSourceId: null,
      title: "Fixture video",
      artists: ["Fixture artist"],
      artworkUrl: null,
      mode: "audio",
      state: "completed",
      destinationDirectory: "C:\\Downloads",
      outputPath: "C:\\Downloads\\Fixture.webm",
      outputExtension: "webm",
      outputCodec: null,
      sourceQualityProvenance: "providerEncoded",
      transcoded: false,
      expectedBytes: 4,
      downloadedBytes: 4,
      progressPermille: 1000,
      speedBytesPerSecond: null,
      etaSeconds: null,
      retryCount: 0,
      errorCode: null,
      errorDetail: null,
      createdAt: "2026-09-01T00:00:00Z",
      updatedAt: "2026-09-01T00:00:00Z",
      startedAt: "2026-09-01T00:00:00Z",
      completedAt: "2026-09-01T00:00:01Z",
      outputMissing: false,
    }],
    maxConcurrent: 2,
    downloadsDirectory: "C:\\Downloads",
    tools: {
      ytDlp: { status: "ready", version: "2026.08.19", detail: null },
      ffmpeg: { status: "missing", version: null, detail: "Install FFmpeg for video downloads." },
    },
  };
}

function createWrapper() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

afterEach(() => {
  cleanup();
  useDownloadSnapshotMock.mockReset();
  cancelMutation.mutateAsync.mockReset();
  retryMutation.mutateAsync.mockReset();
  concurrencyMutation.mutateAsync.mockReset();
  pickDownloadDirectoryMock.mockReset();
  setSettingMock.mockReset();
  openDownloadLocationMock.mockReset();
  isTauriRuntimeMock.mockReturnValue(true);
});

describe("DownloadsPage", () => {
  it("renders persistent task facts and filters the queue", () => {
    useDownloadSnapshotMock.mockReturnValue({ data: snapshot(), isLoading: false, isError: false, error: null, eventError: null });
    render(<DownloadsPage />, { wrapper: createWrapper() });

    expect(screen.getByRole("heading", { name: "Managed downloads" })).toBeInTheDocument();
    expect(screen.getByText("Fixture video")).toBeInTheDocument();
    expect(screen.getByText(/Provider encoded/)).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Filter downloads" }), { target: { value: "missing" } });
    expect(screen.getByText("No tasks match this filter.")).toBeInTheDocument();
  });

  it("keeps folder selection and open-location actions inside native IPC", async () => {
    useDownloadSnapshotMock.mockReturnValue({ data: snapshot(), isLoading: false, isError: false, error: null, eventError: null });
    pickDownloadDirectoryMock.mockResolvedValueOnce("D:\\SpotDIY Downloads");
    setSettingMock.mockResolvedValueOnce({});
    openDownloadLocationMock.mockResolvedValueOnce(undefined);
    render(<DownloadsPage />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByRole("button", { name: /Choose folder/ }));
    await waitFor(() => expect(setSettingMock).toHaveBeenCalledWith({ key: "downloadsDirectory", value: "D:\\SpotDIY Downloads" }));
    fireEvent.click(screen.getByRole("button", { name: /Open folder/ }));
    await waitFor(() => expect(openDownloadLocationMock).toHaveBeenCalledWith("download-1"));
  });
});
