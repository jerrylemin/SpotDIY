import { useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { ProviderSearchSection } from "../components/search/ProviderSearchSection";
import { SearchControls } from "../components/search/SearchControls";
import { SpotIcon } from "../components/icons/SpotIcon";
import { useAppStatus } from "../hooks/useAppStatus";
import { searchProviderOrder, useSearch } from "../hooks/useSearch";
import { providerLabel } from "../services/ipc";
import type { ProviderKind, ProviderSearchSection as ProviderSearchSectionDto, ProviderStatus, SearchLens, SearchSortDirection, SearchSortField } from "../types/domain";

function fallbackSection(provider: ProviderKind, status: ProviderStatus | undefined, busy: boolean): ProviderSearchSectionDto {
  if (busy) {
    return { provider, state: "loading", results: [], error: null };
  }
  const disabled = status?.runtimeStatus === "disabled";
  return {
    provider,
    state: "failed",
    results: [],
    error: {
      code: disabled ? "disabled" : "unavailable",
      detail: status?.detail ?? "This provider has no search response.",
      retryAfterSeconds: null,
    },
  };
}

function providerName(kind: ProviderKind): string {
  switch (kind) {
    case "local":
      return "Local library";
    case "youtube":
      return "YouTube";
    case "soundcloud":
      return "SoundCloud";
    case "spotify":
      return "Spotify catalog";
  }
}

export function SearchPage() {
  const [query, setQuery] = useState("");
  const [lens, setLens] = useState<SearchLens>("all");
  const [sortField, setSortField] = useState<SearchSortField>("relevance");
  const [sortDirection, setSortDirection] = useState<SearchSortDirection>("descending");
  const status = useAppStatus();
  const search = useSearch({ query, lens, sortField, sortDirection });
  const providerStatuses = useMemo(() => new Map((status.data?.providers ?? []).map((provider) => [provider.kind, provider])), [status.data?.providers]);
  const providers = searchProviderOrder(lens);
  const hasQuery = query.trim().length > 0;

  return (
    <div className="page-stack search-page">
      <section className="page-intro search-intro">
        <div><span className="eyebrow">GLOBAL SEARCH</span><h1>Find your next <em>listen.</em></h1><p>One query, independent source responses, and enough context to choose the right version.</p></div>
        <span className="command-hint"><SpotIcon name="command" size={15} /> <kbd>CTRL K</kbd> commands</span>
      </section>
      <SearchControls
        isSearching={search.isSearching}
        lens={lens}
        onCancel={() => void search.cancel()}
        onClear={() => {
          setQuery("");
          void search.clear();
        }}
        onLensChange={setLens}
        onQueryChange={setQuery}
        onSortDirectionChange={setSortDirection}
        onSortFieldChange={setSortField}
        query={query}
        sortDirection={sortDirection}
        sortField={sortField}
      />
      {hasQuery ? (
        <section className="search-results-area">
          <div className="section-heading">
            <div><span className="eyebrow">RESULTS FOR</span><h2>“{query.trim()}”</h2></div>
            <span className="section-note">{search.isDebouncing ? "Waiting 250 ms" : `Relevance · ${sortDirection === "descending" ? "descending" : "ascending"}`}</span>
          </div>
          {search.error ? <div className="search-global-error" role="alert"><SpotIcon name="alert" size={17} /><span>{search.error}</span><button className="button button-small" onClick={search.retry} type="button">Retry search</button></div> : null}
          <div className="provider-result-groups">
            {providers.map((provider) => (
              <ProviderSearchSection
                key={provider}
                onRetry={search.retry}
                section={search.sections[provider] ?? fallbackSection(provider, providerStatuses.get(provider), search.isSearching)}
                status={providerStatuses.get(provider) ?? {
                  kind: provider,
                  label: providerName(provider),
                  configured: false,
                  available: false,
                  runtimeStatus: "unknown",
                  capabilities: {
                    search: false,
                    playback: false,
                    metadata: false,
                    artwork: false,
                    lyrics: false,
                    downloads: false,
                    popularity: false,
                    releaseDate: false,
                    lyricsMetadata: false,
                  },
                  detail: `No ${providerLabel(provider)} search status is available.`,
                }}
              />
            ))}
          </div>
        </section>
      ) : (
        <EmptyState icon="search" eyebrow="READY WHEN YOU ARE" title="Search starts with a signal" description="Type above to search local tracks and configured sources. Results keep provider context visible so you can choose with confidence." action={<Link className="button button-quiet" to="/library">Set up local library <SpotIcon name="arrow" size={14} /></Link>} />
      )}
    </div>
  );
}
