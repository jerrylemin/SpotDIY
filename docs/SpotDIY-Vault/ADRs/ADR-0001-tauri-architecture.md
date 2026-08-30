# ADR-0001: single Tauri application

Status: accepted

SpotDIY uses one Tauri 2 application with React/TypeScript in the webview and Rust for native services. This keeps the local-first desktop boundary explicit and avoids a premature monorepo. Provider and playback implementations remain replaceable behind service interfaces.
