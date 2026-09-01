import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";
import { SearchResultCard } from "./SearchResultCard";
import type { ProviderSearchSection as ProviderSearchSectionDto, ProviderStatus } from "../../types/domain";

interface ProviderSearchSectionProps {
  section: ProviderSearchSectionDto;
  status?: ProviderStatus;
  onRetry: () => void;
}

function stateLabel(section: ProviderSearchSectionDto): string {
  if (section.state === "loading") {
    return "Searching independently";
  }
  if (section.state === "ready") {
    return section.results.length > 0 ? `${section.results.length} result${section.results.length === 1 ? "" : "s"}` : "No matches";
  }
  if (section.state === "cancelled") {
    return "Cancelled";
  }
  if (section.error?.code === "disabled") {
    return "Disabled";
  }
  if (section.error?.code === "rate_limited") {
    return "Rate limited";
  }
  if (section.error?.code === "quota_exceeded") {
    return "Quota reached";
  }
  return "Unavailable";
}

function detailMessage(section: ProviderSearchSectionDto, status?: ProviderStatus): string {
  return section.error?.detail ?? status?.detail ?? "This provider did not return a result.";
}

export function ProviderSearchSection({ section, status, onRetry }: ProviderSearchSectionProps) {
  const hasResults = section.state === "ready" && section.results.length > 0;
  const canRetry = section.state === "failed" && section.error?.code !== "disabled";

  return (
    <article className={`provider-result-group provider-result-state-${section.state}`} data-provider={section.provider}>
      <div className="provider-result-header">
        <div><ProviderBadge kind={section.provider} /><strong>{status?.label ?? section.provider}</strong></div>
        <span>{stateLabel(section)}</span>
      </div>
      {section.state === "loading" ? (
        <div aria-live="polite" className="provider-result-empty provider-result-loading"><SpotIcon name="spark" size={18} /><span>Searching this source independently…</span></div>
      ) : hasResults ? (
        <div className="provider-result-list">{section.results.map((result) => <SearchResultCard key={`${result.providerItemId}-${result.originalRank}`} result={result} />)}</div>
      ) : section.state === "ready" ? (
        <div className="provider-result-empty"><SpotIcon name="search" size={18} /><span>No results for this source.</span></div>
      ) : (
        <div className="provider-result-empty provider-result-message" role={section.state === "failed" ? "alert" : undefined}>
          <SpotIcon name={section.state === "cancelled" ? "close" : section.error?.code === "disabled" ? "settings" : "alert"} size={18} />
          <span>{detailMessage(section, status)}</span>
          {canRetry ? <button className="button button-small" onClick={onRetry} type="button">Retry</button> : null}
        </div>
      )}
    </article>
  );
}
