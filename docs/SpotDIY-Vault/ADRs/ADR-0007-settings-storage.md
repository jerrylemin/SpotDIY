# ADR-0007: Typed ordinary settings and secure credential boundary

- Status: Accepted
- Date: 2026-08-30
- Scope: Plan 02 settings foundation

## Context

The shell needs durable theme, download-directory, source-preference, first-run, and storage-mode state. The application must not place Spotify credentials or other sensitive provider material in SQLite settings, IPC payloads, logs, or source control.

## Decision

Ordinary settings use the allowlisted `settings_metadata` table with JSON values, a declared value type, schema version, and update timestamp. Rust exposes typed reads, typed writes, a snapshot, and an atomic first-run transition through `SettingsRepository`; the frontend receives explicit DTOs validated with Zod. The current supported startup mode is standard storage at `%LOCALAPPDATA%\\SpotDIY\\spotdiy.sqlite3`.

The Rust settings layer distinguishes `SettingClass::Ordinary` from `SettingClass::Secret` and reserves a `SecretSettingKey` for future secure storage. No secret-bearing setting is accepted by the ordinary table or current IPC commands. Portable mode remains a later startup concern; the database opener is path-injected now, and a persisted portable value is rejected until the executable-location path selection is implemented.

## Consequences

Settings remain version-aware, constrained, and transactionally durable without exposing arbitrary SQL or generic key/value IPC. Credential storage can later use Windows Credential Manager without a schema migration or ordinary-settings API pretending that secrets are safe to persist in SQLite.
