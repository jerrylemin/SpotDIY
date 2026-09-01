# ADR-0011: Spotify PKCE and compliance isolation

- Status: Accepted
- Date: 2026-09-01
- Scope: Plan 05

## Context

The Plan 05 Spotify catalog boundary must not ship a client secret in a
desktop application or expose Spotify catalog functionality by default. The
older catalog-only Client Credentials recommendation is superseded for this
gated implementation.

## Decision

Use Authorization Code with S256 PKCE behind an explicit development and
compliance gate. The callback listener binds only to `127.0.0.1` on a dynamic
port, validates state and the exact callback path, and uses a fresh
43-to-128-character verifier. No client secret is accepted or sent.

Access and refresh tokens are kept in the Windows credential seam or process
memory only. Tokens, authorization codes, verifiers, raw callback data, and
provider payloads do not cross the frontend DTO boundary, enter SQLite, or
appear in logs. Refresh rotates the stored refresh token when Spotify returns
one.

Spotify is never included in unified search lenses. It is queried only by the
Spotify lens, and that lens remains disabled until the explicit gate and
authorization state permit it. Spotify results are metadata-only: no playback,
downloads, previews, scraping, or persistent catalog mirror is included.

## Consequences

- The application can remain secret-free in source control and in packaged
  frontend code.
- Catalog search requires a deliberate developer authorization step and can
  truthfully present a disabled/setup-required state.
- A later production policy decision may replace or extend this boundary, but
  it must preserve secret isolation and the no-playback/no-download scope.

## Evidence

Rust tests cover PKCE challenge/verifier generation, loopback binding, state and
callback validation, token exchange without a secret, refresh rotation,
disabled-gate no-network behavior, and typed Spotify error mapping. Frontend
IPC tests verify strict setup parsing and that no client secret is sent.
