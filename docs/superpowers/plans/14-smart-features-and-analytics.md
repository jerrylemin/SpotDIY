# Plan 14 — smart features and analytics

## Goal

Add smart playlists, smart shuffle, discovery controls, sessions, local analytics, heatmap, Taste Timeline, Time Machine, Private Session, and Temporary Mode.

## Dependencies

Plans 02, 04, 08, and 13.

## Exact files

`src-tauri/src/analytics/**`, `src-tauri/src/smart/**`, `src-tauri/src/sessions/**`, `src/pages/AnalyticsPage.tsx`, `src/components/analytics/**`, `src/components/smart/**`, and tests.

## Interfaces consumed

History, queue, playlist rules, settings, and local clock/time data.

## Interfaces produced

SQL-backed rule evaluator, shuffle policy engine, session DTOs, analytics queries, private/temporary state boundaries, and reopen-as-queue commands.

## Tests

AND/OR rule evaluation, all shuffle modes/anti-repetition rules, session grouping, private-mode exclusion, temporary cleanup, and analytics aggregation.

## Acceptance criteria

Analytics never leaves the machine and smart features operate on local authoritative data without fake recommendations.

## Commit boundary

`feat: add local smart listening features and analytics`

## Delivery evidence — 2026-09-03

The implementation is present in the current checkout: schema 9's four new
tables, local qualified history/session recording, genre/release-date
metadata, Private Session, Temporary Mode, typed parameter-bound smart rules,
deterministic Smart Shuffle, analytics queries/UI, and Plan 13 trusted staging
hardening. Frontend typecheck/lint/test/build and Rust fmt pass; packaged
script syntax passes. Rust test/Clippy, Playwright, Tauri packaging, and
release-based smoke/roundtrip gates remain unverified because the local
MSVC/SDK and Chromium runtime are incomplete. Phase commits are `f516eee`,
`6a02daa`, `ab01f3e`, `aedd3a8`, `4527b02`, and `dbcdb2f`; the final docs commit
closes the record. These commits are local only until the blocked native and
package gates can run.
