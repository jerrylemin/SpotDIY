import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const startSearchMock = vi.hoisted(() => vi.fn());
const cancelSearchMock = vi.hoisted(() => vi.fn());
const subscribeProviderMock = vi.hoisted(() => vi.fn());
const subscribeCompletedMock = vi.hoisted(() => vi.fn());

vi.mock("../src/services/ipc", () => ({
  IpcError: class IpcError extends Error {},
  cancelSearch: cancelSearchMock,
  startSearch: startSearchMock,
  subscribeToSearchCompleted: subscribeCompletedMock,
  subscribeToSearchProviderUpdates: subscribeProviderMock,
}));

import { useSearch } from "../src/hooks/useSearch";

const requestOptions = {
  lens: "all" as const,
  sortField: "relevance" as const,
  sortDirection: "descending" as const,
};

function readyEvent(searchId: string, provider: "local" | "youtube" = "local") {
  return {
    searchId,
    section: {
      provider,
      state: "ready" as const,
      results: [],
      error: null,
    },
  };
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  startSearchMock.mockReset();
  cancelSearchMock.mockReset();
  subscribeProviderMock.mockReset();
  subscribeCompletedMock.mockReset();
});

describe("useSearch", () => {
  it("debounces requests by 250ms and ignores stale search IDs", async () => {
    vi.useFakeTimers();
    let providerListener: ((event: ReturnType<typeof readyEvent>) => void) | undefined;
    subscribeProviderMock.mockImplementation(async (listener: typeof providerListener) => {
      providerListener = listener;
      return () => undefined;
    });
    subscribeCompletedMock.mockResolvedValue(() => undefined);
    startSearchMock.mockResolvedValue({ searchId: "search-current" });

    const { result } = renderHook(() => useSearch({ query: "signal", ...requestOptions }));
    expect(startSearchMock).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(249));
    expect(startSearchMock).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(startSearchMock).toHaveBeenCalledOnce();
    expect(result.current.activeSearchId).toBe("search-current");

    act(() => providerListener?.(readyEvent("search-old")));
    expect(result.current.sections.local?.state).toBe("loading");
    act(() => providerListener?.(readyEvent("search-current")));
    expect(result.current.sections.local?.state).toBe("ready");
  });

  it("cancels the previous active search when a query is replaced", async () => {
    vi.useFakeTimers();
    let providerListener: ((event: ReturnType<typeof readyEvent>) => void) | undefined;
    subscribeProviderMock.mockImplementation(async (listener: typeof providerListener) => {
      providerListener = listener;
      return () => undefined;
    });
    subscribeCompletedMock.mockResolvedValue(() => undefined);
    startSearchMock
      .mockResolvedValueOnce({ searchId: "search-one" })
      .mockResolvedValueOnce({ searchId: "search-two" });
    cancelSearchMock.mockResolvedValue("search-one");

    const { result, rerender } = renderHook(({ query }: { query: string }) => useSearch({ query, ...requestOptions }), {
      initialProps: { query: "one" },
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    expect(result.current.activeSearchId).toBe("search-one");

    rerender({ query: "two" });
    expect(cancelSearchMock).toHaveBeenCalled();
    act(() => providerListener?.(readyEvent("search-one")));
    expect(result.current.sections.local).toBeUndefined();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    expect(result.current.activeSearchId).toBe("search-two");
    act(() => providerListener?.(readyEvent("search-two", "youtube")));
    expect(result.current.sections.youtube?.state).toBe("ready");
  });

  it("exposes cancellation and clear behavior", async () => {
    vi.useFakeTimers();
    subscribeProviderMock.mockResolvedValue(() => undefined);
    subscribeCompletedMock.mockResolvedValue(() => undefined);
    startSearchMock.mockResolvedValue({ searchId: "search-cancel" });
    cancelSearchMock.mockResolvedValue("search-cancel");

    const { result } = renderHook(() => useSearch({ query: "signal", ...requestOptions }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    expect(result.current.activeSearchId).toBe("search-cancel");
    await act(async () => result.current.cancel());
    expect(result.current.activeSearchId).toBeNull();
    expect(result.current.sections.local?.state).toBe("cancelled");

    await act(async () => result.current.clear());
    expect(result.current.sections).toEqual({});
  });
});
