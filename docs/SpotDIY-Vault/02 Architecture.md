# Architecture

SpotDIY uses a single Tauri 2 application. React/TypeScript renders the UI; Rust owns local files, SQLite, external media tools, providers, and Windows integration. See the root `ARCHITECTURE.md` and [[ADR-0001 Tauri Architecture]].

The frontend uses TanStack Query for backend-owned asynchronous state and Zustand for interaction state. Provider adapters normalize into shared domain DTOs and expose capabilities.
