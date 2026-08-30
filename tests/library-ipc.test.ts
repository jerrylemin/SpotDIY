import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const openMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));

import {
  IpcError,
  addLibraryFolders,
  getLibraryPage,
  parseScanProgress,
  pickLibraryFolders,
} from "../src/services/ipc";

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
  openMock.mockReset();
});

describe("library dialog IPC", () => {
  it("returns multiple selected directories and uses the directory picker", async () => {
    enableNativeRuntime();
    openMock.mockResolvedValueOnce(["C:\\Music", "D:\\Archive"]);

    await expect(pickLibraryFolders()).resolves.toEqual(["C:\\Music", "D:\\Archive"]);
    expect(openMock).toHaveBeenCalledWith({
      directory: true,
      multiple: true,
      title: "Choose music folders",
    });
  });

  it("turns dialog cancellation into an empty selection", async () => {
    enableNativeRuntime();
    openMock.mockResolvedValueOnce(null);

    await expect(pickLibraryFolders()).resolves.toEqual([]);
  });

  it("does not invoke native IPC for an empty add selection", async () => {
    enableNativeRuntime();

    await expect(addLibraryFolders([])).resolves.toEqual([]);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("validates page responses and wraps malformed native data", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValueOnce({ items: [], page: 0 });

    await expect(
      getLibraryPage({
        page: 0,
        pageSize: 50,
        sort: "title",
        descending: false,
        folderId: null,
      }),
    ).rejects.toBeInstanceOf(IpcError);
  });
});

describe("library scan progress parsing", () => {
  it("accepts the typed progress payload", () => {
    expect(
      parseScanProgress({
        folderId: "folder-id",
        status: "scanning",
        currentFile: "C:\\Music\\song.flac",
        processed: 1,
        candidates: 2,
        summary: null,
        startedAt: null,
        finishedAt: null,
        error: null,
      }),
    ).toMatchObject({ status: "scanning", processed: 1 });
  });
});
