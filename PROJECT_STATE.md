# SpotDIY project state

State date: 2026-08-30

## Repository

- Branch: `main`
- Origin: `https://github.com/jerrylemin/SpotDIY`
- Remote state: empty repository; no upstream branch existed to clone.
- Working tree: initial bootstrap changes are uncommitted until final verification.

## Runtime

- Frontend: React 19, TypeScript 6 strict, Vite 8, TanStack Router/Query, Zustand, Zod.
- Native: Tauri 2, Rust stable MSVC, typed serialized DTOs plus runtime frontend parsing.
- Current native commands: `get_app_status`, `get_source_capabilities`.
- Current persistence: none; status is an honest first-run empty state.

## Decisions in force

- Keep a single Tauri application.
- Keep provider-specific logic in adapters.
- Use explicit DTOs plus Zod at the initial IPC boundary; revisit generated types after compatibility is proven.
- Generate icons from `public/spotdiy-mark.svg`; keep provider colors secondary.

## Next slice

SQLite foundation and durable app settings, followed by local folder selection/indexing. Do not implement provider search before shared domain interfaces are frozen.
