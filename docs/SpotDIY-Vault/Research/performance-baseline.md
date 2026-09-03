# SpotDIY performance baseline

Measurement date: 2026-09-03. All packaged measurements below use the exact
CI-built Plan 16 release executable from source `39b79bc63396897b6ddfaf81cce3cb2bd3180c2a`.

## Environment

- Windows 11 Pro, build 26200.
- GIGABYTE G5 KC, Intel Core i5-10500H, 6 cores / 12 logical processors,
  15.8 GiB RAM.
- Node `v24.14.0`, pnpm `11.19.0`; repository/CI Rust toolchain
  `1.98.1-x86_64-pc-windows-msvc`.
- Packaged app used the locked WebView2 runtime and repository mpv
  `v0.41.0-dev-g41f6a6450`.

## Reproducible frontend harness

`scripts/performance-baseline.test.ts` generates deterministic metadata only.
It exercises production `buildMusicMapGraph` and `buildGalaxyLayout`, performs
one warm-up plus ten measured runs, and asserts the supported graph bounds of
1,500 nodes / 3,500 edges and 5,000 Galaxy points.

```powershell
pnpm exec vitest run scripts/performance-baseline.test.ts --reporter=verbose
```

| Measurement | Median | P95 | Max | Result |
|---|---:|---:|---:|---|
| Music Map layout proxy, 1,500 nodes / 3,500 edges | 123.83 ms | 154.02 ms | 154.02 ms | PASS |
| Galaxy layout proxy, 5,000 points | 14.83 ms | 48.24 ms | 48.24 ms | PASS |

These are production layout-computation timings, not native SQL or packaged
render-readiness timings.

## Packaged measurements

| Target | Observed | Budget | Result |
|---|---:|---:|---|
| Cold launch, five fresh WebView2 profiles | median 0.541 s; p95/max 0.624 s | median <=3.5 s; p95 <=5.0 s | PASS |
| Idle SpotDIY parent after 30 s + 5 s sample | 2.50% CPU; 40.0 MiB | <=2%; <=350 MiB | FAIL on CPU; PASS on memory |
| Idle full process tree | 6.56% CPU; 448.8 MiB | <=2%; <=350 MiB | FAIL |
| 60 s local playback, full process tree peak | 57.81% CPU; 522.5 MiB | <=10%; <=450 MiB | FAIL |
| VisualExplorer 5,000-track native SQL p95 | not measured | <=750 ms | BLOCKED |
| Music Map packaged render readiness | not timed; route/interaction smoke passed | <=2.0 s | BLOCKED for timed gate |
| Galaxy packaged render readiness | not timed; route/interaction smoke passed | <=1.5 s | BLOCKED for timed gate |

The full process tree includes `spotdiy.exe`, WebView2 descendants, and any
owned mpv process. The parent-only sample explains why working-set numbers
from a single executable are much smaller; it does not satisfy the broader
process-tree budget. The playback harness reached position `60059 ms` and
confirmed continuous playback before pausing and closing. The CPU value is
the peak one-second aggregate over the 60-second window.

The idle sample observed WebView2 descendants at approximately 408.8 MiB and
the aggregate at 448.8 MiB. The playback sample peaked at 522.5 MiB. These
are recorded failures, not suppressed warnings; Plan 16 therefore remains
`PARTIAL`.

## Interpretation

Frontend layout computation and cold launch are within budget. The aggregate
idle/playback budgets need an explicit process-scope decision and, if the
full process tree remains authoritative, a follow-up optimization. Native
VisualExplorer SQL and packaged route readiness still need dedicated timed
instrumentation. No product optimization was added speculatively in Plan 16.
