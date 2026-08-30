# SpotDIY Windows setup

This document records the verified bootstrap environment on 2026-08-30.

## Prerequisites

- Windows 11 x64 (Windows 10 compatibility is a target where Tauri/WebView2 supports it).
- Node.js 24.11.1.
- pnpm 11.22.0.
- Rust stable MSVC: rustc 1.98.0, cargo 1.98.0.
- WebView2 Runtime 148.0.3967.54.
- Python 3.10.11, FFmpeg, and yt-dlp 2026.08.19 are present for future media-tool slices.
- Visual C++ build tools are required by the MSVC Rust target.

Rust was installed with the official Rustup package through WinGet. Obsidian was updated to 1.13.7; the project vault remains plain Markdown and does not require Obsidian to build.

The repository pins TypeScript 6.0.3 because the current `typescript-eslint` release used by the project does not support TypeScript 7 yet. Revisit this pin when parser support is released.

## Install dependencies

From `C:\Users\Administrator\Documents\MEGA\SpotDIY`:

```powershell
pnpm install
```

Use `pnpm install --frozen-lockfile` in CI and release verification.

## Development

Run the browser preview:

```powershell
pnpm dev
```

Run the native Tauri window:

```powershell
$env:Path = "$(Join-Path $env:USERPROFILE '.cargo\bin');$env:Path"
pnpm tauri dev
```

The native window uses `http://127.0.0.1:1420` during development.

## Verification

```powershell
pnpm typecheck
pnpm lint
pnpm test
pnpm build

$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
Push-Location src-tauri
& $cargo fmt --all -- --check
& $cargo clippy --all-targets --all-features -- -D warnings
& $cargo test --all-targets
Pop-Location
```

## Release build

```powershell
$env:Path = "$(Join-Path $env:USERPROFILE '.cargo\bin');$env:Path"
pnpm tauri build
```

The Windows NSIS output is written under `src-tauri\target\release\bundle\nsis\` when a release build succeeds. The icon source is `public\spotdiy-mark.svg`; generated Tauri assets live under `src-tauri\icons\`.

## Media tools

The application will manage or validate `yt-dlp`, FFmpeg, and mpv through a Rust `MediaToolManager`. Do not place third-party executables, copyrighted downloads, credentials, or local databases in Git. The current shell only reports source intent; it does not claim that media playback or downloading is implemented yet.

## Data locations

The approved target is standard mode under `%LOCALAPPDATA%\SpotDIY`, with user music left at selected paths. Portable mode will keep `Data`, `Music`, `Covers`, `Lyrics`, `Database`, `Cache`, and `Config` beside `SpotDIY.exe`. The bootstrap shell has not created a user database yet; these paths become active with the storage/database slice.
