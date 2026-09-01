# Spotify Web API research

> Superseded for Plan 05 (2026-09-01): the older Client Credentials
> recommendation in this research note is historical and is not the active
> implementation decision. Plan 05 uses Authorization Code with S256 PKCE,
> loopback-only callback handling, keyring/memory-only tokens, no client
> secret, and an explicit development/compliance gate. The historical research
> below is retained for provenance; do not use its former flow recommendation
> for the current implementation.

## Date

2026-08-30

This note records the official Spotify Web API, Developer Policy, Developer Terms, and design guidance reviewed on this date. It assumes that SpotDIY is a non-streaming catalog lookup tool, not a Spotify player, user-library manager, or data-analytics product.

## Primary sources (URLs)

- [Web API overview](https://developer.spotify.com/documentation/web-api)
- [API calls](https://developer.spotify.com/documentation/web-api/concepts/api-calls)
- [Search for Item](https://developer.spotify.com/documentation/web-api/reference/search)
- [Get Track](https://developer.spotify.com/documentation/web-api/reference/get-track)
- [Get Album](https://developer.spotify.com/documentation/web-api/reference/get-an-album)
- [Get Album Tracks](https://developer.spotify.com/documentation/web-api/reference/get-an-albums-tracks)
- [Get Artist](https://developer.spotify.com/documentation/web-api/reference/get-an-artist)
- [Spotify URIs and IDs](https://developer.spotify.com/documentation/web-api/concepts/spotify-uris-ids)
- [Track Relinking](https://developer.spotify.com/documentation/web-api/concepts/track-relinking)
- [Authorization](https://developer.spotify.com/documentation/web-api/concepts/authorization)
- [Client Credentials Flow](https://developer.spotify.com/documentation/web-api/tutorials/client-credentials-flow)
- [Rate Limits](https://developer.spotify.com/documentation/web-api/concepts/rate-limits)
- [Quota modes](https://developer.spotify.com/documentation/web-api/concepts/quota-modes)
- [February 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/february-2026)
- [March 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/march-2026)
- [July 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/july-2026)
- [Spotify Developer Policy](https://developer.spotify.com/policy)
- [Spotify Developer Terms](https://developer.spotify.com/terms)
- [Design & Branding Guidelines](https://developer.spotify.com/documentation/design)
- [Compliance Tips](https://developer.spotify.com/compliance-tips)

## Current API behavior

### Recommended SpotDIY boundary

SpotDIY can use the documented catalog endpoints to search and display album, artist, and track metadata. The least-privilege shape is a server-side Client Credentials integration using `type=album,artist,track`, an explicit market, and user-initiated requests. It should not add Spotify login, playback, audio previews, library writes, or user-profile access unless the product scope changes and a user OAuth flow is reviewed separately.

The Web API base URL is `https://api.spotify.com`; all requests require authorization and normally return JSON. Catalog responses include Spotify IDs, URIs, Spotify URLs, names, relationships, artwork URLs, and other metadata. ([API calls](https://developer.spotify.com/documentation/web-api/concepts/api-calls), [Spotify URIs and IDs](https://developer.spotify.com/documentation/web-api/concepts/spotify-uris-ids))

### Catalog search

`GET https://api.spotify.com/v1/search` requires:

- `q`: the search string.
- `type`: a comma-separated list. Supported values include `album`, `artist`, `playlist`, `track`, `show`, `episode`, and `audiobook`; SpotDIY should request only `album,artist,track`.
- `market`: an ISO 3166-1 alpha-2 country code when market-specific availability matters.

The query can use the documented filters `album`, `artist`, `track`, `year`, `upc`, `tag:hipster`, `tag:new`, `isrc`, and `genre`, but each filter applies only to particular result types. `year` supports a single year or a range. `tag:new` is an album filter for releases from the past two weeks; `tag:hipster` selects the lowest 10% by popularity in the documented legacy behavior.

The current limit is **5 by default and 10 maximum per requested item type**. `offset` starts at zero and is limited to 1000. Paginated results expose `next`, `previous`, `offset`, `limit`, and `total`. The February 2026 changelog records the reduction from a former maximum of 50/default of 20. ([Search for Item](https://developer.spotify.com/documentation/web-api/reference/search), [February 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/february-2026))

With Client Credentials there is no user country to take precedence. Spotify documents content as unavailable to the client when neither `market` nor a user country is provided, so SpotDIY should send a real, product-defined market on search and other market-sensitive catalog requests. Do not use a fabricated market to bypass availability restrictions. ([Search for Item](https://developer.spotify.com/documentation/web-api/reference/search), [Track Relinking](https://developer.spotify.com/documentation/web-api/concepts/track-relinking))

### Track, artist, and album metadata

The useful current catalog shape is:

| Entity | Useful metadata | Important caveats |
| --- | --- | --- |
| Track | `id`, `uri`, `name`, `artists[]`, nested `album`, `duration_ms`, `disc_number`, `track_number`, `explicit`, `external_urls.spotify`, and `external_ids` such as ISRC/EAN/UPC. | `popularity`, `available_markets`, and `linked_from` were removed from current responses by the February 2026 changelog. `preview_url` is nullable and deprecated. A market-specific response may expose `is_playable` and `restrictions`; restriction reasons include `market`, `product`, and `explicit`, with unknown future values to be handled safely. |
| Artist | `id`, `uri`, `name`, `images[]`, `external_urls.spotify`, and `href`. | `followers` and `popularity` were removed by the February 2026 changelog. The current reference page also marks `genres` as deprecated; do not make it required. Artists do not carry an album-style `release_date` field. |
| Album | `id`, `uri`, `name`, `album_type`, `total_tracks`, `artists[]`, `images[]`, `tracks` page, `release_date`, `release_date_precision`, `copyrights`, and `external_ids`. | `release_date` is the date the album was first released, not necessarily a complete day. `release_date_precision` is `year`, `month`, or `day`; SpotDIY must preserve that precision and must not invent missing month/day values. `available_markets`, `label`, and `popularity` were removed by the February 2026 changelog. |

The March 2026 changelog explicitly reverted the planned removal of `external_ids` for tracks and albums, so those identifiers remain usable but should still be treated as nullable/optional in response handling. ([Get Track](https://developer.spotify.com/documentation/web-api/reference/get-track), [Get Artist](https://developer.spotify.com/documentation/web-api/reference/get-an-artist), [Get Album](https://developer.spotify.com/documentation/web-api/reference/get-an-album), [March 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/march-2026))

The current endpoint reference pages still render some removed fields with a `Deprecated` label. The changelog is the safer compatibility baseline: treat `popularity`, `available_markets`, `linked_from`, album `label`, and artist `followers` as absent and never make them required for SpotDIY. The individual catalog endpoints that remain listed as available are `GET /search`, `GET /tracks/{id}`, `GET /albums/{id}`, `GET /albums/{id}/tracks`, `GET /artists/{id}`, and `GET /artists/{id}/albums`. ([February 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/february-2026))

#### Release and popularity fields

- Album release information is available as `release_date` plus `release_date_precision`. A value such as `1981-12` is intentionally month-precision; display it as such.
- The older reference description defines track popularity as an algorithmic 0–100 value influenced mainly by play volume and recency, not real-time, with duplicate track releases scored independently. It also says artist and album popularity is derived from track popularity. However, the February 2026 changelog removes popularity from track, artist, and album responses. Treat that description as historical documentation, not a stable field contract.
- SpotDIY must not replace the removed field with its own Spotify-derived popularity, listenership metric, benchmark, or ranking. Spotify’s policy prohibits analyzing Spotify Content or the Spotify Service to create derived listenership metrics, benchmarking, usage statistics, user metrics, or user profiles. ([Get Track](https://developer.spotify.com/documentation/web-api/reference/get-track), [Spotify Developer Policy](https://developer.spotify.com/policy), [February 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/february-2026))

### Artwork and attribution

Album and artist objects expose an `images` array ordered from widest to narrowest. Each image has a source `url`; `height` and `width` are nullable. Tracks receive artwork through their nested album object rather than a separate track-art field. Use the provided Spotify image URL and the matching entity’s `external_urls.spotify` link. ([Get Track](https://developer.spotify.com/documentation/web-api/reference/get-track), [Get Album](https://developer.spotify.com/documentation/web-api/reference/get-an-album), [Get Artist](https://developer.spotify.com/documentation/web-api/reference/get-an-artist))

When SpotDIY displays Spotify metadata, cover art, or artist images:

- Attribute the content with the Spotify brand/logo and link back to the applicable Spotify artist, album, or track.
- Keep artwork in its original form. Do not crop, animate, distort, blur, overlay text/images, or place SpotDIY branding on top of it. Follow the current design/branding guidance for presentation.
- Do not offer Spotify metadata or artwork as a standalone API, download, mirror, or product. A temporary cache is allowed only when strictly necessary for SpotDIY performance/functionality; it must not become an indefinite catalog.

Spotify’s Terms permit temporary local caching of metadata and cover art only as strictly necessary, and Spotify’s API-call guidance supports honoring `Cache-Control`, `ETag`, `If-None-Match`, and `304 Not Modified`. ([Spotify Developer Policy](https://developer.spotify.com/policy), [Spotify Developer Terms](https://developer.spotify.com/terms), [Design & Branding Guidelines](https://developer.spotify.com/documentation/design), [API calls](https://developer.spotify.com/documentation/web-api/concepts/api-calls))

### Client Credentials flow and restrictions

Client Credentials is Spotify’s server-to-server flow. It does not include user authorization, does not access user resources, and does not provide a refresh token. The documented token request is:

```http
POST https://accounts.spotify.com/api/token
Authorization: Basic base64(client_id:client_secret)
Content-Type: application/x-www-form-urlencoded

grant_type=client_credentials
```

The successful response contains `access_token`, `token_type` (`Bearer`), and `expires_in` (the documented example is 3600 seconds). Send the token to Web API requests as `Authorization: Bearer <access_token>`, obtain a new token after expiry, and keep the client secret exclusively on a trusted server. Never place the secret or a reusable Client Credentials token in browser/mobile code. ([Client Credentials Flow](https://developer.spotify.com/documentation/web-api/tutorials/client-credentials-flow), [Authorization](https://developer.spotify.com/documentation/web-api/concepts/authorization), [Spotify Developer Terms](https://developer.spotify.com/terms))

Spotify’s Web API getting-started documentation states that a Spotify Premium account is required to use the Web API. Development Mode additionally requires the app owner to have an active Premium account. Do not assume that a Client Credentials token removes that account prerequisite. ([Web API overview](https://developer.spotify.com/documentation/web-api), [Quota modes](https://developer.spotify.com/documentation/web-api/concepts/quota-modes))

Client Credentials is appropriate for SpotDIY’s public catalog lookup. It is not appropriate for `/me` endpoints, saved items, playlists owned by a user, playback control, or any other user-specific operation. If SpotDIY later needs those capabilities, it must use the appropriate user authorization flow and request only the minimum scopes. ([Authorization](https://developer.spotify.com/documentation/web-api/concepts/authorization), [Compliance Tips](https://developer.spotify.com/compliance-tips))

### Rate limits and quotas

Spotify does not publish one universal numeric request ceiling. The Web API rate limit is calculated from app calls in a **rolling 30-second window**, and the limit varies between Development Mode and Extended Quota Mode. Other endpoints may have additional limits. A rate-limited request returns HTTP 429; the response normally includes `Retry-After` in seconds. SpotDIY must wait as directed, use backoff with jitter, deduplicate requests, and avoid tight retry loops. ([Rate Limits](https://developer.spotify.com/documentation/web-api/concepts/rate-limits))

Rate limits and Development Mode quotas are separate:

- Newly created apps start in Development Mode. The app owner needs an active Spotify Premium account, and at most five authenticated Spotify users can use the app; users must be allowlisted.
- Development Mode quota is counted per developer account and shared by its Development Mode Client IDs. The July 2026 update raised the account’s Client ID/app limit from one to 25 and added `reason: QUOTA_EXCEEDED` to the structured 429 response for quota exhaustion.
- Endpoint quota buckets and their limits can change. Extended Quota Mode is for wider distribution, has a higher rate limit and no Development Mode allowlist, but Spotify’s current page says new quota-extension applications are accepted only from organizations and lists requirements including an established entity, launched service, at least 250k MAUs, key-market availability, commercial viability, and terms compliance.

SpotDIY must log status/reason without logging tokens, honor `Retry-After`, distinguish quota 429s from ordinary rate-limit 429s where the response supplies `reason`, and never create multiple Client IDs to evade a shared Development Mode quota. ([Quota modes](https://developer.spotify.com/documentation/web-api/concepts/quota-modes), [July 2026 Web API changelog](https://developer.spotify.com/documentation/web-api/references/changes/july-2026), [API calls](https://developer.spotify.com/documentation/web-api/concepts/api-calls))

## What SpotDIY must not do

The following are product and implementation prohibitions for the assumed catalog-only scope:

- Do not expose, ship, sell, transfer, or ask users for the Spotify client secret, access tokens, passwords, or other Spotify credentials. Use only Spotify’s documented authorization mechanisms.
- Do not scrape Spotify’s website, use private/undocumented endpoints, use robots or retrieval tools to copy/index Spotify Content, or make excessive calls that are not strictly required for the product.
- Do not use Client Credentials to access user data, control playback, or imply that catalog metadata grants streaming rights.
- Do not download, enable download, stream-rip, or make permanent copies of sound recordings or previews. Do not make `preview_url` a standalone preview service; it is deprecated and nullable.
- Do not build a permanent Spotify catalog mirror, bulk database, artwork CDN, metadata API, or other standalone Spotify-content service. Retain only what is strictly necessary, keep it current, and delete stale content.
- Do not send Spotify Content, including aggregate/anonymous/derived data, to ad networks, ad exchanges, data brokers, or other advertising/monetization toolsets. Do not use Spotify data for AI/ML training, embeddings, model input, derived listenership metrics, benchmarking, profiling, or targeted marketing.
- Do not alter Spotify metadata or visual content, crop/overlay artwork, remove rights notices, bypass country/market restrictions, or mislead users about the artist, creator, Spotify, or SpotDIY’s relationship with Spotify.
- Do not artificially manipulate plays, follows, or other Spotify activity with bots, scripts, automation, or compensation.
- Do not turn the integration into ringtones/alarms, games or trivia, voice control, webcasting, DJ/mixing/overlap, audio/visual synchronization, a business/public-broadcast product, a child-targeted product, or a core Spotify-experience replacement. These are prohibited or restricted use cases in the current policy.
- Do not use Spotify content to generate news media or commercial product offers. Non-streaming commercialization has only the limited permissions described in the policy and must still comply with all other terms.

The most important launch-specific warning is naming: Spotify’s policy says an app name should not begin with “Spot” or be confusing in sound or spelling with Spotify, while the design guidance separately prohibits names or logos that include or resemble Spotify’s marks. Because `SpotDIY` begins with `Spot`, obtain Spotify/legal review or rename the application before registration or distribution; do not treat the current name as cleared. ([Spotify Developer Policy](https://developer.spotify.com/policy), [Design & Branding Guidelines](https://developer.spotify.com/documentation/design), [Spotify Developer Terms](https://developer.spotify.com/terms))

## Rejected alternatives

| Alternative | Decision | Reason |
| --- | --- | --- |
| Scrape `open.spotify.com`, Spotify clients, or undocumented APIs | Reject | The Terms prohibit robots/spiders/retrieval tools used to retrieve, duplicate, or index Spotify Content and prohibit unauthorized access. |
| Client Credentials in a browser or packaged client | Reject | The flow requires a client secret; Spotify requires Security Codes to remain confidential and inaccessible to third parties. Use a server-side token broker. |
| Authorization Code or PKCE for the initial catalog feature | Reject for current scope | These flows add user authorization and scopes that catalog-only SpotDIY does not need. Reconsider only if SpotDIY adds user data, library, or playback capabilities. |
| `GET /tracks?ids=...`, `GET /albums?ids=...`, or `GET /artists?ids=...` batch fetches | Reject | The February 2026 changelog says these endpoints were removed. Fetch individual resources, or use the current album-track endpoint where it fits, while applying rate control. |
| `GET /browse/new-releases` as a release feed | Reject | The endpoint was removed in February 2026. Use album `release_date` and `release_date_precision` as metadata, not as a Spotify-curated new-release feed. |
| Popularity as a required sort/rank signal | Reject | Popularity was removed from current track, artist, and album responses and is marked deprecated on reference pages; deriving a replacement popularity metric would also create policy risk. |
| Permanent local catalog/artwork mirror | Reject | Storage is limited to what is strictly necessary, with temporary metadata/artwork caching allowed only for required performance/functionality. |

## Risks

1. **Reference/changelog divergence.** Current reference pages still display some removed fields and removed batch endpoints as deprecated. Build response parsing defensively, rely on the February/March 2026 changelogs for removal status, and test against live responses before implementation.
2. **Market-specific results.** A Client Credentials token has no user country. Missing `market` can make content unavailable, while different markets can produce different availability, restrictions, or relinking behavior. Do not promise global availability from one request.
3. **No stable popularity contract.** Popularity is both removed/deprecated and historically non-real-time. A product requirement that depends on popularity is not compatible with this baseline.
4. **Rate and quota uncertainty.** Spotify publishes the rolling-window model but not a universal numeric ceiling. Development quota is shared at the developer-account level, and quota buckets can change. A traffic spike can produce 429s even when an individual request looks harmless.
5. **Retention and artwork compliance.** The name “SpotDIY,” permanent storage, missing Spotify attribution/linking, altered artwork, or an art/metadata-only surface could independently create policy or branding problems.
6. **Credential and token security.** A leaked client secret can allow third parties to consume the app’s quota and access Spotify catalog data under SpotDIY’s identity. Keep secrets server-side, restrict logs, and follow the Terms’ compromise-notification obligation.
7. **Catalog volatility.** Spotify may remove content or change the platform without notice. Album takedowns can produce an empty name; image URLs, metadata, and availability should be treated as revocable and nullable rather than permanent assets.
8. **Commercial/extended-quota gate.** Extended quota currently targets organizations with a launched service and a stated 250k-MAU threshold. Do not assume a personal or early-stage app can obtain production-scale quota or unrestricted distribution.

## Version assumptions

- Research date is **2026-08-30**. The official Developer Terms page identifies Version 10, effective 2025-05-15; the Developer Policy page identifies the same effective date. Re-check both before launch because Spotify reserves the right to change the platform and terms.
- The February 2026 endpoint/field removals, March 2026 `external_ids` reversion, and July 2026 Development Mode quota update are treated as the current compatibility baseline.
- SpotDIY is assumed to be a non-streaming, catalog-only SDA: server-side Client Credentials, no Spotify user login, no user data, no playlists/library writes, no playback, no audio previews, no scraping, and no AI/ML processing of Spotify Content.
- The default market must be an actual product decision and should be passed explicitly to market-sensitive endpoints. It is not a mechanism for bypassing territorial restrictions.
- This note is research only. No production code or other repository files were changed.
