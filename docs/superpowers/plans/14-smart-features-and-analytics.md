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
