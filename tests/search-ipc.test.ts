import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import {
  IpcError,
  beginSpotifyAuthorization,
  getSpotifySetupStatus,
  openProviderResult,
  parseProviderSearchEvent,
  startSearch,
  subscribeToSearchProviderUpdates,
} from "../src/services/ipc";

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
  listenMock.mockReset();
});

describe("search IPC contracts", () => {
  it("forwards the normalized request and validates the native search id", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValueOnce({ searchId: "search-native-1" });

    await expect(startSearch({
      query: "  signal ",
      lens: "all",
      sortField: "relevance",
      sortDirection: "descending",
      limit: 25,
    })).resolves.toEqual({ searchId: "search-native-1" });
    expect(invokeMock).toHaveBeenCalledWith("start_search", {
      request: {
        query: "signal",
        lens: "all",
        sortField: "relevance",
        sortDirection: "descending",
        limit: 25,
      },
    });
  });

  it("rejects event payloads with unrecognized or secret-bearing fields", () => {
    expect(() => parseProviderSearchEvent({
      searchId: "search-1",
      section: {
        provider: "local",
        state: "ready",
        results: [],
        error: null,
        stderr: "raw tool output",
      },
    })).toThrow();
  });

  it("parses normalized provider events and reports malformed native events", async () => {
    enableNativeRuntime();
    let handler: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation(async (_name: string, next: (event: { payload: unknown }) => void) => {
      handler = next;
      return () => undefined;
    });
    const received: unknown[] = [];
    const errors: Error[] = [];
    await subscribeToSearchProviderUpdates((event) => received.push(event), (error) => errors.push(error));
    handler?.({ payload: { searchId: "search-1", section: { provider: "local", state: "ready", results: [], error: null } } });
    handler?.({ payload: { searchId: "search-1", section: { provider: "local", state: "ready", results: [], error: null, raw: true } } });

    expect(received).toHaveLength(1);
    expect(errors[0]).toBeInstanceOf(IpcError);
    expect(errors[0].message).toContain("invalid provider search event");
  });
});

describe("Spotify IPC contracts", () => {
  it("keeps setup status normalized and sends no client secret", async () => {
    enableNativeRuntime();
    invokeMock
      .mockResolvedValueOnce({ enabled: true, configured: false, available: false, state: "setup_required", market: null, detail: "setup" })
      .mockResolvedValueOnce({ authorizationUrl: "https://accounts.spotify.com/authorize", redirectUri: "http://127.0.0.1:3210/callback" });

    await expect(getSpotifySetupStatus()).resolves.toMatchObject({ state: "setup_required" });
    await beginSpotifyAuthorization("client-id", "us");
    expect(invokeMock).toHaveBeenLastCalledWith("begin_spotify_authorization", { clientId: "client-id", market: "US" });
    expect(JSON.stringify(invokeMock.mock.calls)).not.toContain("secret");
  });

  it("rejects unsafe provider URLs before native IPC", async () => {
    enableNativeRuntime();
    await expect(openProviderResult("youtube", "https://evil.example/video")).rejects.toBeInstanceOf(IpcError);
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("browser preview search boundary", () => {
  it("does not fabricate results outside the explicit E2E adapter gate", async () => {
    await expect(startSearch({
      query: "signal",
      lens: "all",
      sortField: "relevance",
      sortDirection: "descending",
      limit: 25,
    })).rejects.toMatchObject({ message: "Search requires the native SpotDIY runtime." });
  });
});
