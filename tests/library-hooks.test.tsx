import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";

const listenMock = vi.hoisted(() => vi.fn());
const isTauriRuntimeMock = vi.hoisted(() => vi.fn());
const parseScanProgressMock = vi.hoisted(() => vi.fn((value: unknown) => value));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("../src/services/ipc", () => ({
  LIBRARY_PROGRESS_EVENT: "library://scan-progress",
  isTauriRuntime: isTauriRuntimeMock,
  parseScanProgress: parseScanProgressMock,
  getLibraryStatus: vi.fn(),
  getLibraryPage: vi.fn(),
  addLibraryFolders: vi.fn(),
  removeLibraryFolder: vi.fn(),
  rescanLibraryFolder: vi.fn(),
  rescanAllLibraryFolders: vi.fn(),
  revealLocalFile: vi.fn(),
}));

import { useLibraryProgress } from "../src/hooks/useLibrary";

function createWrapper(queryClient: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

afterEach(() => {
  listenMock.mockReset();
  isTauriRuntimeMock.mockReset();
  parseScanProgressMock.mockReset();
});

describe("library scan progress", () => {
  it("subscribes in the native runtime and cleans up the listener", async () => {
    isTauriRuntimeMock.mockReturnValue(true);
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const { result, unmount } = renderHook(() => useLibraryProgress(), {
      wrapper: createWrapper(queryClient),
    });
    await waitFor(() => expect(listenMock).toHaveBeenCalledWith("library://scan-progress", expect.any(Function)));

    const payload = {
      folderId: "folder-id",
      status: "scanning",
      currentFile: "C:\\Music\\song.flac",
      processed: 2,
      candidates: 3,
      summary: null,
      startedAt: null,
      finishedAt: null,
      error: null,
    };
    listenMock.mock.calls[0][1]({ payload });
    await waitFor(() => expect(result.current?.processed).toBe(2));

    unmount();
    expect(unlisten).toHaveBeenCalledOnce();
    queryClient.clear();
  });

  it("does not subscribe in browser preview", () => {
    isTauriRuntimeMock.mockReturnValue(false);
    const queryClient = new QueryClient();

    renderHook(() => useLibraryProgress(), { wrapper: createWrapper(queryClient) });

    expect(listenMock).not.toHaveBeenCalled();
    queryClient.clear();
  });
});
