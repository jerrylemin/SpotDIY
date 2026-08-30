---
name: spotdiy-session-bootstrap
description: Resume any SpotDIY development task from repository memory, verified Git state, and the first incomplete feature slice.
---

# SpotDIY session bootstrap

At the start of a SpotDIY task:

1. Read `codex_context.md`, `PROJECT_STATE.md`, `session_handoff.md`, and `feature_progress.md`.
2. Read the relevant ADRs and the selected implementation plan.
3. Run `git status --short` and `git log --oneline -20`.
4. Query CodeGraph for the subsystem when it is installed; do not perform a broad recursive crawl first.
5. Query Graphify when architecture or documentation relationships matter.
6. Resume the first incomplete task in `feature_progress.md`.

Treat current source, tests, and explicit user instructions as authoritative over stale memory. Preserve unrelated user changes. Keep provider logic behind capability-bearing adapters, keep Spotify metadata-only, and verify the smallest decisive check before reporting completion.
