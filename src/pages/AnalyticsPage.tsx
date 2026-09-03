import { useState } from "react";

import { AnalyticsHeatmap } from "../components/analytics/AnalyticsHeatmap";
import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";
import {
  useAnalyticsActions,
  useAnalyticsOverview,
  useListeningHeatmap,
  useListeningSessionHistory,
  useListeningSessions,
  useTasteTimeline,
  useTimeMachineDay,
  useTopArtists,
  useTopTracks,
} from "../hooks/useAnalytics";
import { useListeningModes } from "../hooks/useListeningModes";
import { IpcError } from "../services/ipc";
import type { HistoryEntry, ListeningSession, ListeningSessionId } from "../types/domain";

function formatDuration(milliseconds: number): string {
  const totalMinutes = Math.floor(milliseconds / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function localToday(): string {
  const date = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
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

function outcomeLabel(entry: HistoryEntry): string {
  return entry.qualifiedPlay ? "Qualified play" : entry.outcome;
}

function HistoryRows({ entries }: { entries: HistoryEntry[] }) {
  return (
    <div className="analytics-history-list">
      {entries.map((entry) => (
        <div className="analytics-history-row" key={entry.id}>
          <div className="analytics-history-main">
            <strong>{entry.titleSnapshot}</strong>
            <span>{entry.artists.join(" · ") || "Unknown artist"}{entry.albumSnapshot ? ` · ${entry.albumSnapshot}` : ""}</span>
          </div>
          <span className={`analytics-outcome analytics-outcome-${entry.outcome}`}>{outcomeLabel(entry)}</span>
          <span className="analytics-history-time">{formatDuration(entry.listenedMs)}</span>
          <time dateTime={entry.startedAt}>{formatDate(entry.startedAt)}</time>
        </div>
      ))}
    </div>
  );
}

function SessionRow({
  session,
  selected,
  onSelect,
  onReopen,
  onLabel,
  pending,
}: {
  session: ListeningSession;
  selected: boolean;
  onSelect: () => void;
  onReopen: () => void;
  onLabel: () => void;
  pending: boolean;
}) {
  return (
    <div className={`analytics-session-row${selected ? " analytics-session-row-selected" : ""}`}>
      <button className="analytics-session-select" onClick={onSelect} type="button">
        <strong>{session.label ?? "Untitled listening session"}</strong>
        <span>{formatDate(session.startedAt)} · {session.eventCount} event{session.eventCount === 1 ? "" : "s"} · {formatDuration(session.listenedMs)}</span>
      </button>
      <div className="analytics-row-actions">
        <button className="button button-quiet button-small" disabled={pending} onClick={onLabel} type="button"><SpotIcon name="edit" size={13} /> Label</button>
        <button className="button button-quiet button-small" disabled={pending} onClick={onReopen} type="button"><SpotIcon name="queue" size={13} /> Reopen</button>
      </div>
    </div>
  );
}

export function AnalyticsPage() {
  const overview = useAnalyticsOverview();
  const heatmap = useListeningHeatmap();
  const topTracks = useTopTracks(10);
  const topArtists = useTopArtists(10);
  const timeline = useTasteTimeline();
  const sessions = useListeningSessions(0, 20);
  const modes = useListeningModes();
  const actions = useAnalyticsActions();
  const [selectedSessionId, setSelectedSessionId] = useState<ListeningSessionId | null>(null);
  const [selectedDate, setSelectedDate] = useState(localToday);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const sessionHistory = useListeningSessionHistory(selectedSessionId);
  const dayHistory = useTimeMachineDay(selectedDate);
  const data = overview.data;
  const hasHistory = Boolean(data && (data.listenedMs > 0 || data.sessionCount > 0 || data.qualifiedPlays > 0 || data.skips > 0));
  const selectedSession = sessions.data?.items.find((session) => session.id === selectedSessionId) ?? null;
  const mode = modes.state.data ?? { privateSession: false, temporary: false };

  const runAction = async (action: () => Promise<unknown>, success: string) => {
    setActionMessage(null);
    try {
      const result = await action();
      const dropped = typeof result === "object" && result !== null && "droppedCount" in result && typeof result.droppedCount === "number" && result.droppedCount > 0 ? ` ${result.droppedCount} unavailable track${result.droppedCount === 1 ? "" : "s"} dropped.` : "";
      setActionMessage(`${success}${dropped}`);
    } catch (error) {
      setActionMessage(errorMessage(error, "SpotDIY could not complete that analytics action."));
    }
  };

  const togglePrivate = () => {
    if (mode.temporary) return;
    void runAction(() => modes.privateSession.mutateAsync(!mode.privateSession), mode.privateSession ? "Private Session disabled." : "Private Session enabled.");
  };

  const toggleTemporary = () => {
    void runAction(
      () => mode.temporary ? modes.temporaryExit.mutateAsync() : modes.temporaryEnter.mutateAsync(),
      mode.temporary ? "Temporary Listening ended." : "Temporary Listening started.",
    );
  };

  return (
    <div className="page-stack analytics-page">
      <section className="page-intro">
        <div><span className="eyebrow">LOCAL ANALYTICS</span><h1>Notice your <em>listening.</em></h1><p>History, sessions, and taste patterns stay in SpotDIY’s local SQLite database.</p></div>
        <div className="analytics-mode-actions">
          <button className={`button ${mode.privateSession ? "button-primary" : "button-quiet"}`} disabled={mode.temporary || modes.privateSession.isPending} onClick={togglePrivate} type="button"><SpotIcon name="info" size={15} /> {mode.privateSession ? "Private Session on" : "Private Session off"}</button>
          <button className={`button ${mode.temporary ? "button-primary" : "button-quiet"}`} disabled={modes.temporaryEnter.isPending || modes.temporaryExit.isPending} onClick={toggleTemporary} type="button"><SpotIcon name="spark" size={15} /> {mode.temporary ? "Exit Temporary" : "Temporary Listening"}</button>
        </div>
      </section>

      <section className="analytics-mode-card" aria-label="Listening privacy modes">
        <strong>{mode.temporary ? "Temporary Listening is active." : mode.privateSession ? "Private Session is active." : "Listening privacy controls"}</strong>
        <p>Private Session hides listening activity from SpotDIY local history. It does not disable explicit library changes.</p>
        {mode.temporary ? <p>Queue and listening activity in Temporary Mode are discarded when the mode ends or SpotDIY restarts.</p> : null}
      </section>

      {actionMessage ? <div className="library-alert" role="status"><SpotIcon name="check" size={16} /><span>{actionMessage}</span></div> : null}
      {overview.error ? <div className="library-alert library-alert-error" role="alert"><SpotIcon name="alert" size={16} /><span>{errorMessage(overview.error, "SpotDIY could not read local analytics.")}</span></div> : null}

      {!data || overview.isLoading ? <div className="library-pending-state" role="status"><SpotIcon name="spark" size={18} /> Reading local analytics…</div> : null}
      {data && !hasHistory ? <EmptyState icon="analytics" eyebrow="LOCAL ANALYTICS" title="No listening history yet." description="Start a local or connected track to build your first private listening timeline. Private Session and Temporary Listening never write history." /> : null}

      {data && hasHistory ? (
        <>
          <section className="analytics-overview-grid" aria-label="Listening overview">
            <div className="analytics-stat-card"><span className="eyebrow">LISTENED</span><strong>{formatDuration(data.listenedMs)}</strong><span>qualified and unqualified listening</span></div>
            <div className="analytics-stat-card"><span className="eyebrow">QUALIFIED PLAYS</span><strong>{data.qualifiedPlays}</strong><span>plays above the local threshold</span></div>
            <div className="analytics-stat-card"><span className="eyebrow">UNIQUE TRACKS</span><strong>{data.uniqueTracks}</strong><span>tracks in recorded history</span></div>
            <div className="analytics-stat-card"><span className="eyebrow">SESSIONS</span><strong>{data.sessionCount}</strong><span>{data.uniqueArtists} artists represented</span></div>
          </section>

          <div className="analytics-dashboard-grid">
            <section className="analytics-panel analytics-panel-wide"><div className="section-heading"><div><span className="eyebrow">WHEN YOU LISTEN</span><h2>Weekly rhythm</h2></div><span className="section-note">local time</span></div>{heatmap.data ? <AnalyticsHeatmap cells={heatmap.data} /> : <div className="queue-section-empty">Heatmap unavailable.</div>}</section>
            <section className="analytics-panel"><div className="section-heading"><div><span className="eyebrow">TOP TRACKS</span><h2>On repeat</h2></div></div><div className="analytics-ranking-list">{topTracks.data?.map((track, index) => <div className="analytics-ranking-row" key={`${track.trackId ?? track.title}-${index}`}><span className="analytics-rank">{String(index + 1).padStart(2, "0")}</span><div><strong>{track.title}</strong><span>{track.artists.join(" · ") || "Unknown artist"}</span></div><small>{formatDuration(track.listenedMs)}</small></div>)}</div></section>
            <section className="analytics-panel"><div className="section-heading"><div><span className="eyebrow">TOP ARTISTS</span><h2>Your orbit</h2></div></div><div className="analytics-ranking-list">{topArtists.data?.map((artist, index) => <div className="analytics-ranking-row" key={`${artist.name}-${index}`}><span className="analytics-rank">{String(index + 1).padStart(2, "0")}</span><div><strong>{artist.name}</strong><span>{artist.qualifiedPlays} qualified plays</span></div><small>{formatDuration(artist.listenedMs)}</small></div>)}</div></section>
          </div>

          <section className="analytics-panel"><div className="section-heading"><div><span className="eyebrow">TASTE TIMELINE</span><h2>How the months moved</h2></div><span className="section-note">last 36 months</span></div><div className="analytics-timeline">{timeline.data?.map((month) => <div className="analytics-timeline-row" key={month.month}><strong>{month.month}</strong><span className="analytics-timeline-bar"><span style={{ width: `${Math.min(100, Math.max(4, month.listenedMs / Math.max(...(timeline.data ?? []).map((item) => item.listenedMs), 1) * 100))}%` }} /></span><span>{formatDuration(month.listenedMs)}</span><small>{month.topArtists.slice(0, 2).join(" · ") || "No artist snapshot"}</small></div>)}</div></section>

          <section className="analytics-panel"><div className="section-heading"><div><span className="eyebrow">LISTENING SESSIONS</span><h2>Return to a context</h2></div><span className="section-note">30-minute grouping</span></div><div className="analytics-session-list">{sessions.data?.items.map((session) => <SessionRow key={session.id} pending={actions.reopenSession.isPending || actions.labelSession.isPending} selected={session.id === selectedSessionId} session={session} onSelect={() => setSelectedSessionId(session.id)} onReopen={() => void runAction(() => actions.reopenSession.mutateAsync(session.id), "Session reopened as a queue.")} onLabel={() => { const label = window.prompt("Label this listening session:", session.label ?? ""); if (label !== null) void runAction(() => actions.labelSession.mutateAsync({ sessionId: session.id, label: label.trim() || null }), "Session label updated."); }} />)}</div>{sessions.data?.items.length === 0 ? <div className="queue-section-empty">No sessions are available.</div> : null}</section>

          {selectedSession ? <section className="analytics-panel"><div className="section-heading"><div><span className="eyebrow">SESSION DETAIL</span><h2>{selectedSession.label ?? "Untitled listening session"}</h2></div><span className="section-note">{formatDate(selectedSession.startedAt)} → {formatDate(selectedSession.endedAt)}</span></div>{sessionHistory.data ? <HistoryRows entries={sessionHistory.data} /> : <div className="library-pending-state">Loading session history…</div>}<button className="button button-primary button-small" disabled={actions.reopenSession.isPending} onClick={() => void runAction(() => actions.reopenSession.mutateAsync(selectedSession.id), "Session reopened as a queue.")} type="button"><SpotIcon name="queue" size={14} /> Reopen this session</button></section> : null}

          <section className="analytics-panel"><div className="section-heading"><div><span className="eyebrow">TIME MACHINE</span><h2>What played on a day?</h2></div><span className="section-note">chronological history</span></div><div className="analytics-time-machine-controls"><label htmlFor="analytics-day">Local date</label><input id="analytics-day" max={localToday()} onChange={(event) => setSelectedDate(event.target.value)} type="date" value={selectedDate} /><button className="button button-quiet button-small" disabled={actions.reopenDay.isPending || !dayHistory.data?.length} onClick={() => void runAction(() => actions.reopenDay.mutateAsync(selectedDate), "Day reopened as a queue.")} type="button"><SpotIcon name="queue" size={14} /> Reopen day</button></div>{dayHistory.data && dayHistory.data.length > 0 ? <HistoryRows entries={dayHistory.data} /> : <div className="queue-section-empty">No listening history for this day.</div>}</section>
        </>
      ) : null}
    </div>
  );
}
