import { Link } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { DownloadTaskRow } from "../components/downloads/DownloadTaskRow";
import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";
import {
  IpcError,
  isTauriRuntime,
  openDownloadLocation,
  pickDownloadDirectory,
  setSetting,
} from "../services/ipc";
import {
  DOWNLOAD_SNAPSHOT_QUERY_KEY,
  useCancelDownload,
  useDownloadSnapshot,
  useRetryDownload,
  useSetDownloadConcurrency,
} from "../hooks/useDownloads";
import type { DownloadTask, DownloadTaskId, DownloadToolStatus } from "../types/domain";

const activeStates: DownloadTask["state"][] = ["resolving", "downloading", "postprocessing"];

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

function toolLabel(status: DownloadToolStatus): string {
  switch (status.status) {
    case "ready":
      return "Ready";
    case "missing":
      return "Missing";
    case "broken":
      return "Broken";
    case "unsupported":
      return "Unsupported";
    case "disabled":
      return "Disabled";
    default:
      return "Unknown";
  }
}

function toolStatusClass(status: DownloadToolStatus): string {
  return `download-tool-status download-tool-status-${status.status}`;
}

export function DownloadsPage() {
  const nativeRuntime = isTauriRuntime();
  const queryClient = useQueryClient();
  const downloads = useDownloadSnapshot();
  const cancel = useCancelDownload();
  const retry = useRetryDownload();
  const concurrency = useSetDownloadConcurrency();
  const [filter, setFilter] = useState("");
  const [directoryBusy, setDirectoryBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const snapshot = downloads.data;
  const tasks = useMemo(() => {
    const query = filter.trim().toLowerCase();
    if (!snapshot || !query) {
      return snapshot?.tasks ?? [];
    }
    return snapshot.tasks.filter((task) => [
      task.title,
      ...task.artists,
      task.providerKind,
      task.mode,
      task.state,
    ].join(" ").toLowerCase().includes(query));
  }, [filter, snapshot]);
  const activeCount = snapshot?.tasks.filter((task) => activeStates.includes(task.state)).length ?? 0;
  const mutationBusy = cancel.isPending || retry.isPending || concurrency.isPending || directoryBusy;
  const visibleError = actionError
    ?? downloads.eventError
    ?? errorMessage(downloads.error, "The download service could not be read.");

  async function chooseDirectory() {
    if (!nativeRuntime || mutationBusy) {
      return;
    }
    setDirectoryBusy(true);
    setActionError(null);
    try {
      const directory = await pickDownloadDirectory();
      if (directory) {
        await setSetting({ key: "downloadsDirectory", value: directory });
        await queryClient.invalidateQueries({ queryKey: DOWNLOAD_SNAPSHOT_QUERY_KEY });
      }
    } catch (error) {
      setActionError(errorMessage(error, "SpotDIY could not set the download folder."));
    } finally {
      setDirectoryBusy(false);
    }
  }

  async function updateConcurrency(value: string) {
    setActionError(null);
    try {
      await concurrency.mutateAsync(Number(value));
    } catch (error) {
      setActionError(errorMessage(error, "SpotDIY could not update download concurrency."));
    }
  }

  function runTaskAction(action: () => Promise<unknown>, fallback: string) {
    setActionError(null);
    void action().catch((error: unknown) => {
      setActionError(errorMessage(error, fallback));
    });
  }

  function cancelTask(taskId: DownloadTaskId) {
    runTaskAction(() => cancel.mutateAsync(taskId), "SpotDIY could not cancel that download.");
  }

  function retryTask(taskId: DownloadTaskId) {
    runTaskAction(() => retry.mutateAsync(taskId), "SpotDIY could not retry that download.");
  }

  function openLocation(taskId: DownloadTaskId) {
    runTaskAction(() => openDownloadLocation(taskId), "SpotDIY could not open that download folder.");
  }

  return (
    <div className="page-stack downloads-page">
      <section className="page-intro">
        <div><span className="eyebrow">DOWNLOADS</span><h1>Offline, with <em>provenance.</em></h1><p>Persistent provider downloads stay visible from queue to finalized file.</p></div>
        <div className="page-intro-stat"><strong>{activeCount}</strong><span>active tasks</span></div>
      </section>

      {visibleError && (actionError || downloads.eventError || downloads.isError) ? <div className="library-alert library-alert-error" role="alert"><SpotIcon name="alert" size={16} /><span>{visibleError}</span></div> : null}

      {!nativeRuntime ? <div className="library-alert library-alert-warning" role="status"><SpotIcon name="settings" size={16} /><span>Downloads require the native SpotDIY desktop runtime. Browser preview shows the queue contract but cannot write files.</span></div> : null}

      <section className="downloads-toolbar" aria-label="Download settings">
        <div className="downloads-destination">
          <span className="eyebrow">DESTINATION</span>
          <strong>{snapshot?.downloadsDirectory ?? "Not configured"}</strong>
          <span>{snapshot?.downloadsDirectory ? "New tasks use this folder." : "Choose a folder before queuing a task."}</span>
        </div>
        <button className="button button-primary" disabled={!nativeRuntime || mutationBusy} onClick={() => void chooseDirectory()} type="button"><SpotIcon name="folder" size={15} /> Choose folder</button>
        <label className="downloads-concurrency"><span className="eyebrow">CONCURRENT TASKS</span><select aria-label="Maximum concurrent downloads" disabled={!nativeRuntime || mutationBusy} onChange={(event) => void updateConcurrency(event.target.value)} value={snapshot?.maxConcurrent ?? 2}><option value={1}>1 task</option><option value={2}>2 tasks</option><option value={3}>3 tasks</option><option value={4}>4 tasks</option></select></label>
      </section>

      <section className="downloads-tools" aria-labelledby="download-tools-heading">
        <div className="section-heading"><div><span className="eyebrow">MEDIA TOOLS</span><h2 id="download-tools-heading">Execution health</h2></div><span className="section-note">Validated at startup</span></div>
        <div className="download-tool-grid">
          {(["ytDlp", "ffmpeg"] as const).map((key) => {
            const status = snapshot?.tools[key] ?? { status: "unknown" as const, version: null, detail: null };
            const label = key === "ytDlp" ? "yt-dlp" : "FFmpeg";
            return <div className="download-tool-card" key={key}><div><strong>{label}</strong><span className={toolStatusClass(status)}>{toolLabel(status)}</span></div><span>{status.version ?? "Version unavailable"}</span><p>{status.detail ?? (status.status === "ready" ? "Ready for managed downloads." : "Tool health is not available yet.")}</p></div>;
          })}
        </div>
      </section>

      {downloads.isLoading ? <EmptyState icon="download" eyebrow="DOWNLOAD QUEUE" title="Loading downloads" description="Reading persistent task state from the local database…" /> : snapshot && snapshot.tasks.length === 0 ? <EmptyState icon="download" eyebrow="DOWNLOAD QUEUE EMPTY" title="Downloaded tracks appear here" description="Queue a YouTube or SoundCloud track from Search. The task and its provenance remain available across restarts." action={<Link className="button button-quiet" to="/search">Browse sources <SpotIcon name="arrow" size={14} /></Link>} /> : (
        <section className="downloads-list-section" aria-labelledby="downloads-list-heading">
          <div className="section-heading"><div><span className="eyebrow">TASK QUEUE</span><h2 id="downloads-list-heading">Managed downloads</h2></div><label className="downloads-filter"><SpotIcon name="search" size={14} /><input aria-label="Filter downloads" onChange={(event) => setFilter(event.target.value)} placeholder="Filter title, artist, provider…" value={filter} /></label></div>
          {tasks.length > 0 ? <div className="download-task-list">{tasks.map((task) => <DownloadTaskRow actionPending={mutationBusy} key={task.id} onCancel={cancelTask} onOpenLocation={openLocation} onRetry={retryTask} task={task} />)}</div> : <div className="library-pending-state"><SpotIcon name="search" size={18} /> No tasks match this filter.</div>}
        </section>
      )}

      <section className="download-state-legend"><span className="eyebrow">TASK STATES</span><div><span className="state-chip">Queued</span><span className="state-chip">Resolving</span><span className="state-chip">Downloading</span><span className="state-chip">Post-processing</span><span className="state-chip">Completed</span><span className="state-chip">Failed / cancelled</span></div></section>
    </div>
  );
}
