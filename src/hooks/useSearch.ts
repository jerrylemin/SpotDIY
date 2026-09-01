import { useCallback, useEffect, useRef, useState } from "react";

import {
  IpcError,
  cancelSearch,
  startSearch,
  subscribeToSearchCompleted,
  subscribeToSearchProviderUpdates,
} from "../services/ipc";
import type {
  ProviderKind,
  ProviderSearchSection,
  SearchId,
  SearchLens,
  SearchRequest,
  SearchSortDirection,
  SearchSortField,
} from "../types/domain";

export const SEARCH_PROVIDER_ORDER: ProviderKind[] = ["local", "youtube", "soundcloud"];

export interface UseSearchOptions {
  query: string;
  lens: SearchLens;
  sortField: SearchSortField;
  sortDirection: SearchSortDirection;
  limit?: number;
}

export type SearchSections = Partial<Record<ProviderKind, ProviderSearchSection>>;

export interface UseSearchResult {
  sections: SearchSections;
  activeSearchId: SearchId | null;
  isSearching: boolean;
  isDebouncing: boolean;
  error: string | null;
  cancel: () => Promise<void>;
  clear: () => Promise<void>;
  retry: () => void;
}

function providersForLens(lens: SearchLens): ProviderKind[] {
  switch (lens) {
    case "local":
      return ["local"];
    case "youtube":
      return ["youtube"];
    case "soundcloud":
      return ["soundcloud"];
    case "spotify":
      return ["spotify"];
    case "artists":
    case "albums":
      return ["local"];
    default:
      return SEARCH_PROVIDER_ORDER;
  }
}

export function searchProviderOrder(lens: SearchLens): ProviderKind[] {
  return [...providersForLens(lens)];
}

function loadingSections(lens: SearchLens): SearchSections {
  return Object.fromEntries(providersForLens(lens).map((provider) => [provider, {
    provider,
    state: "loading",
    results: [],
    error: null,
  }])) as SearchSections;
}

function cancelledSections(lens: SearchLens): SearchSections {
  return Object.fromEntries(providersForLens(lens).map((provider) => [provider, {
    provider,
    state: "cancelled",
    results: [],
    error: {
      code: "cancelled",
      detail: "Search cancelled.",
      retryAfterSeconds: null,
    },
  }])) as SearchSections;
}

function failedSections(lens: SearchLens, detail: string): SearchSections {
  return Object.fromEntries(providersForLens(lens).map((provider) => [provider, {
    provider,
    state: "failed",
    results: [],
    error: {
      code: "failed",
      detail,
      retryAfterSeconds: null,
    },
  }])) as SearchSections;
}

function errorMessage(error: unknown): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "SpotDIY could not search the configured sources.";
}

export function useSearch({ query, lens, sortField, sortDirection, limit = 25 }: UseSearchOptions): UseSearchResult {
  const [sections, setSections] = useState<SearchSections>({});
  const [activeSearchId, setActiveSearchId] = useState<SearchId | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isDebouncing, setIsDebouncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryNonce, setRetryNonce] = useState(0);
  const activeSearchIdRef = useRef<SearchId | null>(null);
  const generationRef = useRef(0);
  const debounceTimerRef = useRef<number | null>(null);

  useEffect(() => {
    let mounted = true;
    let stopProviderUpdates: (() => void) | undefined;
    let stopCompletions: (() => void) | undefined;

    void Promise.all([
      subscribeToSearchProviderUpdates((event) => {
        if (!mounted || activeSearchIdRef.current !== event.searchId) {
          return;
        }
        setSections((current) => ({ ...current, [event.section.provider]: event.section }));
      }, (bridgeError) => {
        if (mounted) {
          setError(bridgeError.message);
        }
      }),
      subscribeToSearchCompleted((event) => {
        if (!mounted || activeSearchIdRef.current !== event.searchId) {
          return;
        }
        activeSearchIdRef.current = null;
        setActiveSearchId(null);
        setIsSearching(false);
        setIsDebouncing(false);
      }, (bridgeError) => {
        if (mounted) {
          setError(bridgeError.message);
        }
      }),
    ]).then(([providerStop, completionStop]) => {
      if (mounted) {
        stopProviderUpdates = providerStop;
        stopCompletions = completionStop;
      } else {
        providerStop();
        completionStop();
      }
    }).catch((subscriptionError) => {
      if (mounted) {
        setError(errorMessage(subscriptionError));
      }
    });

    return () => {
      mounted = false;
      stopProviderUpdates?.();
      stopCompletions?.();
    };
  }, []);

  useEffect(() => {
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    const normalizedQuery = query.trim();
    const previousSearchId = activeSearchIdRef.current;
    activeSearchIdRef.current = null;
    setActiveSearchId(null);
    setSections({});
    setError(null);
    setIsDebouncing(false);

    if (debounceTimerRef.current !== null) {
      window.clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }

    if (previousSearchId) {
      void Promise.resolve(cancelSearch()).catch(() => undefined);
    }

    if (!normalizedQuery) {
      setIsSearching(false);
      return undefined;
    }

    setIsSearching(true);
    setIsDebouncing(true);
    const request: SearchRequest = {
      query: normalizedQuery,
      lens,
      sortField,
      sortDirection,
      limit,
    };

    debounceTimerRef.current = window.setTimeout(() => {
      debounceTimerRef.current = null;
      if (generationRef.current !== generation) {
        return;
      }
      setIsDebouncing(false);
      setSections(loadingSections(lens));
      void startSearch(request).then((started) => {
        if (generationRef.current !== generation) {
          return;
        }
        activeSearchIdRef.current = started.searchId;
        setActiveSearchId(started.searchId);
      }).catch((searchError) => {
        if (generationRef.current !== generation) {
          return;
        }
        activeSearchIdRef.current = null;
        setActiveSearchId(null);
        setIsSearching(false);
        setError(errorMessage(searchError));
        setSections(failedSections(lens, errorMessage(searchError)));
      });
    }, 250);

    return () => {
      if (debounceTimerRef.current !== null) {
        window.clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
      if (activeSearchIdRef.current) {
        void Promise.resolve(cancelSearch()).catch(() => undefined);
      }
    };
  }, [lens, limit, query, retryNonce, sortDirection, sortField]);

  const cancel = useCallback(async () => {
    generationRef.current += 1;
    if (debounceTimerRef.current !== null) {
      window.clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
    const hasActiveSearch = activeSearchIdRef.current !== null;
    activeSearchIdRef.current = null;
    setActiveSearchId(null);
    setIsDebouncing(false);
    setIsSearching(false);
    if (hasActiveSearch) {
      await cancelSearch().catch((cancelError) => {
        setError(errorMessage(cancelError));
      });
    }
    if (query.trim()) {
      setSections(cancelledSections(lens));
    }
  }, [lens, query]);

  const clear = useCallback(async () => {
    await cancel();
    setSections({});
    setError(null);
  }, [cancel]);

  const retry = useCallback(() => {
    setRetryNonce((value) => value + 1);
  }, []);

  return {
    sections,
    activeSearchId,
    isSearching,
    isDebouncing,
    error,
    cancel,
    clear,
    retry,
  };
}
