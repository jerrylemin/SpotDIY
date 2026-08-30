import { useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { ProviderBadge } from "../components/common/ProviderBadge";
import { SpotIcon } from "../components/icons/SpotIcon";
import { useAppStatus } from "../hooks/useAppStatus";

const lenses = ["ALL", "TRACKS", "ARTISTS", "ALBUMS", "LOCAL", "YOUTUBE", "SOUNDCLOUD", "SPOTIFY"] as const;

export function SearchPage() {
  const [query, setQuery] = useState("");
  const [lens, setLens] = useState<(typeof lenses)[number]>("ALL");
  const status = useAppStatus();
  const matchingProviders = useMemo(() => status.data?.providers.filter((provider) => lens === "ALL" || lens === provider.kind.toUpperCase()) ?? [], [lens, status.data?.providers]);

  return (
    <div className="page-stack search-page">
      <section className="page-intro search-intro"><div><span className="eyebrow">GLOBAL SEARCH</span><h1>Find your next <em>listen.</em></h1><p>One query, independent source responses, and enough context to choose the right version.</p></div><span className="command-hint"><SpotIcon name="command" size={15} /> <kbd>CTRL K</kbd> commands</span></section>
      <div className="search-field-large"><SpotIcon name="search" size={22} /><input aria-label="Search music" autoFocus onChange={(event) => setQuery(event.target.value)} placeholder="Try an artist, track, album, or mood" value={query} /><span className="search-field-clear">{query ? "CLEAR" : "⌘ /"}</span></div>
      <div className="lens-row" role="tablist" aria-label="Search lenses">{lenses.map((item) => <button aria-selected={lens === item} className={`lens-button${lens === item ? " lens-button-active" : ""}`} key={item} onClick={() => setLens(item)} role="tab" type="button">{item}</button>)}</div>
      {query ? <section className="search-results-area"><div className="section-heading"><div><span className="eyebrow">RESULTS FOR</span><h2>“{query}”</h2></div><span className="section-note">Relevance · descending</span></div><div className="provider-result-groups">{matchingProviders.map((provider) => <article className="provider-result-group" key={provider.kind}><div className="provider-result-header"><div><ProviderBadge kind={provider.kind} /><strong>{provider.label}</strong></div><span>{provider.available ? provider.capabilities.search ? "Searching independently" : "Search unavailable" : "Setup required"}</span></div><div className="provider-result-empty"><SpotIcon name={provider.available && provider.capabilities.search ? "spark" : "settings"} size={18} /><span>{provider.available && provider.capabilities.search ? "No results are loaded yet — local indexing and provider adapters are the next foundation slice." : provider.detail}</span></div></article>)}</div></section> : <EmptyState icon="search" eyebrow="READY WHEN YOU ARE" title="Search starts with a signal" description="Type above to search local tracks and configured sources. Results keep provider context visible so you can choose with confidence." action={<Link className="button button-quiet" to="/library">Set up local library <SpotIcon name="arrow" size={14} /></Link>} />}
    </div>
  );
}
