# ADR-0018: Windows overlays and system integration

Status: Accepted

Date: 2026-09-02

## Context

SpotDIY needs optional desktop surfaces and Windows system controls without
moving playback ownership, filesystem paths, native handles, or unbounded
diagnostics into the frontend. Windows behavior also varies by host, so
unsupported media controls and shortcut conflicts must remain visible states.

## Decision

Keep all Windows integration behind one `WindowsIntegrationService` native
boundary. It lazily owns four Tauri overlay windows with stable labels and
dimensions, a tray menu, nine typed global shortcuts, SMTC, Gaming click-through
recovery, and output-profile application. Reopening an overlay reuses its
window; visibility and click-through are session-only.

Persist only ordinary settings, shortcut bindings, and normalized output
profiles in schema 8. Migration 8 is destructive but WAL-safe and copies every
schema-7 settings row unchanged before adding the new allowlist entries.
Output-profile apply is serialized through `PlaybackService`, preserves track,
queue, position, and phase, and reports rollback outcomes on failure.

SMTC is enabled by default but exposes `ready`, `disabled`, `unsupported`, or
`failed` with detail. The WinRT implementation is isolated in the Windows-only
`spotdiy-windows-smtc` helper crate. Global shortcut registration exposes
per-binding status and does not claim a binding after conflict or registration
failure. Gaming click-through has a reserved `Ctrl+Alt+Shift+G` rescue path.

Capabilities stay narrow: overlay windows receive only the event and window
permissions required by their surfaces, while the main window receives only
the additional show/focus permissions needed by tray and command-palette
actions.

## Consequences

- Browser preview remains usable through an in-memory adapter but cannot claim
  native overlays, tray behavior, global shortcuts, or SMTC.
- Native failures are recoverable and explainable through typed snapshots;
  unavailable features are not represented as successful actions.
- The implementation adds no provider playback, no portable startup, no
  persisted overlay state, and no media-file mutation.
- Exclusive fullscreen games may cover a standard always-on-top Gaming overlay;
  windowed and borderless modes are the supported expectation.

## Evidence

Plan 12 implementation commits are `95eb41b`, `b7daac6`, `d9b58c3`, `e4793b6`,
and `3d39e1d`. The final packaged Windows smoke reported `SMTC READY`, a
registered controlled shortcut, overlay reuse/topmost, click-through recovery,
output-profile apply/restore, schema-8 restart persistence, and zero owned mpv
processes.
