# Task 1 report: Search contracts, adapter boundary, and URL security

## Implementation

Implemented the typed search boundary and provider adapter seam. The change adds UUID-backed `SearchId`, validated search requests, search lenses/entities/sorts, normalized result and provider-section DTOs, typed provider errors/runtime states, partial-date precision, lifecycle DTOs, and Tokio watch-based cancellation. Provider selection maps `All`, `Tracks`, `Artists`, `Albums`, and `Playlists` to Local/YouTube/SoundCloud only; provider-specific lenses select only their provider, with Spotify isolated.

The `SourceAdapter` trait is object-safe and `Send + Sync`, using a boxed future without adding a dependency. Provider URL validation requires HTTPS and an explicit provider host allowlist. Artwork sanitization accepts only `i.ytimg.com`, `yt3.ggpht.com`, `i1.sndcdn.com`, and `i.scdn.co`.

## Files

- `src-tauri/src/search/types.rs`
- `src-tauri/src/search/sort.rs`
- `src-tauri/src/sources/mod.rs`
- `src-tauri/src/sources/traits.rs`
- `src-tauri/src/lib.rs`
- This report file

The existing uncommitted rewrite of `docs/superpowers/plans/05-source-adapters-and-search.md` was preserved. No runtime state was wired, and no later-task files were changed.

## RED evidence

Required literal command:

```text
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml search::tests sources::tests -- --nocapture
```

Result: failed before compilation because this Cargo version accepts only one positional test filter (`unexpected argument 'sources::tests'`).

Equivalent focused RED checks were then run separately before implementation:

```text
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml search::tests -- --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml sources::tests -- --nocapture
```

Result: failed at compilation for the intended missing-contract reasons: the `search` module did not exist and URL helper functions were unresolved.

## GREEN evidence

After implementation and formatting:

```text
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml search::types::tests -- --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml search::sort::tests -- --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml sources::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Results: 5 search-type tests passed, 2 lens-selection tests passed, 2 URL/security tests passed, and formatting passed. The tests cover all ten names required by the brief, including object safety in `sources::traits::tests`.

The required post-change graph refresh also completed with `graphify update .`; `src-tauri/target` remains absent.

## Self-review

- `all_provider_kinds_for_lens` excludes Spotify for All, Tracks, Artists, Albums, and Playlists.
- Spotify selection returns only Spotify.
- URL validation rejects non-HTTPS, JavaScript, file, data, missing-host, and arbitrary-host URLs.
- Artwork URLs are restricted to the four explicit HTTPS CDN hosts.
- `SearchResult` contains normalized typed fields only; no raw JSON, credentials, headers, cookies, executable paths, stderr, direct media URLs, or provider secrets.
- DTO fields use camelCase serialization and native enum values use lowercase/camelCase conventions.
- Only the external Cargo target was used; no repository-local target was created.
- `git diff --check` passed.

## Concerns

- The brief’s combined Cargo filter command is incompatible with the installed Cargo CLI, so equivalent single-filter commands provide the executable GREEN evidence.
- `SearchResult.published_at` is currently an optional wire date string paired with `published_precision`; later provider parsers should construct it only from validated partial dates.
- Provider URL host lists for canonical result URLs are intentionally explicit and conservative; later adapters must validate before constructing result DTOs.
