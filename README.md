# SpotDIY

SpotDIY is a local-first Windows music operating environment. It is designed to bring local files, YouTube, SoundCloud, and Spotify catalog metadata into one source-aware workspace without requiring a SpotDIY account.

The repository currently contains the first buildable foundation milestone:

- Tauri 2 + React + TypeScript strict + Vite desktop shell.
- Rust source-capability contract and Zod-validated native IPC.
- Custom SpotDIY mark and SVG icon language.
- Home, Search, Library, Playlists, Downloads, Settings, command palette, and empty-state player surfaces.
- Windows-oriented CI, frontend tests, Rust tests, formatting, and lint gates.

The product is intentionally being built in vertical slices. Library indexing, playback, provider adapters, downloads, lyrics, persistence, and advanced workspaces are tracked in [`feature_progress.md`](feature_progress.md) and the approved design spec.

## Run locally

Requirements and exact Windows commands live in [`setup_and_run.md`](setup_and_run.md).

```powershell
pnpm install
pnpm tauri dev
```

For browser-only UI work:

```powershell
pnpm dev
```

## Product boundaries

SpotDIY has no account creation or mandatory cloud database. User data is intended to live locally. Spotify is a catalog metadata source only; SpotDIY does not rip or circumvent Spotify audio protection. Playable equivalents are resolved from local, YouTube, or SoundCloud sources when available.

Provider requests are made only when the selected source requires them. Telemetry is off by default.

## Current source status

The shell exposes the intended provider capabilities and honest setup states. Source adapters are the next implementation slice; production screens must never substitute fake provider results for real adapter responses.

## Documentation

- [`docs/superpowers/specs/2026-08-30-spotdiy-design.md`](docs/superpowers/specs/2026-08-30-spotdiy-design.md) — approved product and technical design.
- [`docs/superpowers/plans/`](docs/superpowers/plans/) — independently testable implementation plans.
- [`docs/SpotDIY-Vault/00 Home.md`](docs/SpotDIY-Vault/00%20Home.md) — repository knowledge vault.
- [`docs/execution/`](docs/execution/) — milestone, agent, integration, and verification ledger.
