import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { LyricsPanel } from "../components/lyrics/LyricsPanel";
import { SpotIcon } from "../components/icons/SpotIcon";
import { formatLyricsOffset, MAX_LYRICS_OFFSET_MS, LYRICS_OFFSET_STEP_MS, useAbLoopPresets, useBookmarks, useLyrics, useLyricsOffset } from "../hooks/useLyrics";
import { usePlayback } from "../hooks/usePlayback";
import { usePlaybackClock } from "../hooks/usePlaybackClock";
import { IpcError } from "../services/ipc";
import type { BookmarkId, LyricsDocument, ManualLyricsMode } from "../types/domain";

function formatClock(positionMs: number): string {
  const totalSeconds = Math.floor(positionMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function formatLyricsTimestamp(positionMs: number): string {
  const minutes = Math.floor(positionMs / 60_000);
  const seconds = Math.floor(positionMs / 1_000) % 60;
  const milliseconds = positionMs % 1_000;
  const fraction = milliseconds === 0 ? "" : `.${String(milliseconds).padStart(3, "0").replace(/0+$/, "")}`;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}${fraction}`;
}

function formatEditableLyrics(document: LyricsDocument): string {
  if (document.syncKind !== "timed") {
    return document.plainText ?? "";
  }
  return document.cues.map((cue) => {
    if (cue.words.length > 0 && cue.lines.length === 1) {
      return `[${formatLyricsTimestamp(cue.startMs)}]${cue.words.map((word) => `<${formatLyricsTimestamp(word.startMs)}>${word.text}`).join(" ")}`;
    }
    return cue.lines.map((line) => `[${formatLyricsTimestamp(cue.startMs)}]${line}`).join("\n");
  }).join("\n");
}

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

function errorCode(error: unknown): string | null {
  if (!(error instanceof IpcError) || typeof error.cause !== "object" || error.cause === null) {
    return null;
  }
  const code = (error.cause as { code?: unknown }).code;
  return typeof code === "string" ? code : null;
}

function onlineErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "notFound":
      return "LRCLIB did not find a matching record.";
    case "rateLimited":
      return "LRCLIB is rate limiting requests. Try again later.";
    case "provider":
      return "LRCLIB could not be reached or returned an invalid response.";
    default:
      return errorMessage(error, "The online lyrics action failed.");
  }
}

function sourceLabel(source: LyricsDocument["source"]): string {
  switch (source) {
    case "manual":
      return "Manual override";
    case "sidecar":
      return "Local .lrc sidecar";
    case "embedded":
      return "Embedded metadata";
    case "lrclib":
      return "Cached LRCLIB";
  }
}

export function LyricsPage() {
  const playback = usePlayback();
  const visualPositionMs = usePlaybackClock(playback.snapshot);
  const trackId = playback.snapshot.currentTrackId;
  const sourceId = playback.snapshot.currentSourceId;
  const lyrics = useLyrics(trackId, sourceId);
  const lyricsOffset = useLyricsOffset(trackId);
  const bookmarks = useBookmarks(trackId);
  const presets = useAbLoopPresets(trackId);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorMode, setEditorMode] = useState<ManualLyricsMode>("plain");
  const [editorText, setEditorText] = useState("");
  const [bookmarkNote, setBookmarkNote] = useState("");
  const [editingBookmarkId, setEditingBookmarkId] = useState<BookmarkId | null>(null);
  const [editingBookmarkNote, setEditingBookmarkNote] = useState("");
  const [presetName, setPresetName] = useState("");
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const document = lyrics.data ?? null;
  const candidates = lyrics.searchOnline.data ?? [];
  const resetOnlineSearch = lyrics.searchOnline.reset;
  const busy = playback.pending
    || lyrics.saveManual.isPending
    || lyrics.removeManual.isPending
    || lyrics.importFile.isPending
    || lyrics.findBest.isPending
    || lyrics.searchOnline.isPending
    || lyrics.selectCandidate.isPending
    || lyrics.clearCache.isPending
    || bookmarks.create.isPending
    || bookmarks.update.isPending
    || bookmarks.remove.isPending
    || presets.save.isPending
    || presets.remove.isPending;

  useEffect(() => {
    setEditorOpen(false);
    setActionStatus(null);
    setActionError(null);
    resetOnlineSearch();
  }, [resetOnlineSearch, sourceId, trackId]);

  useEffect(() => {
    if (!editorOpen && document) {
      setEditorMode(document.syncKind === "timed" ? "lrc" : "plain");
      setEditorText(formatEditableLyrics(document));
    }
  }, [document, editorOpen]);

  if (trackId === null) {
    return (
      <div className="page-stack lyrics-page">
        <section className="page-intro"><div><span className="eyebrow">LYRICS & NOTES</span><h1>Stay with the <em>words.</em></h1><p>Read local lyrics in sync with playback, add study bookmarks, and keep A/B loops close to the music.</p></div></section>
        <EmptyState icon="lyrics" eyebrow="NO CURRENT TRACK" title="Choose a track first" description="Lyrics, bookmarks, and A/B controls become available when a track is playing or queued as the current selection." action={<Link className="button button-quiet" to="/library">Open your library <SpotIcon name="arrow" size={14} /></Link>} />
      </div>
    );
  }

  async function runAction(action: () => Promise<unknown>, success: string): Promise<boolean> {
    setActionError(null);
    setActionStatus(null);
    try {
      await action();
      setActionStatus(success);
      return true;
    } catch (error) {
      setActionError(errorMessage(error, "SpotDIY could not complete that action."));
      return false;
    }
  }

  function openEditor() {
    if (document) {
      setEditorMode(document.syncKind === "timed" ? "lrc" : "plain");
      setEditorText(formatEditableLyrics(document));
    }
    setEditorOpen(true);
    setActionError(null);
    setActionStatus(null);
  }

  async function saveEditor() {
    if (await runAction(
      () => lyrics.saveManual.mutateAsync({ mode: editorMode, text: editorText }),
      "Manual lyrics saved. Local source files were left unchanged.",
    )) {
      setEditorOpen(false);
    }
  }

  async function addBookmark() {
    if (await runAction(
      () => bookmarks.create.mutateAsync({ positionMs: Math.round(visualPositionMs), note: bookmarkNote }),
      "Bookmark added at the current playback position.",
    )) {
      setBookmarkNote("");
    }
  }

  async function saveBookmarkNote(bookmarkId: BookmarkId, positionMs: number) {
    if (await runAction(
      () => bookmarks.update.mutateAsync({ bookmarkId, positionMs, note: editingBookmarkNote }),
      "Bookmark note updated.",
    )) {
      setEditingBookmarkId(null);
    }
  }

  const lyricsError = lyrics.error ? errorMessage(lyrics.error, "Lyrics could not be loaded.") : null;
  const onlineError = lyrics.findBest.error
    ? onlineErrorMessage(lyrics.findBest.error)
    : lyrics.searchOnline.error
      ? onlineErrorMessage(lyrics.searchOnline.error)
      : lyrics.selectCandidate.error
        ? onlineErrorMessage(lyrics.selectCandidate.error)
        : null;

  return (
    <div className="page-stack lyrics-page">
      <section className="page-intro">
        <div><span className="eyebrow">LYRICS & NOTES</span><h1>{playback.snapshot.title ?? "Current track"} <em>in context.</em></h1><p>{playback.snapshot.artists.join(" · ") || "Unknown artist"}{playback.snapshot.album ? ` · ${playback.snapshot.album}` : ""} · position {formatClock(visualPositionMs)}</p></div>
        <div className="page-intro-stat"><strong>{bookmarks.data?.length ?? 0}</strong><span>Bookmarks</span></div>
      </section>

      <div className="lyrics-layout">
        <section className="lyrics-panel" aria-label="Lyrics">
          <div className="lyrics-panel-header">
            <div><span className="eyebrow">LYRICS</span><h2>{document ? sourceLabel(document.source) : "No lyrics loaded"}</h2></div>
            {document ? <div className="lyrics-source-meta"><span className={`lyrics-source-chip lyrics-source-${document.source}`}>{document.source}</span><span className="lyrics-sync-chip">{document.syncKind}</span></div> : null}
          </div>
          {document?.attribution ? <div className="lyrics-attribution">{document.attribution.label}{document.attribution.url ? <> · <a href={document.attribution.url} rel="noreferrer" target="_blank">Open provider</a></> : null}</div> : null}
          {lyrics.isLoading ? <div className="lyrics-empty-inline">Reading local lyrics sources…</div> : null}
          {lyricsError ? <div className="lyrics-action-error" role="alert">{lyricsError}</div> : null}
          {!lyrics.isLoading && !lyricsError && lyrics.findBest.isPending ? <div className="lyrics-empty-inline">Finding the closest LRCLIB lyrics match…</div> : null}
          {!lyrics.isLoading && !lyrics.findBest.isPending && !lyricsError && !document ? <div className="lyrics-instrumental">No lyrics are available for this track. You can retry the online lookup or import a local file.</div> : null}
          {document?.syncKind === "timed" ? (
            <div className="lyrics-sync-controls" aria-label="Lyrics synchronization">
              <span>Sync offset</span>
              <input
                aria-label="Lyrics sync offset in milliseconds"
                max={MAX_LYRICS_OFFSET_MS}
                min={-MAX_LYRICS_OFFSET_MS}
                onChange={(event) => lyricsOffset.setOffset(Number(event.target.value))}
                step={LYRICS_OFFSET_STEP_MS / 5}
                type="range"
                value={lyricsOffset.offsetMs}
              />
              <output>{formatLyricsOffset(lyricsOffset.offsetMs)}</output>
              <button className="button button-small button-quiet" disabled={lyricsOffset.offsetMs === 0} onClick={lyricsOffset.reset} type="button">Reset</button>
            </div>
          ) : null}
          {document ? <LyricsPanel document={document} durationMs={playback.snapshot.durationMs} lyricOffsetMs={lyricsOffset.offsetMs} onSeek={(positionMs) => { void playback.seekPlayback(positionMs); }} positionMs={visualPositionMs} /> : null}
          {editorOpen ? (
            <div className="lyrics-editor">
              <div className="lyrics-editor-heading"><strong>Edit a manual copy</strong><span>Source files and embedded tags remain read-only.</span></div>
              <label>Mode<select aria-label="Manual lyrics mode" onChange={(event) => setEditorMode(event.target.value as ManualLyricsMode)} value={editorMode}><option value="plain">Plain text</option><option value="lrc">Timed LRC</option></select></label>
              <textarea aria-label="Lyrics editor" onChange={(event) => setEditorText(event.target.value)} value={editorText} />
              <div className="lyrics-form-actions"><button className="button button-primary" disabled={busy || editorText.trim().length === 0} onClick={() => void saveEditor()} type="button">{lyrics.saveManual.isPending ? "Saving…" : "Save manual"}</button><button className="button button-quiet" disabled={busy} onClick={() => setEditorOpen(false)} type="button">Cancel</button></div>
            </div>
          ) : null}
          {actionStatus ? <div className="lyrics-action-status" role="status">{actionStatus}</div> : null}
          {actionError ? <div className="lyrics-action-error" role="alert">{actionError}</div> : null}
        </section>

        <div className="lyrics-tool-stack">
          <section className="lyrics-tools-card">
            <div className="lyrics-tool-heading"><div><span className="eyebrow">SOURCE ACTIONS</span><h3>Keep local first</h3></div><SpotIcon name="lyrics" size={20} /></div>
            <p className="lyrics-help">Sidecars and embedded tags stay first. If they are unavailable, SpotDIY automatically searches LRCLIB and syncs timed lyrics to playback.</p>
            <div className="lyrics-tool-actions"><button className="button button-primary" disabled={busy} onClick={() => void runAction(() => lyrics.findBest.mutateAsync(), "LRCLIB lyrics cached for this track.")} type="button">{lyrics.findBest.isPending ? "Finding…" : "Find online"}</button><button className="button button-quiet" disabled={busy} onClick={() => void runAction(() => lyrics.searchOnline.mutateAsync(), "LRCLIB candidates loaded.")} type="button">{lyrics.searchOnline.isPending ? "Searching…" : "Search online"}</button><button className="button button-quiet" disabled={busy} onClick={() => void runAction(() => lyrics.importFile.mutateAsync(), "Lyrics imported as a manual copy.")} type="button">{lyrics.importFile.isPending ? "Importing…" : "Import file"}</button></div>
            <div className="lyrics-tool-actions"><button className="button button-small" disabled={busy || !document} onClick={openEditor} type="button">Edit</button><button className="button button-small" disabled={busy || document?.source !== "manual"} onClick={() => void runAction(() => lyrics.removeManual.mutateAsync(), "Manual override deleted; normal local precedence is active again.")} type="button">Delete manual override</button><button className="button button-small" disabled={busy} onClick={() => void runAction(() => lyrics.clearCache.mutateAsync(), "Cached LRCLIB lyrics cleared.")} type="button">Clear LRCLIB cache</button></div>
            {onlineError ? <div className="lyrics-action-error" role="alert">{onlineError}</div> : null}
            {candidates.length > 0 ? <div className="lyrics-candidate-list"><span className="eyebrow">LRCLIB CANDIDATES</span>{candidates.map((candidate) => <div className="lyrics-bookmark-row" key={candidate.providerRecordId}><div className="lyrics-bookmark-main"><strong>{candidate.trackName}</strong><span>{candidate.artistName}{candidate.albumName ? ` · ${candidate.albumName}` : ""} · {candidate.hasSynced ? "timed" : candidate.hasPlain ? "plain" : "no text"}</span></div><button className="button button-small" disabled={busy || (!candidate.hasSynced && !candidate.hasPlain && !candidate.instrumental)} onClick={() => void runAction(() => lyrics.selectCandidate.mutateAsync(candidate.providerRecordId), "Selected LRCLIB lyrics cached for this track.")} type="button">Select</button></div>)}</div> : null}
          </section>

          <section className="lyrics-tools-card">
            <div className="lyrics-tool-heading"><div><span className="eyebrow">BOOKMARKS</span><h3>Mark the useful moments</h3></div><SpotIcon name="spark" size={20} /></div>
            <div className="lyrics-inline-form"><input aria-label="Bookmark note" maxLength={500} onChange={(event) => setBookmarkNote(event.target.value)} placeholder={`At ${formatClock(visualPositionMs)} · optional note`} value={bookmarkNote} /><button className="button button-small" disabled={busy} onClick={() => void addBookmark()} type="button">Add</button></div>
            {bookmarks.isLoading ? <div className="lyrics-empty-inline">Loading bookmarks…</div> : bookmarks.data && bookmarks.data.length > 0 ? <div className="lyrics-bookmark-list">{bookmarks.data.map((bookmark) => <div className="lyrics-bookmark-row" key={bookmark.id}><button className="lyrics-row-action" onClick={() => { void playback.seekPlayback(bookmark.positionMs); }} type="button">{formatClock(bookmark.positionMs)}</button><div className="lyrics-bookmark-main">{editingBookmarkId === bookmark.id ? <input aria-label={`Edit bookmark note at ${formatClock(bookmark.positionMs)}`} maxLength={500} onChange={(event) => setEditingBookmarkNote(event.target.value)} value={editingBookmarkNote} /> : <span>{bookmark.note || "No note"}</span>}</div><div className="lyrics-row-actions">{editingBookmarkId === bookmark.id ? <><button className="lyrics-row-action" onClick={() => void saveBookmarkNote(bookmark.id, bookmark.positionMs)} type="button">Save</button><button className="lyrics-row-action" onClick={() => setEditingBookmarkId(null)} type="button">Cancel</button></> : <button className="lyrics-row-action" onClick={() => { setEditingBookmarkId(bookmark.id); setEditingBookmarkNote(bookmark.note); }} type="button">Edit</button>}<button className="lyrics-row-action lyrics-row-action-danger" onClick={() => void runAction(() => bookmarks.remove.mutateAsync(bookmark.id), "Bookmark deleted.")} type="button">Delete</button></div></div>)}</div> : <div className="lyrics-empty-inline">No bookmarks yet.</div>}
          </section>

          <section className="lyrics-tools-card">
            <div className="lyrics-tool-heading"><div><span className="eyebrow">A/B LOOP</span><h3>Repeat a passage</h3></div><span className="lyrics-sync-chip">Playback-owned</span></div>
            <div className="lyrics-tool-actions"><button className="button button-small" disabled={busy} onClick={() => void runAction(() => playback.setAbLoopA(), "A loop point set.")} type="button">Set A</button><button className="button button-small" disabled={busy || playback.snapshot.abLoop.aMs === null} onClick={() => void runAction(() => playback.setAbLoopB(), "A/B loop activated.")} type="button">Set B</button><button className="button button-small" disabled={busy || (playback.snapshot.abLoop.aMs === null && playback.snapshot.abLoop.bMs === null)} onClick={() => void runAction(() => playback.clearAbLoop(), "A/B loop cleared.")} type="button">Clear A/B</button></div>
            <div className="lyrics-help">A: {playback.snapshot.abLoop.aMs === null ? "—" : formatClock(playback.snapshot.abLoop.aMs)} · B: {playback.snapshot.abLoop.bMs === null ? "—" : formatClock(playback.snapshot.abLoop.bMs)}{playback.snapshot.abLoop.active ? " · active" : ""}</div>
            <div className="lyrics-inline-form"><input aria-label="A/B preset name" maxLength={80} onChange={(event) => setPresetName(event.target.value)} placeholder="Preset name" value={presetName} /><button className="button button-small" disabled={busy || !playback.snapshot.abLoop.active || playback.snapshot.abLoop.aMs === null || playback.snapshot.abLoop.bMs === null || presetName.trim().length === 0} onClick={() => void runAction(async () => { await presets.save.mutateAsync({ name: presetName }); setPresetName(""); }, "A/B preset saved.")} type="button">Save preset</button></div>
            {presets.data && presets.data.length > 0 ? <div className="lyrics-preset-list">{presets.data.map((preset) => <div className="lyrics-preset-row" key={preset.id}><div className="lyrics-preset-main"><strong>{preset.name}</strong><span>{formatClock(preset.aMs)} → {formatClock(preset.bMs)}</span></div><div className="lyrics-row-actions"><button className="lyrics-row-action" disabled={busy} onClick={() => void runAction(() => playback.applyAbLoopPreset(preset.id), "A/B preset applied without starting playback.")} type="button">Apply</button><button className="lyrics-row-action lyrics-row-action-danger" disabled={busy} onClick={() => void runAction(() => presets.remove.mutateAsync(preset.id), "A/B preset deleted.")} type="button">Delete</button></div></div>)}</div> : <div className="lyrics-empty-inline">No saved A/B presets.</div>}
          </section>
        </div>
      </div>
    </div>
  );
}
