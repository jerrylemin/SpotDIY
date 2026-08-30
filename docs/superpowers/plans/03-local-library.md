# Local Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Plan 03 as a durable, local-first Windows music library: selected folder roots persist, recursive indexing is incremental and restartable, metadata/artwork/quality are extracted safely, file changes are reconciled without identity churn, and the Library screen exposes real paged data and scan state.

**Architecture:** Rust owns path validation, Notify watchers, WalkDir traversal, Lofty parsing, streaming SHA-256, artwork cache writes, SQLite persistence, and trusted reveal operations. `LibraryService` coordinates one scan per folder and emits progress. Tauri commands expose only the folder, scan, page, and reveal contracts. React/TanStack Query consumes typed DTOs and scan events; the UI never invents tracks or reads arbitrary paths.

**Tech Stack:** Rust stable MSVC, Tauri 2, rusqlite/SQLite WAL, Lofty 0.25.1, Notify 8.2.0, WalkDir 2.5.0, sha2 0.11.0, tauri-plugin-dialog 2.7.2, tauri-plugin-opener 2.5.4, React 19, TanStack Query 5, Zod 4, Vitest.

**Spec:** `docs/superpowers/specs/2026-08-30-spotdiy-design.md`, Plan 03 sections in the supplied master execution prompt, ADRs 0003–0007, and the repository context documents.

## Global Constraints

- Preserve `0001_initial.sql`; add only migration `0002_local_library.sql`.
- Never use an absolute path as a track/source/provider item identity. Generate an opaque local provider item identity at first discovery and preserve it across rename/reconciliation.
- Normalize Windows paths with `Path`/`PathBuf`; reject invalid, unreadable, duplicate, nested, or containing roots with typed human-readable errors.
- Scan only `.mp3`, `.flac`, `.m4a`, `.aac`, `.ogg`, `.opus`, and `.wav`, case-insensitively. Do not traverse symlinks or junctions, and do not write/delete/rename user media.
- Keep expensive file I/O outside SQLite write transactions. Persist each track aggregate atomically. Do not add a fingerprint uniqueness constraint or claim losslessness from a codec name alone.
- Keep artwork app-owned and content-addressed under `%LOCALAPPDATA%\SpotDIY\cache\artwork`; validate input and write atomically. Do not put artwork blobs in page DTOs or expose selected music roots through the asset protocol.
- Keep IPC narrow and typed; do not expose raw SQL, arbitrary filesystem reads, arbitrary reveals, or generic command execution.
- Do not implement playback, provider adapters, Source Fusion, downloads, lyrics, full metadata editing, analytics, or full-text search.
- Preserve unrelated working-tree changes, do not commit until all gates pass, and do not push without explicit repository authorization.

---

## Phase 0: Repository, graph, and plan baseline

- [x] Read the active instruction chain and required context/spec/ADR documents.
- [x] Inspect the `main` worktree, preserve the user-owned partial changes, and establish the existing domain/database/IPC/UI seams and verification baseline.
- [x] Query CodeGraph and Graphify for repository/database/domain/UI relationships before editing.
- [x] Replace this file with this executable plan and keep checkboxes current.

> Execution note: this task resumed from a user-owned partial implementation, so a separate pre-implementation red-test transcript was not captured for every historical slice. The focused tests and final verification below are the authoritative green evidence.

## Phase 1: Domain and persistence seams

**Ownership:** `src-tauri/src/domain/mod.rs`, `src-tauri/src/db/mod.rs`, `src-tauri/migrations/0002_local_library.sql`, focused Rust tests.

- [x] Add typed folder/scan/library DTOs, scan counters, deterministic sort/page request types, and local artwork/path metadata without duplicating existing quality columns.
- [x] Change local provider identity generation so `TrackSource::new_local` never uses a path; preserve an explicit identity constructor for persisted/reconciled rows.
- [x] Add migration 0002 with `library_folders`, nullable legacy-compatible `local_files` folder/path/artwork columns, foreign keys, checks, and lookup indexes. Leave migration 0001 unchanged.
- [x] Add migration ordering/version tests and repository tests for folder persistence, identity preservation, duplicate bytes at distinct paths, unavailable/restore state, and paged deterministic reads.
- [x] Run the focused Rust tests and make the slice green; the historical red transcript was unavailable because the worktree entered with partial implementation.

## Phase 2: Path validation, fingerprints, metadata, and artwork

**Ownership:** `src-tauri/src/library/folders.rs`, `src-tauri/src/library/fingerprint.rs`, `src-tauri/src/library/metadata.rs`, `src-tauri/src/library/artwork.rs`, focused Rust tests.

- [x] Implement canonical absolute Windows path keys with root handling, case-insensitive duplicate/overlap checks, directory/readability validation, and safe display paths.
- [x] Implement streaming SHA-256 with bounded buffers and tests for equal/different content.
- [x] Implement Lofty extraction for title, multiple artists, album, duration, codec, bitrate, sample rate, bit depth, and first artwork; apply filename/Unknown Artist/absent-album fallbacks without comma splitting.
- [x] Implement conservative artwork signature/size validation, SHA-256 content-addressed cache keys, and atomic app-owned writes with separate failure reporting.
- [x] Run focused tests, formatter, and clippy for the changed Rust modules.

## Phase 3: Scanner and reconciliation

**Ownership:** `src-tauri/src/library/scanner.rs`, `src-tauri/src/library/mod.rs`, repository scan methods, scanner tests.

- [x] Implement recursive WalkDir discovery with no link traversal, case-insensitive supported-extension filtering, per-file error isolation, stat-based ordinary-rescan skipping, and forced watcher rescans.
- [x] Persist new/changed records transactionally after metadata/fingerprint/artwork work completes; update measured quality fields and source path without changing opaque IDs.
- [x] Reconcile observed files against folder records: mark missing sources unavailable with detail, reactivate restored paths, and pair unambiguous rename candidates by stable identity/fingerprint without guessing ambiguous matches.
- [x] Emit required `ScanSummary` counters and progress state, retain partial errors, support cancellation/restart persistence, and enforce one active scan per folder.
- [x] Run scanner integration tests using synthetic supported/unsupported/corrupt files and bounded large paging fixtures.

## Phase 4: Watcher and trusted native operations

**Ownership:** `src-tauri/src/library/watcher.rs`, `src-tauri/src/lib.rs`, Tauri capabilities/configuration, watcher/native-operation tests.

- [x] Register one recursive Notify watcher per enabled root, debounce 300–750 ms, coalesce duplicate events, pair rename events, and route watcher errors to reconciliation.
- [x] Register watchers promptly during startup and queue background scans; recover watcher/scan state after restart.
- [x] Add dialog and opener plugins/capabilities, scope any asset protocol only to the artwork cache, and implement reveal-after-ownership-validation through the official opener API.
- [x] Add the narrow folder/status/rescan/page/reveal Tauri command surface and `library://scan-progress` event contract.
- [x] Run watcher, command, capability, and native path-ownership tests.

## Phase 5: Frontend Library experience

**Ownership:** `src/types/domain.ts`, `src/services/ipc.ts`, new library hooks/components under `src/`, `src/pages/LibraryPage.tsx`, `src/styles/globals.css`, frontend tests.

- [x] Add Zod-validated library DTOs, query/mutation hooks, event subscription cleanup, and invalidation/refetch behavior.
- [x] Implement native multiple-folder selection, add/remove/rescan actions, folder status/count/last-scan/error display, and disabled/recovery states.
- [x] Implement real paged track display with deterministic sort/page controls, measured quality facts, original-file provenance, artwork or placeholder, unavailable detail, and trusted reveal actions.
- [x] Keep playback visibly disabled and truthful; handle no folders, pending, scanning, indexed, no supported tracks, partial errors, unavailable rows, and empty pages without fake data.
- [x] Add frontend unit/component tests in the Vitest suite; the Playwright runner gap is recorded in Task 6 and native smoke evidence is retained.

## Phase 6: Full verification, documentation, and handoff

- [x] Run `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build`.
- [x] Run `cargo fmt --all -- --check`, clippy for all targets/features with `-D warnings`, and cargo tests for all targets.
- [x] Run `git diff --check`, `git diff --cached --check` when staged, `pnpm tauri build`, packaged launch smoke, and a native synthetic-folder smoke test without committing media.
- [x] Run the permitted CodeGraph exploration and `graphify update .`; this workspace has no separate Graphify status command, and generated graph outputs remain excluded by policy.
- [x] Update `PROJECT_STATE.md`, `feature_progress.md`, `project_structure.md`, `session_handoff.md`, `ARCHITECTURE.md` if boundaries changed, `DECISION_LOG.md`, `TEST_MATRIX.md`, and the execution agent-ledger/integration-log/verification-log with exact evidence.
- [x] Update the required Obsidian notes when the implementation adds durable Plan 03 knowledge.
- [x] Review the final diff for secrets, personal absolute paths, database/cache/log/audio artifacts, unrelated edits, stale imports, dead code, and scope expansion.
- [ ] Commit as `feat: add incremental local library indexing` only after every required gate passes; push only if the authorized workflow permits it.

## Interfaces and acceptance checks

**Consumed:** existing `UnifiedTrack`, `TrackSource`, `LocalFileSource`, `Database`, settings, Tauri `AppState`, React query conventions, and established visual tokens.

**Produced:** `LibraryService`; folder CRUD; scan/reconciliation status and progress; deterministic paged library query; validated reveal operation; quality/provenance/artwork DTOs; migration 0002; native folder-picker and watcher integration.

**Acceptance:** A user can choose multiple valid roots, reopen the app and see persisted folders/indexed metadata, recursively discover supported files, skip unchanged files on ordinary rescans, detect changes/renames/removals without duplicate identity churn, recover from partial failures/restarts, inspect measured quality and provenance in a real paged Library UI, reveal only owned local files, and receive truthful empty/scanning/indexed/error states. No user media is modified.

**Commit boundary:** `feat: add incremental local library indexing`
---

## Executable task correction

The phase checklist above is the milestone map. This task contract is the
authoritative execution order for the current partial worktree. Every task
ends with a focused red/green check before the next task starts.

### Task 1: Domain and migration 2

Ownership: src-tauri/src/domain/mod.rs, src-tauri/src/db/mod.rs,
src-tauri/src/db/repository.rs, src-tauri/migrations/0002_local_library.sql,
and adjacent Rust tests.

Exact produced types:

~~~rust
pub enum LibraryFolderStatus { Idle, Queued, Scanning, Complete, Failed }
pub enum LocalFileIndexStatus { Pending, Indexed, Missing, Error }

pub struct LibraryPageRequest {
    pub page: u32,
    pub page_size: u32,
    pub sort: LibrarySort,
    pub descending: bool,
    pub folder_id: Option<LibraryFolderId>,
}

pub struct LibraryPage {
    pub items: Vec<LibraryTrack>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub has_next: bool,
    pub sort: LibrarySort,
    pub descending: bool,
}
~~~

Migration 2 creates library_folders with id, path, normalized_path_key,
enabled, scan status/generation, scan start/completion/error timestamps, and
created/updated timestamps. It extends local_files only with
library_folder_id, normalized_path_key, container, index_status,
status_detail, last_seen_at, last_indexed_at, last_seen_generation,
artwork_cache_key, and artwork_mime_type. It adds indexes for normalized path,
content fingerprint, folder/generation, folder page ordering, and global page
ordering. It never adds a fingerprint uniqueness constraint.

- [x] Add a migration 1 fixture test proving all eight Plan 02 tables and a
  representative track/source/local-file row survive migration 2.
- [x] Add tests proving schema version 2, library_folders, every extension
  column, foreign-key rejection, normalized-key uniqueness, and reopen
  idempotence.
- [x] Run
  cargo test --manifest-path src-tauri/Cargo.toml db::tests repository::tests
  after implementation; the historical pre-implementation red run was unavailable.
- [x] Implement migration ordering/version constants and explicit UUID,
  timestamp, integer, and enum parsing.
- [x] Rerun the focused command and verify the migration/repository assertions
  pass while migration 1 remains unchanged.

### Task 2: Folder paths, fingerprints, metadata, and artwork

Ownership: src-tauri/src/library/folders.rs,
src-tauri/src/library/fingerprint.rs, src-tauri/src/library/metadata.rs,
src-tauri/src/library/artwork.rs, and module-local tests.

Exact methods:

~~~rust
pub fn normalize_folder_path(
    input: impl AsRef<Path>,
) -> Result<NormalizedFolderPath, FolderPathError>;
pub fn normalize_file_path(
    input: impl AsRef<Path>,
) -> Result<(PathBuf, String), FolderPathError>;
pub fn validate_new_folders(
    inputs: impl IntoIterator<Item = PathBuf>,
    existing_keys: impl IntoIterator<Item = String>,
) -> Result<Vec<NormalizedFolderPath>, FolderPathError>;
pub fn sha256_file(path: impl AsRef<Path>) -> Result<String, FingerprintError>;
pub fn extract_metadata(
    path: impl AsRef<Path>,
) -> Result<ExtractedMetadata, MetadataError>;
pub struct ArtworkCacheEntry {
    pub cache_key: String,
    pub mime_type: String,
    pub byte_size: u64,
}
~~~

Folder normalization canonicalizes an existing readable directory, strips
display-only extended Windows prefixes, normalizes separators/case, preserves
roots, and compares segment boundaries. File normalization canonicalizes only
existing files. SHA-256 uses a fixed 64 KiB buffer and never calls
read_to_end. Lofty 0.25.1 supplies tags, ordered artist strings, properties,
file type, and pictures. A single artist string, including commas, remains one
artist. Artwork is capped at 10 MiB, signature-validated, SHA-256 addressed,
written to a temporary file, synced, and renamed into the app cache.

- [x] Add focused tests for missing/non-directory/unreadable/duplicate/case-only/
  nested/containing roots and the C:\music versus C:\music2 boundary.
- [x] Add focused tests for deterministic equal/different fingerprints and a
  source scan that does not use read_to_end.
- [x] Add focused tests for valid synthetic metadata, filename and Unknown Artist
  fallbacks, absent album, comma-bearing artist, corrupt input, artwork
  signatures, oversized artwork, cache deduplication, and cache-key traversal.
- [x] Run focused module tests and verify the implemented behavior passes for the
  expected assertions; the historical pre-implementation red run was unavailable.
- [x] Implement the smallest helpers and rerun
  cargo test --manifest-path src-tauri/Cargo.toml library::folders
  library::fingerprint library::metadata library::artwork.
- [x] Run cargo fmt --manifest-path src-tauri/Cargo.toml -- --check.

### Task 3: Scanner and transactional reconciliation

Ownership: src-tauri/src/library/scanner.rs,
src-tauri/src/library/mod.rs, scan persistence helpers, and library tests.

Exact service surface:

~~~rust
pub fn new(
    database: Database,
    artwork_cache_root: impl Into<PathBuf>,
) -> Result<LibraryService, LibraryError>;
pub fn list_folders(&self) -> Result<Vec<LibraryFolder>, LibraryError>;
pub fn add_folders(&self, paths: Vec<PathBuf>)
    -> Result<Vec<LibraryFolder>, LibraryError>;
pub fn remove_folder(&self, folder_id: LibraryFolderId)
    -> Result<(), LibraryError>;
pub fn status(&self) -> Result<LibraryStatus, LibraryError>;
pub fn page(&self, request: LibraryPageRequest)
    -> Result<LibraryPage, LibraryError>;
pub fn scan_folder_now(
    &self,
    folder_id: LibraryFolderId,
    force: bool,
    sink: Option<ProgressSink>,
) -> Result<ScanSummary, LibraryError>;
pub fn start_scan(
    &self,
    folder_id: LibraryFolderId,
    force: bool,
    sink: Option<ProgressSink>,
) -> Result<(), LibraryError>;
pub fn start_all_scans(
    &self,
    sink: Option<ProgressSink>,
) -> Result<(), LibraryError>;
~~~

WalkDir is recursive with follow_links(false). The scanner counts directories,
candidates, unsupported files, unchanged files, new/changed/renamed files,
missing files, metadata/artwork/database failures, and elapsed time. It
compares size and modified time before any metadata/hash/artwork work on an
ordinary scan. Forced scans always refresh a candidate. All changed aggregate
rows are written in one SQLite transaction after file I/O completes.

The scan fixture is a valid non-zero-frame WAV plus a deliberately malformed
supported-format file. The valid fixture remains available after indexing; the
malformed fixture is present as an error without stopping the valid track.
Missing paths become unavailable, a same-path restoration reuses its source, a
single missing fingerprint candidate is a rename, and multiple candidates are
ambiguous and receive a new identity. Identical files in different paths
remain two records.

- [x] Write focused integration tests for empty/nested folders, uppercase
  extensions, unsupported files, valid/corrupt files, multiple folders,
  ordinary unchanged skip, forced same-size modify, changed data, missing,
  restore, unambiguous rename, ambiguous rename, duplicate bytes, quality,
  artwork, and page ordering.
- [x] Run the focused test and resolve the current zero-length-WAV regression
  or missing implementation failure; the focused fixture is now green.
- [x] Implement the scanner, folder status persistence, observed-key
  reconciliation, identity-preserving upsert, and orphan-track cleanup.
- [x] Rerun the focused tests and verify the valid fixture is available, the
  corrupt fixture is an error, unchanged files skip work, and rename/missing/
  restore identities are stable.
- [x] Add a generated page fixture with at least 120 records and prove a
  50/100-entry page does not materialize the full dataset.

### Task 4: Watcher and startup recovery

Ownership: src-tauri/src/library/watcher.rs,
src-tauri/src/library/mod.rs, and watcher/startup tests.

Exact event seam:

~~~rust
pub enum WatchAction {
    Create(PathBuf),
    Modify(PathBuf),
    Remove(PathBuf),
    Rename { from: PathBuf, to: PathBuf },
    Reconcile,
    WatcherFailure,
}
pub const DEBOUNCE_WINDOW: Duration;
pub fn coalesce_events(
    events: &[notify::Result<Event>],
) -> Vec<WatchAction>;
~~~

One Notify 8.2.0 recursive watcher is registered per enabled root. Events
coalesce for 450 ms. Create, modify, and paired rename actions start one
forced scan for the folder. Remove and Reconcile start one ordinary
reconciliation scan. Rename Both and ordered From/To are paired; Any,
incomplete, directory-ambiguous, error, and overflow cases request
reconciliation. `WatcherFailure` triggers watcher re-registration; `Reconcile`
does not churn a healthy watcher. Unregister drops the watcher and its handler
cannot launch new work for a removed folder. A missing persisted root is marked
failed and does not abort startup.

- [x] Add focused tests for create/modify/remove, paired Both and From/To rename,
  duplicate modify collapse, unsupported-event routing, unpaired rename, and
  event error to Reconcile; the historical red baseline was unavailable.
- [x] Implement the event seam, watcher registry, debounced handler, and
  startup recovery.
- [x] Add a test that constructs a service with a missing persisted root and
  an interrupted scan, then confirms the folder remains stored as failed.
- [x] Run the focused watcher/startup tests and clippy with warnings denied.

### Task 5: Tauri command and security boundary

Ownership: src-tauri/src/lib.rs, src-tauri/src/ipc/mod.rs,
src-tauri/tauri.conf.json, and src-tauri/capabilities/default.json.

Exact commands:

~~~text
get_library_folders() -> Result<Vec<LibraryFolder>, String>
add_library_folders(paths: Vec<String>) -> Result<Vec<LibraryFolder>, String>
remove_library_folder(folder_id: LibraryFolderId) -> Result<(), String>
get_library_status() -> Result<LibraryStatus, String>
rescan_library_folder(folder_id: LibraryFolderId) -> Result<(), String>
rescan_all_library_folders() -> Result<(), String>
get_library_page(request: LibraryPageRequest) -> Result<LibraryPage, String>
reveal_local_file(source_id: SourceId) -> Result<(), String>
~~~

The add command persists validated paths, registers their watchers, and
starts initial scans in the background. The folder dialog runs in the
frontend with the official Tauri 2 dialog plugin and returns null/empty on
cancellation. Rust validates every received string. Reveal accepts only a
source ID, confirms the source is local and owned by a managed folder, and
uses the scoped opener reveal operation. The asset protocol scope contains
only the application artwork cache; selected music roots are not scoped.

- [x] Add focused command tests for add kickoff, empty selection no-op, invalid
  paths, status/page bounds, unknown/non-local reveal, and missing-root startup.
- [x] Implement command wiring, plugin initialization, capability permissions,
  and startup ordering: database, service, state management, watchers, scans.
- [x] Run cargo fmt and clippy with -D warnings.

### Task 6: Typed frontend IPC and Library UI

Ownership: src/types/domain.ts, src/services/ipc.ts,
src/hooks/useLibrary.ts, src/components/library/LibraryFolderRow.tsx,
src/components/library/LibraryTrackRow.tsx, src/pages/LibraryPage.tsx,
src/styles/globals.css, and frontend tests.

Exact frontend functions:

~~~typescript
export async function pickLibraryFolders(): Promise<string[]>;
export async function getLibraryFolders(): Promise<LibraryFolder[]>;
export async function addLibraryFolders(paths: string[]): Promise<LibraryFolder[]>;
export async function getLibraryStatus(): Promise<LibraryStatus>;
export async function getLibraryPage(request: LibraryPageRequest): Promise<LibraryPage>;
export async function rescanLibraryFolder(folderId: LibraryFolderId): Promise<void>;
export async function rescanAllLibraryFolders(): Promise<void>;
export async function removeLibraryFolder(folderId: LibraryFolderId): Promise<void>;
export async function revealLocalFile(sourceId: SourceId): Promise<void>;
export function parseScanProgress(value: unknown): ScanProgress;
~~~

The query keys are ['library-status'] and ['library-page', request]. The
progress listener parses library://scan-progress, invalidates both relevant
queries, stores transient progress, and always unregisters on unmount. The
dialog calls open with directory=true and multiple=true; null returns [].
Zod validates all response objects. Artwork conversion accepts only the
backend-provided app-cache path and returns null for missing art.

The page implements no-folder, pending/queued, scanning, indexed, no
supported tracks, unavailable, partial-error, and empty-page states. It
renders real paged local tracks with title, ordered artists, optional album,
duration, measured quality, Local badge, artwork or original placeholder,
availability/error detail, and Open file location. A non-empty removal asks
for confirmation and says music files remain untouched. Playback buttons are
disabled with a Plan 04 explanation.

- [x] Add focused IPC tests for dialog return shapes, cancellation, invoke names
  and payloads, Zod rejection, progress parsing, and listener cleanup.
- [x] Add focused UI tests for every state above, add/remove/rescan/reveal,
  pagination, quality rendering, fallback artwork, and disabled playback.
- [x] Run the focused Vitest files and verify the implemented behavior.
- [x] Implement the hooks, components, page, and styles.
- [x] Run pnpm test, pnpm typecheck, pnpm lint, and pnpm build.
- [x] Check for the mocked-IPC Playwright states; the repository has no browser
  test project, so the exact runner gap and native dialog/filesystem evidence are recorded separately.

### Task 7: Verification, review, documentation, and delivery

Ownership: required project memory, Obsidian notes, execution logs, plan
checkboxes, graph state, and final Git state.

- [x] Run the focused Plan 03 Rust tests separately and record the exact count.
- [x] Run all required frontend/Rust/Tauri checks from Phase 6 freshly after
  final edits, including pnpm tauri build and packaged launch smoke.
- [x] Run the synthetic-folder native sequence: launch, add, scan, restart,
  unchanged rescan, create, forced modify, rename identity, delete/missing,
  restore, reveal validation, remove folder, and confirm media remains.
- [x] Dispatch a separate read-only reviewer for the listed security,
  migration, watcher, identity, transaction, UI, dependency, and scope checks.
- [x] Validate the review against the actual diff and resolve every
  correctness-relevant Critical, High, or Medium finding.
- [x] Run CodeGraph synchronization/impact checks and graphify update .
  Record final graph node/edge counts and exclude heavy generated indexes.
- [x] Update PROJECT_STATE.md, feature_progress.md, project_structure.md,
  session_handoff.md, ARCHITECTURE.md, DECISION_LOG.md, TEST_MATRIX.md,
  docs/execution/agent-ledger.md, docs/execution/integration-log.md,
  docs/execution/verification-log.md, and the seven listed Obsidian notes with
  exact evidence and the next plan, Plan 04 Playback Engine.
- [x] Inspect staged changes for databases, WAL/SHM, cache/audio/log output,
  personal paths, and credential values before commit.
- [x] Commit the reviewed milestone and run git push origin main only after
  every acceptance condition is proven. Verify local/remote SHA equality and
  a clean worktree; the final delivery result records those checks.
