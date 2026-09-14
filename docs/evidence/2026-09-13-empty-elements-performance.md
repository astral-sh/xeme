# Deliver native empty elements without transient stack and event entries

We previously pushed and popped a temporary element and queued an owned End event for eligible empty tags. We retain the independently owned End name in a dedicated slot, move it directly into the detached End frame, and reuse the scanner's committed position.

Native identity eligibility remains bounded; namespace expansion or declarations, defaults/types, converted input, external sources, and larger frames retain their existing paths. End keeps byte count zero and clears raw markup. Stop/abort completion, handler replacement, DefaultCurrent, allocation ownership, and switching between adapter and Rust event delivery retain their contracts.

Measured runtime [`08ccf6f9e6d3ced949893b0e00bac684c10bd120`](https://github.com/astral-sh/xeme/commit/08ccf6f9e6d3ced949893b0e00bac684c10bd120); base [`1957281e1422c9669ac9f7a68d350dab491eae85`](https://github.com/astral-sh/xeme/commit/1957281e1422c9669ac9f7a68d350dab491eae85). These identities describe the measured source even if later test, CI, or documentation commits advance the PR head; no byte-for-byte source-identity claim is made for those later heads. Ordinary O3/ThinLTO, one codegen unit, generic x86-64 builds use the same compiler; Ohm experimental defaults were disabled and each frozen build used fresh intermediates.

Shared library SHA-256: `eecf30dbed92fa692d67bd55dc0c34c30e3eb16bb3ac4316c373b98122fdb241`. Static SHA-256: `0133acd78b00eef4e182f8590251a8bb1484212102c49ea0a48a9cba3327577a`. Base shared SHA-256: `b364ffc5164f2057a1f339508e7be135ad6f6a74be9f1e347466b7586359cbf9`. Expat 2.8.4 control SHA-256: `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

## Matched elapsed-time results

Two separate epochs each use seven matched rounds, randomizing candidate/base/Expat order within each condition. Each native process contributes 20 measured parses and each CPython process 10, after one discarded warmup. A condition ratio is the median of seven paired process-median ratios; aggregate rows are geometric means of those condition ratios. Epochs are reported separately. Ratios are candidate/base or candidate/Expat elapsed time; lower is faster. Runs use CPU affinity 6, 4,096/65,536-byte feeds, native namespaces off/on, and CPython pyexpat/ElementTree modes. Each epoch has 60 native and 44 Python conditions: six tuning projects, five previously observed holdout projects, and four generated native controls. Python generated controls were not measured. Seeds are 20260910 and 20260911.

| Consumer / corpus | Conditions / epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | --- | --- | --- | --- | --- |
| native / tuning | 24 | 0.9713× | 1.1389× | 0.9763× | 1.1412× |
| native / observed-holdout | 20 | 0.9944× | 1.4119× | 0.9927× | 1.4111× |
| native / generated | 16 | 0.9128× | 1.5526× | 0.9158× | 1.5484× |
| python / tuning | 24 | 1.0007× | 1.1508× | 0.9992× | 1.1479× |
| python / observed-holdout | 20 | 0.9993× | 1.1908× | 0.9956× | 1.1908× |

Aggregate slowdowns occur in python/tuning (+0.07% / -0.08% in epochs 1 / 2). Native observed-holdout time is 1.4119× / 1.4111× Expat's time in epochs 1 / 2. This change does not close the overall native gap on that corpus.

Targeted rows are selected by the optimization's input shape; each combines both feed widths and the listed mode(s).

| Target | Epoch 1 time change / base | Epoch 1 / Expat | Epoch 2 time change / base | Epoch 2 / Expat |
| --- | --- | --- | --- | --- |
| native/elements/all | -37.69% | 1.1337× | -36.91% | 1.1136× |
| native/musescore/all | +1.99% | 1.6336× | +1.85% | 1.6340× |
| python/musescore/all | +2.35% | 1.2481× | +2.02% | 1.2510× |

## Adverse observations

We retain all 208 condition rows, including 65 / 64 slower-than-base observations in epochs 1 / 2; 62 conditions are slower in both epochs. No adverse observations are dropped or treated as statistically significant.

| Consumer / corpus | Slower epoch 1 | Slower epoch 2 | Conditions / epoch |
| --- | --- | --- | --- |
| native / tuning | 11 | 11 | 24 |
| native / observed-holdout | 16 | 16 | 20 |
| native / generated | 8 | 8 | 16 |
| python / tuning | 14 | 14 | 24 |
| python / observed-holdout | 16 | 15 | 20 |

Largest repeat regressions, ranked by the smaller of the two increases:

| Condition | Epoch 1 time change / base | Epoch 2 time change / base |
| --- | --- | --- |
| `native/entities/65536/namespaces-0` | +8.64% | +8.05% |
| `native/entities/65536/namespaces-1` | +8.45% | +7.73% |
| `native/namespaces/65536/namespaces-0` | +7.17% | +7.71% |
| `native/entities/4096/namespaces-1` | +7.32% | +6.58% |
| `native/entities/4096/namespaces-0` | +7.70% | +6.50% |

## Mechanism evidence

On Generated empty elements, namespaces off, 4,096-byte feeds, separately collected Callgrind Ir counts change from 219,622,005 to 161,093,361 (0.7335×, -26.65%). Four parses include warmup, parser lifecycle, and native callback hashing; callback signatures/counts match. These instruction counts are not a wall-time measurement.

## Compatibility and coverage

The workspace passes 540 default-feature tests across 38 suites and 540 tests without default features; formatting and all-target Clippy pass. The default log covers the final runtime; the final test-only matrix expansion ran without default features. Complete hashed logs and the command script are retained, but original command exit-code receipts are absent for these local runs; the separately linked exact-head CI provides complete process-status evidence. All ten frozen-library compatibility commands pass with source/library hashes unchanged. The original API suite retains 495 known Xeme failures among 4,740 rows, with 14 improvements against the reviewed baseline and 0 regressions; Expat retains 0 failures. Strict adapted allocation behavior passes 1,008 rows and 1,008 ownership reports per engine; Xeme has 0 live tracked blocks and 0 unexpected system failures. Its retry sweeps record 23,792 denied malloc calls and 1,625 denied realloc calls, not distinct allocation sites. W3C has 6,003 rows per engine, 960 mandatory catalog failures for Xeme and 960 for Expat, and 0 engine differences. All 2,730 differential semantic cases pass; exact observations retain 8 callback-stream and 84 final-position differences (exact_pass=false).

CPython 3.12.13 passes six unchanged upstream XML suites for local shared/static extensions with both original consumer sources and the disclosed upstream allocation-cleanup backport. Each run discovers the complete 803-case inventory and reports 802 run / 14 skipped, with no accepted failures; loaded-origin and text/order/buffering checks pass. Separate cleanup probes reproduce the original consumer's expected SIGSEGV and the backported consumer's MemoryError recovery in both linkage modes. These local regression gates retain the documented compatibility boundaries and do not establish an installed-distribution or exhaustive allocation-failure claim. Sanitizer and callback-aliasing coverage is separately identified by the exact CI check inventory and tested head.

## CI

PR head [`08ccf6f9e6d3ced949893b0e00bac684c10bd120`](https://github.com/astral-sh/xeme/commit/08ccf6f9e6d3ced949893b0e00bac684c10bd120) has 18 successful checks and one intentional `Profile-guided library` skip: [passed CI run 1](https://github.com/astral-sh/xeme/actions/runs/34787505775). The exact check inventory and retained status/report hashes are in the [CI summary](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-empty-elements/docs/evidence/2026-09-13-empty-elements-performance.json); the [CI report](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-empty-elements/docs/evidence/2026-09-13-empty-elements-performance.md#ci) identifies the tested head. The frozen timing and local compatibility results remain bound to their measured runtime. Subsequent test and workflow fixes receive their own CI qualification, and later documentation commits are not implicitly covered by this earlier run.

## Combined integration

The five selected changes also compose at frozen runtime `61c8c6d1784b92180436370e55b74050410dde74`. That exact combined source passes 551 default-feature tests and 551 without default features, formatting, Clippy, and all ten compatibility gates, with the same 495 known API failures, 960 W3C failures per engine, and eight callback/84 position differences. Its independent timings below measure the whole combination, not an additive estimate or the individual PR head. Its complete 208 condition rows and compatibility results are retained in the raw archive under `performance-followup/combined-outlined/`. The superseded integration `4eeb93a0df41c56a5b27653143529c60bcc1c7f1` remains archived separately under `performance-followup/combined/`; its initial native epoch observed -5.19% tuning, -4.56% observed-holdout, and -18.53% generated time changes against base. Those earlier results apply to its original namespace implementation and are excluded from the selected integration's final aggregates.

| Consumer / corpus | Conditions / epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | --- | --- | --- | --- | --- |
| native / tuning | 24 | 0.9430× | 1.1058× | 0.9427× | 1.1072× |
| native / observed-holdout | 20 | 0.9372× | 1.3335× | 0.9395× | 1.3345× |
| native / generated | 16 | 0.8092× | 1.3650× | 0.8084× | 1.3752× |
| python / tuning | 24 | 0.9944× | 1.1391× | 0.9910× | 1.1383× |
| python / observed-holdout | 20 | 0.9752× | 1.1551× | 0.9779× | 1.1586× |

Native observed-holdout time is 1.3335× / 1.3345× Expat's time in epochs 1 / 2. This change does not close the overall native gap on that corpus.

We retain all 208 condition rows, including 28 / 33 slower-than-base observations in epochs 1 / 2; 25 conditions are slower in both epochs. No adverse observations are dropped or treated as statistically significant.

| Consumer / corpus | Slower epoch 1 | Slower epoch 2 | Conditions / epoch |
| --- | --- | --- | --- |
| native / tuning | 3 | 2 | 24 |
| native / observed-holdout | 6 | 8 | 20 |
| native / generated | 2 | 3 | 16 |
| python / tuning | 9 | 10 | 24 |
| python / observed-holdout | 8 | 10 | 20 |

Largest repeat regressions, ranked by the smaller of the two increases:

| Condition | Epoch 1 time change / base | Epoch 2 time change / base |
| --- | --- | --- |
| `native/namespaces/65536/namespaces-0` | +7.70% | +8.17% |
| `native/namespaces/4096/namespaces-0` | +6.10% | +6.43% |
| `native/maven/65536/namespaces-1` | +4.89% | +5.88% |
| `native/maven/4096/namespaces-1` | +4.73% | +5.10% |
| `python/maven/4096/elementtree` | +4.33% | +4.11% |

## Limits and artifacts

The holdout has already been inspected and is an observed regression corpus, with no unseen-input claim. Generated controls remain separate from project aggregates. Every adverse condition is retained; a ratio above 1 is an observation, not a significance finding. The shared-host study does not establish confidence intervals, cross-machine performance, or a general parser-speed guarantee. Native timing includes parser creation, handler setup, feeds, native callback hashing, and free; file/library loading and process startup are excluded. Python timing includes parser and Python result creation/destruction with GC enabled, while imports, input loading, explicit collection, and canonical output checks are excluded. These parser/consumer workloads do not execute complete project builds, rendering, transformations, or external DTD/entity resolution. Python extension compilation uses identical O2 without PGO/LTO; explicit result destruction follows untimed validation and therefore has warmer cache lines than some production lifetimes. Full callback preflights and every measured sample are checked; all input/library hashes remain unchanged within each report. Callgrind and allocator counts were collected separately and are mechanism evidence, not elapsed-time estimates.

[Compact results](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-empty-elements/docs/evidence/2026-09-13-empty-elements-performance.json), [all 208 conditions](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-empty-elements/docs/evidence/2026-09-13-empty-elements-performance.csv), and [verified raw archive](https://github.com/astral-sh/xeme/releases/download/performance-followup-2026-09-13/performance-followup-2026-09-13.tar.zst) (SHA-256 `80222e8cadbf6d90a305ad23f4e06d0e99f662bb7ef7c019dab8c417fbf17ecf`). The archive contains complete raw reports, command logs, compatibility outcomes, source snapshots, and frozen shared/static libraries; its external sidecar records decompression and member-hash verification.
