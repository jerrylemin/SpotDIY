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

## Fix round 1

Addressed all review findings without widening the Task 1 source scope. Native enums now serialize with `snake_case` (including `audio_quality` and `invalid_response`), while DTO field names remain `camelCase`. `SearchRequest.limit` and `ProviderSearchRequest.limit` now deserialize with the runtime default of 25 and retain the maximum validation of 50.

`SearchResult.published_at` now uses the validated `PartialDate` type; date values and precision are private and checked during construction/deserialization. Search result URL fields now use `SafeUrl`, a typed value that accepts only HTTPS URLs on the explicit provider/artwork host set, rejects userinfo, and rejects sensitive query data such as tokens, secrets, OAuth codes, cookies, and API keys. Provider and artwork validators return this safe value rather than an unconstrained `Url`.

Added coverage for accepted surrounding whitespace trimming, default limit deserialization, invalid date/precision pairs, snake_case enum names, sensitive URL data, and exact Spotify exclusion for Tracks, Artists, and Albums.

### Fix-round verification

All commands used the external target through `$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'`:

```text
cargo test --manifest-path src-tauri/Cargo.toml search::types::tests -- --nocapture
```

Result: 9 passed, 0 failed.

```text
cargo test --manifest-path src-tauri/Cargo.toml search::sort::tests -- --nocapture
```

Result: 3 passed, 0 failed.

```text
cargo test --manifest-path src-tauri/Cargo.toml sources::tests -- --nocapture
```

Result: 3 passed, 0 failed.

```text
cargo test --manifest-path src-tauri/Cargo.toml sources::traits::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Results: object-safety test 1 passed, formatting passed, and diff check passed. The source-only diff contains `lib.rs` plus the four owned Task 1 source files; the existing plan rewrite remains unstaged. This report was intentionally left out of the fix commit.

### Fix-round self-review and concerns

- All Task 1 native enum declarations use `snake_case`; structs retain `camelCase` field serialization.
- Missing limits default to 25 at deserialization and values above 50 remain rejected by validation.
- Partial dates cannot be deserialized with inconsistent precision or invalid calendar values.
- Safe URL construction rejects arbitrary hosts, non-HTTPS schemes, userinfo, and sensitive query parameters; benign provider query parameters remain available for canonical URLs.
- All, Tracks, Artists, and Albums contain no Spotify; Spotify contains only Spotify.
- No raw provider data or secrets were added, no runtime state was wired, and `src-tauri/target` remains absent.
- The combined two-filter Cargo command from the original brief remains incompatible with this Cargo CLI; separate focused filters provide the executable evidence.

## Fix round 2

Addressed the remaining re-review findings. SafeUrl now checks decoded query parameter names case-insensitively, recognizing `apiKey`/`api_key`, token, access-token, secret, client-secret, code, cookie, auth, OAuth, password, and related exact credential-bearing names. It no longer scans query values or arbitrary substrings, so `v=decoder1234` remains accepted while credential parameters are rejected. HTTPS and exact provider/artwork host restrictions remain enforced.

Tracks, Artists, and Albums now each have an exact mapping assertion for `[Local, YouTube, SoundCloud]`, excluding Spotify. `ProviderSearchError.retry_after_seconds` explicitly serializes as `retryAfterSeconds`; native enum values remain snake_case.

### Fix-round 2 verification

All focused commands used `$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'`:

```text
cargo test --manifest-path src-tauri/Cargo.toml search::types::tests -- --nocapture
```

Result: 10 passed, 0 failed.

```text
cargo test --manifest-path src-tauri/Cargo.toml search::sort::tests -- --nocapture
```

Result: 3 passed, 0 failed.

```text
cargo test --manifest-path src-tauri/Cargo.toml sources::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml sources::traits::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Results: source URL tests 3 passed, object-safety test 1 passed, formatting passed, and diff check passed. One initial run failed only because the newly added retry-after serialization test correctly exposed the missing explicit field rename; the field was fixed and the complete rerun passed.

### Fix-round 2 self-review

- Query validation examines parameter names only; benign query values are not rejected by credential-like substrings.
- `apiKey` and `api_key` plus the requested credential-bearing names are covered by exact normalized-name matching.
- Result URLs still require safe typed construction, HTTPS, approved hosts, no userinfo, and no sensitive query parameter names.
- All prior date validation, default limit, snake_case enum, camelCase field, whitespace, lens isolation, and object-safety fixes remain intact.
- Only `search/types.rs`, `search/sort.rs`, and `sources/mod.rs` are product changes in this round; the plan is untouched and the report remains outside the product commit.

## Fix round 3

Closed the composite credential query-name bypass. SafeUrl now lowercases query parameter names, normalizes hyphens to underscores, and checks token components, so `oauth_token`, `auth_token`, `api_token`, and their case/hyphen variants are rejected. Matching remains parameter-name based and does not inspect values; benign `v=decoder1234` remains accepted. All prior HTTPS, exact host, userinfo, sensitive-name, typed date, limit, serialization, and lens protections remain intact.

### Fix-round 3 verification

All focused commands used `$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'`:

```text
cargo test --manifest-path src-tauri/Cargo.toml search::types::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml search::sort::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml sources::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml sources::traits::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Results: 10 type tests passed, 3 lens tests passed, 3 URL tests passed, 1 object-safety test passed, formatting passed, and diff check passed. URL tests cover `oauth_token`, `AUTH-TOKEN`, `Api_Token`, `apiKey`, `access_token`, benign `v=decoder1234`, and prior scheme/host/userinfo protections.

### Fix-round 3 self-review

- Composite credential names are detected from normalized parameter-name components; query values are never scanned.
- Exact HTTPS/provider/artwork host restrictions remain enforced by SafeUrl.
- `retryAfterSeconds`, snake_case enum values, typed PartialDate, default limits, trimming, and exact Tracks/Artists/Albums mappings remain covered and green.
- The report and pre-existing plan rewrite remain uncommitted; the fix commit contains only `search/types.rs` and `sources/mod.rs` product changes.
