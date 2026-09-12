# PGO native allocator comparison

Namespaced jemalloc and mimalloc did not produce a useful real-XML time gain for this Oriole build. Both improved generated-input elapsed aggregates. Expat benefited slightly on real XML, so the matched real-XML Oriole/Expat gap widened under either alternative allocator. No parser source, ordinary driver, default allocator, or allocator settings changed.

## Native elapsed

Time ratios below are geometric means of the per-condition median of seven paired process-median ratios. Lower is faster. All 28 conditions and all 1,568 timed workers are retained; each worker discards one warmup. The 224 preflights matched all eight combinations on each condition.

| Group | Engine | ordinary | libc-MM / ordinary | jemalloc-MM / ordinary | mimalloc-MM / ordinary |
|---|---|---:|---:|---:|---:|
| real | oriole | 1.000000 | 1.002669 (18/24 regressions) | 0.998749 (11/24 regressions) | 0.999783 (7/24 regressions) |
| real | expat | 1.000000 | 1.002685 (17/24 regressions) | 0.984615 (6/24 regressions) | 0.985886 (7/24 regressions) |
| generated | oriole | 1.000000 | 1.001537 (1/4 regressions) | 0.885149 (0/4 regressions) | 0.877155 (1/4 regressions) |
| generated | expat | 1.000000 | 0.994778 (1/4 regressions) | 0.970976 (0/4 regressions) | 0.951537 (0/4 regressions) |

| Group | Oriole/Expat ordinary | libc-MM | jemalloc-MM | mimalloc-MM |
|---|---:|---:|---:|---:|
| real | 1.428561 | 1.431577 | 1.446136 | 1.449922 |
| generated | 5.075722 | 5.126787 | 4.659200 | 4.691064 |

Oriole is faster than Expat in 3/24 real conditions under each mode and 0/4 generated conditions. `elapsed-conditions.tsv` retains every condition and all 16 comparison ratios; `elapsed-groups.tsv` includes every project, both generated fixtures, and explicit real/generated/all groups. Paired raw ratios and process samples remain in `native-screen/results.json` and worker files.

## Workload peak RSS

The separate 672-worker/84-cohort CPU2 campaign ran after elapsed ended. Each fresh process used the same binary, provider mode, input/chunk/namespace condition and original iteration count. RSS is Linux current-executable VmHWM, read once after all parse/free iterations; seconds in these raw records are excluded from elapsed summaries. All observations matched preflight.

Geometric means below summarize per-condition median paired VmHWM ratios; they are not a group peak RSS value.

| Group | Engine | libc-MM / ordinary | jemalloc-MM / ordinary | mimalloc-MM / ordinary |
|---|---|---:|---:|---:|
| real | oriole | 1.001004 (10/24 higher) | 1.030060 (24/24 higher) | 1.733960 (24/24 higher) |
| real | expat | 1.001449 (13/24 higher) | 1.012483 (20/24 higher) | 1.629564 (24/24 higher) |
| generated | oriole | 0.993029 (0/4 higher) | 1.158321 (4/4 higher) | 1.717144 (4/4 higher) |
| generated | expat | 0.992137 (1/4 higher) | 1.054759 (3/4 higher) | 2.038091 (4/4 higher) |

Absolute RSS includes both static providers’ upstream startup constructors, libc/input/output buffers, parser DSO, workload pages and measurement setup. These are same-executable workload results, not isolated allocator resident bytes. The instrumented provider proof’s RSS is excluded.

`rss-conditions.tsv` contains every combination’s three absolute samples and median/min/max KiB for every condition. `rss-screen/results.json` retains paired KiB differences, ratios, all raw outputs and condition-ratio group summaries. No group statistic is a group peak RSS value.

## Validation and limits

- Frozen 8-process provider-proof replay passed: 144 parser-entry origin checks, 88 provider-origin rows, 108 injected allocation failures, 16 matching trace observations and final zero live suite blocks/bytes. Expat child-before-parent destruction contract preserved.
- Fresh strict C driver build passed. Callback/timer functions and timed loop are byte-identical to the ordinary source except parser constructor selection. Exactly six namespaced allocator functions are exported; no process malloc interposition or allocation ledger.
- Author audit checked all raw records, complete sample vectors, original condition/iteration/seed schedule, every ratio and summary, and before/after hashes. 282,688 elapsed parses plus 448 preflight parses and 121,152 RSS parses; proof cases separately retained. No native or RSS worker failed; no reruns.
- Per-worker 90s, preflight 600s and aggregate 1,200s limits were active with process-group cleanup. Root CPU1 C/API and CPU4 strict checks overlapped the early elapsed run; both later completed. Shared-host frequency/load/memory effects remain uncontrolled.
- These are native callback workloads. They do not establish allocator effects for actual CPython applications or justify a default change. Both parser PGO histories are frozen and engine-specific; no retraining occurred here.
- Retained preparation issues: initial exact-source verifier used suffix matching and rejected ambiguous Cargo.toml; fixed to exact archive members before preflight. A comment-only driver correction produced the identical executable. No benchmark failures were discarded.

## Components and reproducibility

`PREPARATION.md`, `RSS-PLAN.md`, both prepared protocols/schedules, full driver/controller patches and approval receipts define the campaigns. `inputs.json` and copied evidence bind source/build histories; provider archives and their complete source/build receipts remain in the separately sealed explicit-MM proof. Parser libraries are pinned by original absolute path and hash. `EXTERNAL_COMPONENTS.json` indexes these immutable dependencies.
