# ADR-0002: explicit typed IPC at bootstrap

Status: accepted

The bootstrap uses explicit Rust DTOs serialized with Serde and Zod validation at the TypeScript boundary. Research found current Specta/tauri-specta compatibility should be re-evaluated per release; generated bindings can be added only after a stable integration is verified.
