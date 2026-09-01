import { SpotIcon } from "../icons/SpotIcon";
import type { SearchLens, SearchSortDirection, SearchSortField } from "../../types/domain";

const SEARCH_LENSES: Array<{ value: SearchLens; label: string }> = [
  { value: "all", label: "ALL" },
  { value: "tracks", label: "TRACKS" },
  { value: "artists", label: "ARTISTS" },
  { value: "albums", label: "ALBUMS" },
  { value: "local", label: "LOCAL" },
  { value: "youtube", label: "YOUTUBE" },
  { value: "soundcloud", label: "SOUNDCLOUD" },
  { value: "spotify", label: "SPOTIFY" },
];

interface SearchControlsProps {
  query: string;
  lens: SearchLens;
  sortField: SearchSortField;
  sortDirection: SearchSortDirection;
  isSearching: boolean;
  onQueryChange: (query: string) => void;
  onLensChange: (lens: SearchLens) => void;
  onSortFieldChange: (field: SearchSortField) => void;
  onSortDirectionChange: (direction: SearchSortDirection) => void;
  onClear: () => void;
  onCancel: () => void;
}

const sortFields: Array<{ value: SearchSortField; label: string }> = [
  { value: "relevance", label: "Relevance" },
  { value: "popularity", label: "Popularity" },
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "duration", label: "Duration" },
];

export function SearchControls({
  query,
  lens,
  sortField,
  sortDirection,
  isSearching,
  onQueryChange,
  onLensChange,
  onSortFieldChange,
  onSortDirectionChange,
  onClear,
  onCancel,
}: SearchControlsProps) {
  return (
    <div className="search-controls">
      <div className="search-field-large">
        <SpotIcon name="search" size={22} />
        <input
          aria-label="Search music"
          autoFocus
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Try an artist, track, album, or mood"
          value={query}
        />
        {query ? <button aria-label="Clear search" className="search-field-clear search-field-clear-button" onClick={onClear} type="button">CLEAR</button> : <span className="search-field-clear">⌘ /</span>}
      </div>
      <div aria-label="Search lenses" className="lens-row" role="tablist">
        {SEARCH_LENSES.map((item) => (
          <button
            aria-selected={lens === item.value}
            className={`lens-button${lens === item.value ? " lens-button-active" : ""}`}
            key={item.value}
            onClick={() => onLensChange(item.value)}
            role="tab"
            type="button"
          >
            {item.label}
          </button>
        ))}
      </div>
      <div className="search-sort-row">
        <label>
          <span>Sort</span>
          <select aria-label="Sort search results" onChange={(event) => onSortFieldChange(event.target.value as SearchSortField)} value={sortField}>
            {sortFields.map((field) => <option key={field.value} value={field.value}>{field.label}</option>)}
          </select>
        </label>
        <label>
          <span>Direction</span>
          <select aria-label="Sort direction" onChange={(event) => onSortDirectionChange(event.target.value as SearchSortDirection)} value={sortDirection}>
            <option value="descending">Descending</option>
            <option value="ascending">Ascending</option>
          </select>
        </label>
        {isSearching ? <button className="button button-quiet search-cancel-button" onClick={onCancel} type="button">Cancel search</button> : null}
      </div>
    </div>
  );
}
