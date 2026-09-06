# SpotDIY

SpotDIY is a local-first Windows music operating environment. It combines a
managed local library with search across Local, YouTube, SoundCloud, and
Spotify. Spotify search uses the local `spotdl` command; Spotify audio is
source-matched to an allowed YouTube/SoundCloud result and downloaded as MP3
through the existing yt-dlp/FFmpeg pipeline.

## Implemented product

- Local indexing, metadata/artwork, playback through an owned external mpv
  process, source fusion, resolver policy, downloads, and provider search.
- Durable playlists, Inbox, likes, ratings, tags, persistent queue, lyrics,
  bookmarks, A/B loop controls, history, analytics, smart playlists, and
  deterministic Smart Shuffle.
- Windows tray, global shortcuts, SMTC, overlays, output profiles, backup and
  Standard/Portable storage modes.
- Music Map, Library Galaxy, local preview, Theme Studio, dynamic accent, and
  persisted layout profiles.

## Boundaries

SpotDIY has no account creation, mandatory cloud database, or application
telemetry. Provider calls are made only when a selected source needs them.
Spotify records remain metadata-only for in-app playback. SpotDIY does not
stream or extract audio from Spotify; it only uses `spotdl` to match a Spotify
track to a permitted source before downloading. Respect the rights and terms
that apply to every source. Visual exploration is local/read-only, and preview
is limited to indexed local audio without queue, history, or analytics writes.

## Run locally

Exact Windows prerequisites and verification commands are in
[`setup_and_run.md`](setup_and_run.md).

```powershell
pnpm install --frozen-lockfile
pnpm dev       # browser preview
pnpm tauri dev # native Tauri window
```

## Release-candidate status

The Plan 16 release candidate is currently `PARTIAL`: the exact pinned CI
native/frontend/package jobs, NSIS artifact, clean install/uninstall, and
packaged feature smokes pass, and the local external-target Tauri/NSIS build
also passes. The broad packaged process-tree performance sample exceeds the
requested playback budget, so Plan 16 remains partial. Evidence is in
[`docs/SpotDIY-Vault/Sessions/final-verification.md`](docs/SpotDIY-Vault/Sessions/final-verification.md).

## Documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system boundaries and ownership.
- [`feature_progress.md`](feature_progress.md) — delivery status.
- [`setup_and_run.md`](setup_and_run.md) — Windows setup and release commands.
- [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) — dependency/license index.
- [`docs/superpowers/specs/2026-08-30-spotdiy-design.md`](docs/superpowers/specs/2026-08-30-spotdiy-design.md) — approved design.
- [`docs/SpotDIY-Vault/`](docs/SpotDIY-Vault/) — project knowledge vault.
- [`docs/execution/`](docs/execution/) — milestone, integration, and verification ledger.
