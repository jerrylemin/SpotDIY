# SpotDIY performance baseline

Measurement date: 2026-09-03.

## Environment

- Windows 11 Pro, build 26200.
- GIGABYTE G5 KC, Intel Core i5-10500H, 6 cores / 12 logical processors.
- 15.8 GiB RAM.
- Local tools observed: Node `v24.14.0`, pnpm `11.19.0`, Rust/Cargo
  `1.95.0`. CI/release pins are Node `24.11.1` and pnpm `11.22.0`.
- Browser harness uses the repository's locked Playwright Chromium.

## Reproducible harness

`scripts/performance-baseline.test.ts` generates deterministic synthetic
metadata only. It exercises the production `buildMusicMapGraph` and
`buildGalaxyLayout` functions, performs one warm-up, then ten measured runs,
and emits median/p95/max timings. It asserts the supported graph bounds of
1,500 nodes and 3,500 edges, 5,000 Galaxy points, and the interaction budgets
of 2.0 seconds and 1.5 seconds.

Run it with:

```powershell
pnpm exec vitest run scripts/performance-baseline.test.ts --reporter=verbose
```

Observed result:

| Measurement | Median | P95 | Max | Result |
|---|---:|---:|---:|---|
| Music Map layout proxy, 1,500 nodes / 3,500 edges | 202.99 ms | 295.40 ms | 295.40 ms | PASS |
| Galaxy layout proxy, 5,000 points | 32.27 ms | 64.87 ms | 64.87 ms | PASS |

The proxy covers production layout computation, not native SQL latency or a
full packaged WebView render. No synthetic commercial media is used.

## Release measurements

| Target | Result | Evidence/limitation |
|---|---|---|
| Packaged cold launch, 5 launches; median <=3.5 s, p95 <=5.0 s | BLOCKED | No release executable could be produced because the local MSVC toolchain is missing headers/libraries. |
| Idle CPU/RAM after 30 s; <=2% / <=350 MiB | BLOCKED | Requires the packaged executable; no permanent visual `requestAnimationFrame` loop was found by source inspection. |
| 60 s local playback; combined CPU/RAM <=10% / <=450 MiB | BLOCKED | Requires a packaged app and owned mpv process; no packaged playback run was possible. |
| VisualExplorer 5,000-track SQL; p95 <=750 ms | BLOCKED | Requires the native Rust service; frontend layout proxy is recorded above. |
| Music Map layout/render readiness; <=2.0 s | PARTIAL | Production layout computation passes at 295.40 ms p95; full packaged render timing awaits a native/package-capable host. |
| Galaxy readiness; <=1.5 s | PARTIAL | Production layout computation passes at 64.87 ms p95; full packaged render timing awaits a native/package-capable host. |

The measured frontend bundle cleanup reduced the main JavaScript chunk from
the previous approximately 797 kB minified warning to `404.04 kB` minified
(`123.88 kB` gzip). Secondary routes now load lazily; no arbitrary Vite chunk
threshold was raised. No separate performance commit is warranted because no
reproducible budget failure remained in the measured production layout code.
