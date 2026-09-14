# Resolve namespace frames once and append attribute names directly

We previously resolved element namespace names more than once and allocated temporary expanded attribute names. We pass the prepared element name into frame lowering and append qualified attribute names directly into the callback frame. Namespace preparation remains a separate function, keeping its temporary state outside the shared start-tag frame.

Namespace triplets, duplicate expanded-name detection, declarations, URI work charges, allocation ownership, and failure cleanup retain their contracts. Larger or otherwise unsupported tags retain general event delivery.

Measured runtime [`ad5e08a3aeda572760dd6ef7a9d402b60880897c`](https://github.com/astral-sh/xeme/commit/ad5e08a3aeda572760dd6ef7a9d402b60880897c); base [`1957281e1422c9669ac9f7a68d350dab491eae85`](https://github.com/astral-sh/xeme/commit/1957281e1422c9669ac9f7a68d350dab491eae85). These identities describe the measured source even if later test, CI, or documentation commits advance the PR head; no byte-for-byte source-identity claim is made for those later heads. Ordinary O3/ThinLTO, one codegen unit, generic x86-64 builds use the same compiler; Ohm experimental defaults were disabled and each frozen build used fresh intermediates.

Shared library SHA-256: `645109ab7d42bf189bd6d929e0144ce6b7e58f357357545ea93b298d10ac08a0`. Static SHA-256: `c7e94c6c605a1b7df35dc6e22afdb0e54a6661988fe97e5b4637e8348f133fc3`. Base shared SHA-256: `b364ffc5164f2057a1f339508e7be135ad6f6a74be9f1e347466b7586359cbf9`. Expat 2.8.4 control SHA-256: `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

## Matched elapsed-time results

Two separate epochs each use seven matched rounds, randomizing candidate/base/Expat order within each condition. Each native process contributes 20 measured parses and each CPython process 10, after one discarded warmup. A condition ratio is the median of seven paired process-median ratios; aggregate rows are geometric means of those condition ratios. Epochs are reported separately. Ratios are candidate/base or candidate/Expat elapsed time; lower is faster. Runs use CPU affinity 6, 4,096/65,536-byte feeds, native namespaces off/on, and CPython pyexpat/ElementTree modes. Each epoch has 60 native and 44 Python conditions: six tuning projects, five previously observed holdout projects, and four generated native controls. Python generated controls were not measured. Seeds are 20260910 and 20260911.

| Consumer / corpus | Conditions / epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | --- | --- | --- | --- | --- |
| native / tuning | 24 | 1.0003× | 1.1736× | 0.9992× | 1.1748× |
| native / observed-holdout | 20 | 0.9844× | 1.4078× | 0.9894× | 1.4061× |
| native / generated | 16 | 0.9780× | 1.6451× | 0.9821× | 1.6616× |
| python / tuning | 24 | 1.0034× | 1.1545× | 0.9985× | 1.1538× |
| python / observed-holdout | 20 | 0.9937× | 1.1762× | 0.9951× | 1.1737× |

Aggregate slowdowns occur in native/tuning (+0.03% / -0.08% in epochs 1 / 2); python/tuning (+0.34% / -0.15% in epochs 1 / 2). Native observed-holdout time is 1.4078× / 1.4061× Expat's time in epochs 1 / 2. This change does not close the overall native gap on that corpus.

Targeted rows are selected by the optimization's input shape; each combines both feed widths and the listed mode(s).

| Target | Epoch 1 time change / base | Epoch 1 / Expat | Epoch 2 time change / base | Epoch 2 / Expat |
| --- | --- | --- | --- | --- |
| native/namespaces/namespaces-1 | -10.33% | 1.9095× | -8.83% | 1.9352× |
| native/libreoffice/namespaces-1 | -6.32% | 1.4742× | -6.19% | 1.4654× |
| python/libreoffice/all | -2.76% | 1.2247× | -2.49% | 1.2260× |

## Adverse observations

We retain all 208 condition rows, including 44 / 39 slower-than-base observations in epochs 1 / 2; 30 conditions are slower in both epochs. No adverse observations are dropped or treated as statistically significant.

| Consumer / corpus | Slower epoch 1 | Slower epoch 2 | Conditions / epoch |
| --- | --- | --- | --- |
| native / tuning | 11 | 10 | 24 |
| native / observed-holdout | 4 | 6 | 20 |
| native / generated | 7 | 7 | 16 |
| python / tuning | 15 | 10 | 24 |
| python / observed-holdout | 7 | 6 | 20 |

Largest repeat regressions, ranked by the smaller of the two increases:

| Condition | Epoch 1 time change / base | Epoch 2 time change / base |
| --- | --- | --- |
| `native/maven/4096/namespaces-1` | +3.09% | +3.83% |
| `native/maven/65536/namespaces-1` | +2.04% | +3.53% |
| `native/batik/4096/namespaces-0` | +1.74% | +2.49% |
| `python/maven/4096/elementtree` | +1.72% | +3.08% |
| `native/namespaces/65536/namespaces-0` | +2.30% | +1.71% |

## Mechanism evidence

On Generated namespaces, namespaces on, 4,096-byte feeds, separately collected Callgrind Ir counts change from 337,362,195 to 290,471,057 (0.8610×, -13.90%). Four parses include warmup, parser lifecycle, and native callback hashing; callback signatures/counts match. These instruction counts are not a wall-time measurement.

A separate counts-only generated namespace probe removes 10,000 malloc/free pairs for 10,000 qualified attributes: namespaces-on malloc calls are 10,190 → 190 at 4,096-byte feeds and 10,040 → 40 at 65,536-byte feeds; realloc calls remain 164 and 14 respectively. Namespaces-off counts remain unchanged. All 12 engine/feed/mode rows retain matching callbacks and zero live tracked blocks/system failures. Counts include supplied-allocator parser lifecycle work, exclude input/tracker metadata, and do not measure RSS or allocations bypassing that allocator.

## Compatibility and coverage

The workspace passes 538 default-feature tests across 38 suites and 538 tests without default features; formatting and all-target Clippy pass. All ten frozen-library compatibility commands pass with source/library hashes unchanged. The original API suite retains 495 known Xeme failures among 4,740 rows, with 14 improvements against the reviewed baseline and 0 regressions; Expat retains 0 failures. Strict adapted allocation behavior passes 1,008 rows and 1,008 ownership reports per engine; Xeme has 0 live tracked blocks and 0 unexpected system failures. Its retry sweeps record 23,888 denied malloc calls and 1,637 denied realloc calls, not distinct allocation sites. W3C has 6,003 rows per engine, 960 mandatory catalog failures for Xeme and 960 for Expat, and 0 engine differences. All 2,730 differential semantic cases pass; exact observations retain 8 callback-stream and 84 final-position differences (exact_pass=false).

CPython 3.12.13 passes six unchanged upstream XML suites for local shared/static extensions with both original consumer sources and the disclosed upstream allocation-cleanup backport. Each run discovers the complete 803-case inventory and reports 802 run / 14 skipped, with no accepted failures; loaded-origin and text/order/buffering checks pass. Separate cleanup probes reproduce the original consumer's expected SIGSEGV and the backported consumer's MemoryError recovery in both linkage modes. These local regression gates retain the documented compatibility boundaries and do not establish an installed-distribution or exhaustive allocation-failure claim. Sanitizer and callback-aliasing coverage is separately identified by the exact CI check inventory and tested head.

## CI

PR head [`ad5e08a3aeda572760dd6ef7a9d402b60880897c`](https://github.com/astral-sh/xeme/commit/ad5e08a3aeda572760dd6ef7a9d402b60880897c) has 18 successful checks and one intentional `Profile-guided library` skip: [passed CI run 1](https://github.com/astral-sh/xeme/actions/runs/34789412553). The exact check inventory and retained status/report hashes are in the [CI summary](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-namespace-expansion/docs/evidence/2026-09-13-namespace-expansion-performance.json); the [CI report](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-namespace-expansion/docs/evidence/2026-09-13-namespace-expansion-performance.md#ci) identifies the tested head. The frozen timing and local compatibility results remain bound to their measured runtime. Subsequent test and workflow fixes receive their own CI qualification, and later documentation commits are not implicitly covered by this earlier run.

## Namespace selection history

These separate native studies informed selection; each uses seven matched rounds and the same 60 conditions. The initial candidate uses 20 measured parses per process; trials A/B/C use 10. All changes below are elapsed time against base. The trials reuse inspected inputs, precede the final two epochs, and are excluded from the final aggregates. Their full condition rows and identities remain in the raw archive.

| Variant | Runtime | Parses / process | Tuning | Observed holdout | Generated | Maven NS on | Slower |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Initial namespace candidate | `db6da8c97c95405d4c68f1238761c3c05c13ee85` | 20 | +1.34% | +0.36% | +0.20% | +4.31% | 49 / 60 |
| A: inline finalizer | `c223426ec2e4a9b5bb48adfe5a0a0e16e41fc942` | 10 | +1.80% | +1.23% | +1.53% | +3.92% | 50 / 60 |
| B: specialize finalizer | `266c72817e96233c3799e92bb80d1fd58e24e77c` | 10 | +0.74% | +0.63% | +1.70% | +5.33% | 43 / 60 |
| C: outline preparation | `ad5e08a3aeda572760dd6ef7a9d402b60880897c` | 10 | -0.12% | -1.49% | -1.67% | +1.94% | 25 / 60 |

The initial namespace candidate improved the namespace target while slowing most conditions. Forcing finalizer inlining (A) and restoring finalizer specialization (B) retained broad regressions. C keeps namespace preparation outside the shared start-tag function while preserving prepared-name reuse and direct attribute-name appends. Its trial brought tuning near even and reduced observed-holdout time, but Maven with namespaces enabled remained slower. Assembly shows a smaller shared start-tag stack frame; the study does not establish stack size or code placement as the cause of unrelated-input timing changes.

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

[Compact results](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-namespace-expansion/docs/evidence/2026-09-13-namespace-expansion-performance.json), [all 208 conditions](https://github.com/astral-sh/xeme/blob/charlie/codex-perf-namespace-expansion/docs/evidence/2026-09-13-namespace-expansion-performance.csv), and [verified raw archive](https://github.com/astral-sh/xeme/releases/download/performance-followup-2026-09-13/performance-followup-2026-09-13.tar.zst) (SHA-256 `80222e8cadbf6d90a305ad23f4e06d0e99f662bb7ef7c019dab8c417fbf17ecf`). The archive contains complete raw reports, command logs, compatibility outcomes, source snapshots, and frozen shared/static libraries; its external sidecar records decompression and member-hash verification.
