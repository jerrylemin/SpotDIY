# SpotDIY Codex context

SpotDIY is a Windows-first, local-first music player. There is no SpotDIY account. The approved design source is `docs/superpowers/specs/2026-08-30-spotdiy-design.md`; plans live in `docs/superpowers/plans/`.

## Non-negotiable invariants

- A musical work is a `UnifiedTrack`; provider records are `TrackSource` values, not duplicate permanent songs.
- Provider adapters expose capabilities. UI code must not infer capability from provider-name strings.
- Spotify is catalog metadata only. Never implement Spotify audio ripping or DRM circumvention.
- Never label lossy-origin audio as lossless merely because it was transcoded to FLAC.
- No hardcoded secrets. Secure credentials belong in Windows Credential Manager through a maintained Rust keyring integration.
- Core user data is local and useful offline. Online sources augment local behavior.
- Imported archives, filesystem paths, provider responses, and media-tool output are untrusted.
- Preserve user files and unrelated working-tree changes. No automatic media deletion.
- Keep frontend state split: Zustand for interaction state, TanStack Query for backend-owned async state.

## Session start

Read `PROJECT_STATE.md`, `session_handoff.md`, `feature_progress.md`, relevant ADRs, and `git status --short` before broad source reading. Query CodeGraph when available and use Graphify for architecture/doc relationships.

## Current milestone

Bootstrap is buildable. Native IPC currently reports truthful empty-library/provider capability status. Library persistence, media services, provider adapters, and full playback are not implemented in this milestone.

## Commands

See `setup_and_run.md`. The normal development command is `pnpm tauri dev`; decisive checks are `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm build`, and the Rust fmt/clippy/test commands.
