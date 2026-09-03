import { useEffect, useMemo, useRef, useState } from "react";

import { SpotIcon } from "../icons/SpotIcon";
import { usePlayback } from "../../hooks/usePlayback";
import { useSmartPlaylistActions, useSmartPlaylistPreview, useSmartPlaylists } from "../../hooks/useSmartPlaylists";
import { isTauriRuntime, IpcError } from "../../services/ipc";
import type { SmartField, SmartOperation, SmartPlaylist, SmartPlaylistInput, SmartRule, SmartValue } from "../../types/domain";

interface FieldSpec {
  value: SmartField;
  label: string;
  operations: SmartOperation[];
}

const fieldSpecs: FieldSpec[] = [
  { value: "artist", label: "Artist", operations: ["contains", "equals"] },
  { value: "album", label: "Album", operations: ["contains", "equals"] },
  { value: "genre", label: "Genre", operations: ["equals"] },
  { value: "year", label: "Release year", operations: ["equals", "between"] },
  { value: "dateAdded", label: "Date added", operations: ["before", "after", "between"] },
  { value: "lastPlayed", label: "Last played", operations: ["never", "before", "after", "between"] },
  { value: "playCount", label: "Qualified plays", operations: ["equals", "greaterThanOrEqual", "lessThanOrEqual"] },
  { value: "skipCount", label: "Skips", operations: ["equals", "greaterThanOrEqual", "lessThanOrEqual"] },
  { value: "rating", label: "Rating", operations: ["absent", "equals", "greaterThanOrEqual", "lessThanOrEqual"] },
  { value: "liked", label: "Liked", operations: ["true", "false"] },
  { value: "downloaded", label: "Downloaded", operations: ["true", "false"] },
  { value: "provider", label: "Provider", operations: ["has"] },
  { value: "audioQuality", label: "Audio quality", operations: ["is"] },
  { value: "duration", label: "Duration (ms)", operations: ["between"] },
  { value: "tag", label: "Tag", operations: ["has", "lacks"] },
];

const operationLabels: Record<SmartOperation, string> = {
  contains: "contains",
  equals: "is",
  before: "before",
  after: "after",
  between: "between",
  never: "never",
  greaterThanOrEqual: "at least",
  lessThanOrEqual: "at most",
  absent: "is not set",
  true: "is true",
  false: "is false",
  has: "has",
  lacks: "lacks",
  is: "is",
};

function defaultPredicate(field: SmartField = "liked", operation?: SmartOperation): Extract<SmartRule, { type: "predicate" }> {
  const nextOperation = operation ?? fieldSpecs.find((spec) => spec.value === field)?.operations[0] ?? "equals";
  return { type: "predicate", field, operation: nextOperation, value: defaultValue(field, nextOperation) };
}

function defaultRule(): SmartRule {
  return { type: "group", operator: "and", children: [defaultPredicate()] };
}

function isIntegerField(field: SmartField): boolean {
  return field === "playCount" || field === "skipCount" || field === "rating" || field === "duration";
}

function defaultValue(field: SmartField, operation: SmartOperation): SmartValue | null {
  if (["never", "absent", "true", "false"].includes(operation)) {
    return null;
  }
  if (operation === "between") {
    return { from: isIntegerField(field) ? 0 : "", to: isIntegerField(field) ? 0 : "" };
  }
  return isIntegerField(field) ? 0 : "";
}

function fieldSpec(field: SmartField): FieldSpec {
  return fieldSpecs.find((spec) => spec.value === field) ?? fieldSpecs[0];
}

function scalarText(value: string | number | boolean): string {
  return String(value);
}

function readValue(field: SmartField, operation: SmartOperation, raw: string): SmartValue {
  if (operation === "between") {
    const current = defaultValue(field, operation);
    if (typeof current === "object" && current !== null && "from" in current) {
      return { ...current, from: isIntegerField(field) ? Number(raw) : raw };
    }
  }
  return isIntegerField(field) ? Number(raw) : raw;
}

function updatePredicate(rule: SmartRule, index: number, field: SmartField, operation: SmartOperation, value?: SmartValue | null): SmartRule {
  if (rule.type !== "group") {
    return rule;
  }
  return {
    ...rule,
    children: rule.children.map((child, childIndex) => childIndex === index && child.type === "predicate"
      ? { ...child, field, operation, value: value === undefined ? defaultValue(field, operation) : value }
      : child),
  };
}

function errorMessage(error: unknown): string {
  if (error instanceof IpcError && error.message) return error.message;
  if (error instanceof Error && error.message) return error.message;
  return "SpotDIY could not update that smart playlist.";
}

function draftFromPlaylist(playlist: SmartPlaylist): SmartPlaylistInput {
  return {
    name: playlist.name,
    rule: playlist.rule,
    sortMode: playlist.sortMode,
    sortDirection: playlist.sortDirection,
    limitCount: playlist.limitCount,
  };
}

function ValueEditor({
  field,
  operation,
  value,
  onChange,
}: {
  field: SmartField;
  operation: SmartOperation;
  value: SmartValue | null;
  onChange: (value: SmartValue | null) => void;
}) {
  if (value === null) {
    return <span className="smart-rule-no-value">No value</span>;
  }
  if (typeof value === "object" && "from" in value) {
    return (
      <span className="smart-rule-range">
        <input aria-label="Smart rule range start" onChange={(event) => onChange({ from: isIntegerField(field) ? Number(event.target.value) : event.target.value, to: value.to })} placeholder="from" type={isIntegerField(field) ? "number" : field === "year" ? "text" : "date"} value={scalarText(value.from)} />
        <span>to</span>
        <input aria-label="Smart rule range end" onChange={(event) => onChange({ from: value.from, to: isIntegerField(field) ? Number(event.target.value) : event.target.value })} placeholder="to" type={isIntegerField(field) ? "number" : field === "year" ? "text" : "date"} value={scalarText(value.to)} />
      </span>
    );
  }
  return <input aria-label="Smart rule value" onChange={(event) => onChange(readValue(field, operation, event.target.value))} type={isIntegerField(field) ? "number" : field === "dateAdded" || field === "lastPlayed" ? "date" : "text"} value={scalarText(value)} />;
}

export function SmartPlaylistPanel() {
  const nativeRuntime = isTauriRuntime();
  const playlists = useSmartPlaylists();
  const actions = useSmartPlaylistActions();
  const playback = usePlayback();
  const [selectedId, setSelectedId] = useState<SmartPlaylist["id"] | null>(null);
  const [name, setName] = useState("New smart playlist");
  const [rule, setRule] = useState<SmartRule>(defaultRule);
  const [sortMode, setSortMode] = useState<SmartPlaylistInput["sortMode"]>("title");
  const [sortDirection, setSortDirection] = useState<SmartPlaylistInput["sortDirection"]>("asc");
  const [limitCount, setLimitCount] = useState<number | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const preview = useSmartPlaylistPreview(selectedId, 0, 20);
  const initializedSelection = useRef(false);

  useEffect(() => {
    if (!initializedSelection.current && selectedId === null && (playlists.data?.length ?? 0) > 0) {
      const first = playlists.data?.[0];
      if (first) {
        initializedSelection.current = true;
        setSelectedId(first.id);
        const draft = draftFromPlaylist(first);
        setName(draft.name);
        setRule(draft.rule);
        setSortMode(draft.sortMode);
        setSortDirection(draft.sortDirection);
        setLimitCount(draft.limitCount);
      }
    }
  }, [playlists.data, selectedId]);

  const predicates = useMemo(() => rule.type === "group" ? rule.children.filter((child): child is Extract<SmartRule, { type: "predicate" }> => child.type === "predicate") : [], [rule]);
  const pending = actions.create.isPending || actions.update.isPending || actions.remove.isPending || playback.pending;

  const selectPlaylist = (playlist: SmartPlaylist) => {
    setSelectedId(playlist.id);
    const draft = draftFromPlaylist(playlist);
    setName(draft.name);
    setRule(draft.rule);
    setSortMode(draft.sortMode);
    setSortDirection(draft.sortDirection);
    setLimitCount(draft.limitCount);
    setMessage(null);
  };

  const newPlaylist = () => {
    initializedSelection.current = true;
    setSelectedId(null);
    setName("New smart playlist");
    setRule(defaultRule());
    setSortMode("title");
    setSortDirection("asc");
    setLimitCount(null);
    setMessage(null);
  };

  const save = async () => {
    const input: SmartPlaylistInput = { name, rule, sortMode, sortDirection, limitCount };
    try {
      const saved = selectedId ? await actions.update.mutateAsync({ playlistId: selectedId, input }) : await actions.create.mutateAsync(input);
      selectPlaylist(saved);
      setMessage("Smart playlist saved. Preview stays live as the library changes.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  };

  const remove = async () => {
    if (!selectedId || !window.confirm(`Delete “${name}”?`)) return;
    try {
      await actions.remove.mutateAsync(selectedId);
      newPlaylist();
      setMessage("Smart playlist deleted.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  };

  const playMix = async () => {
    if (!selectedId) return;
    try {
      await playback.openSmartMix({ smartPlaylist: selectedId }, {
        familiarity: 50,
        variety: 70,
        freshness: 50,
        count: Math.min(limitCount ?? 25, 1000),
        recentTrackIds: playback.snapshot.currentTrackId ? [playback.snapshot.currentTrackId] : [],
      });
      setMessage("Smart mix opened in the queue. Playback will wait for your next action.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  };

  return (
    <section className="smart-playlists-panel">
      <div className="section-heading"><div><span className="eyebrow">SMART PLAYLISTS</span><h2>Rules that stay alive</h2></div><button className="button button-primary button-small" disabled={pending || !nativeRuntime} onClick={newPlaylist} type="button"><SpotIcon name="spark" size={14} /> New smart playlist</button></div>
      <p className="smart-panel-intro">A Dynamic playlist uses typed rules compiled into parameterized local SQL. It is a live view over your library, not a copied track list.</p>
      {!nativeRuntime ? <div className="queue-section-empty">Smart playlists are available in the native SpotDIY workspace.</div> : null}
      <div className="smart-playlists-layout">
        <aside className="smart-playlist-list" aria-label="Smart playlists">
          {playlists.data?.map((playlist) => <button className={`smart-playlist-list-item${playlist.id === selectedId ? " smart-playlist-list-item-active" : ""}`} key={playlist.id} onClick={() => selectPlaylist(playlist)} type="button"><SpotIcon name="spark" size={15} /><span>{playlist.name}</span></button>)}
          {playlists.data?.length === 0 ? <span className="queue-section-empty">No smart playlists yet.</span> : null}
        </aside>
        <div className="smart-playlist-editor">
          <div className="smart-playlist-editor-heading"><label><span>Name</span><input disabled={!nativeRuntime} maxLength={120} onChange={(event) => setName(event.target.value)} value={name} /></label><label><span>Sort</span><select aria-label="Smart playlist sort" disabled={!nativeRuntime} onChange={(event) => setSortMode(event.target.value as SmartPlaylistInput["sortMode"])} value={sortMode}><option value="title">Title</option><option value="artist">Artist</option><option value="dateAdded">Date added</option><option value="lastPlayed">Last played</option><option value="playCount">Play count</option><option value="rating">Rating</option><option value="duration">Duration</option><option value="audioQuality">Audio quality</option></select></label><label><span>Direction</span><select aria-label="Smart playlist sort direction" disabled={!nativeRuntime} onChange={(event) => setSortDirection(event.target.value as SmartPlaylistInput["sortDirection"])} value={sortDirection}><option value="asc">Ascending</option><option value="desc">Descending</option></select></label><label><span>Limit</span><input disabled={!nativeRuntime} min={1} max={5000} onChange={(event) => setLimitCount(event.target.value ? Number(event.target.value) : null)} placeholder="All" type="number" value={limitCount ?? ""} /></label></div>
          <div className="smart-rule-builder"><div className="smart-rule-builder-heading"><div><span className="eyebrow">RULE BUILDER</span><strong>{rule.type === "group" && rule.operator === "or" ? "Match any condition" : "Match all conditions"}</strong></div><label className="smart-rule-logic"><span>Logic</span><select aria-label="Smart rule logic" disabled={!nativeRuntime || rule.type !== "group"} onChange={(event) => setRule((current) => current.type === "group" ? { ...current, operator: event.target.value as "and" | "or" } : current)} value={rule.type === "group" ? rule.operator : "and"}><option value="and">AND</option><option value="or">OR</option></select></label><button className="button button-quiet button-small" disabled={!nativeRuntime || predicates.length >= 63} onClick={() => setRule((current) => current.type === "group" ? { ...current, children: [...current.children, defaultPredicate("artist", "contains")] } : current)} type="button">Add condition</button></div>{predicates.map((predicate, index) => { const spec = fieldSpec(predicate.field); return <div className="smart-rule-row" key={index}><select aria-label="Smart rule field" disabled={!nativeRuntime} onChange={(event) => { const field = event.target.value as SmartField; const operation = fieldSpec(field).operations[0]; setRule((current) => updatePredicate(current, index, field, operation)); }} value={predicate.field}>{fieldSpecs.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select><select aria-label="Smart rule operation" disabled={!nativeRuntime} onChange={(event) => setRule((current) => updatePredicate(current, index, predicate.field, event.target.value as SmartOperation))} value={predicate.operation}>{spec.operations.map((operation) => <option key={operation} value={operation}>{operationLabels[operation]}</option>)}</select><ValueEditor field={predicate.field} onChange={(value) => setRule((current) => updatePredicate(current, index, predicate.field, predicate.operation, value))} operation={predicate.operation} value={predicate.value} /><button aria-label="Remove smart rule" className="icon-button" disabled={!nativeRuntime || predicates.length <= 1} onClick={() => setRule((current) => current.type === "group" ? { ...current, children: current.children.filter((_, childIndex) => childIndex !== index) } : current)} type="button"><SpotIcon name="trash" size={14} /></button></div>; })}</div>
          <div className="smart-playlist-actions"><button className="button button-primary button-small" disabled={pending || !nativeRuntime} onClick={() => void save()} type="button"><SpotIcon name="check" size={14} /> Save rules</button><button className="button button-quiet button-small" disabled={!selectedId || pending || !nativeRuntime} onClick={() => void playMix()} type="button"><SpotIcon name="shuffle" size={14} /> Play Smart Mix</button><button className="button button-quiet button-small playlist-danger" disabled={!selectedId || pending || !nativeRuntime} onClick={() => void remove()} type="button"><SpotIcon name="trash" size={14} /> Delete</button></div>
          {message ? <div className="library-alert" role="status"><SpotIcon name="info" size={15} /><span>{message}</span></div> : null}
          <div className="smart-preview"><div className="section-heading"><div><span className="eyebrow">LIVE PREVIEW</span><h3>{preview.data?.total ?? 0} matching tracks</h3></div><span className="section-note">first 20</span></div>{preview.data?.items.map((track) => <div className="smart-preview-row" key={track.trackId}><div><strong>{track.title}</strong><span>{track.artists.join(" · ") || "Unknown artist"}{track.album ? ` · ${track.album}` : ""}</span></div><small>{track.playCount} plays</small></div>)}{selectedId && preview.data?.items.length === 0 ? <div className="queue-section-empty">No tracks match these rules yet.</div> : null}</div>
        </div>
      </div>
    </section>
  );
}
