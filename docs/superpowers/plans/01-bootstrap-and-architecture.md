# Plan 01 — bootstrap and architecture

## Goal

Create one buildable Tauri 2 + React + strict TypeScript + Rust workspace with the approved SpotDIY identity, project memory, CI gates, and stable initial IPC contracts.

## Dependencies

Windows 11 x64 toolchain, Node/pnpm, Rust stable MSVC, WebView2, and the approved design spec.

## Exact files

`package.json`, `pnpm-lock.yaml`, `vite.config.ts`, `tsconfig*.json`, `eslint.config.ts`, `index.html`, `src/**`, `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/src/**`, `src-tauri/tauri.conf.json`, `src-tauri/icons/**`, `.github/workflows/ci.yml`, root memory docs, execution ledger, and design docs.

## Interfaces consumed

None; this freezes the first `ProviderKind`, `SourceCapabilities`, `AppStatus`, and `get_app_status`/`get_source_capabilities` boundary.

## Interfaces produced

Typed frontend domain vocabulary, Rust serialized DTOs, route tree, command registry, and a custom icon source.

## Tests

`pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm build`, Rust fmt, clippy, and Rust tests.

## Acceptance criteria

The browser bundle and native Rust crate build; the shell has truthful empty states, no login, keyboard command palette, provider badges, and no hardcoded secrets.

## Commit boundary

`chore: bootstrap SpotDIY architecture and development workflow`
