import { useMemo, useState } from "react";

import type { DownloadReadiness } from "../features/actions/track-actions";
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
      return "Spotify";
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
  const downloadReadiness: DownloadReadiness | undefined = status.data ? {
    ytDlpStatus: status.data.mediaTools.ytDlp.status,
    ffmpegStatus: status.data.mediaTools.ffmpeg.status,
    downloadDirectoryStatus: status.data.downloadDirectoryStatus,
    mpvStatus: status.data.mediaTools.mpv.status,
    spotifyStatus: providerStatuses.get("spotify")?.runtimeStatus,
  } : undefined;
  const providers = searchProviderOrder(lens);
  const hasQuery = query.trim().length > 0;

  return (
    <div className="page-stack search-page">
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
          {search.error ? <div className="search-global-error" role="alert"><SpotIcon name="alert" size={17} /><span>{search.error}</span><button aria-label="Retry search" className="button button-small icon-only-button" onClick={search.retry} title="Retry search" type="button"><SpotIcon name="refresh" size={14} /></button></div> : null}
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
                downloadReadiness={downloadReadiness}
              />
            ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}
