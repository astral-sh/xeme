# Buffered delivery: normal-build evidence

## Source and decision

Runtime `279a0bee7bd6267886574063ad89cd5c63fd00bd` is based on published PR161 `5d340f500c5a3936f6e20ae87a968085c31571ea`; its control is unchanged selected empty-state runtime `66ca8e0b` (normal library 93b735). The candidate normal library is 52202d and the static library 87cb191. All 74 main and four auxiliary source files are bound to the actual commit. Five files change, including three runtime files. Dependencies, allocators and compiler policy are unchanged.

The combined change reuses a validated literal-attribute span when retained byte capacity already fits, keeps successful event delivery outside the error helper while preserving recovered-prefix publication, and skips the intermediate pending copy for known valid UTF-8 with empty pending storage and sufficient capacity. Allocation-failure restoration and cold/converted/invalid fallbacks remain. Measurements belong to the combined candidate; they do not establish each change’s isolated benefit. The proposed cross-feed C frame cache is absent from this source and unbuilt.

Selected. Retain the combined buffer/delivery change after improvements in both separate native and CPython epochs, all 24 real native conditions improving in each epoch, and unchanged original compatibility outcomes. Keep generated declaration regressions and every adverse row explicit; no isolated-component or statistical-significance claim.

Incomplete. Confirmation takes 1.2969x Expat native time and 1.1197x CPython time, above the roughly 1.10x aggregate goal. The parser remains experimental; original API 391 failures and two timeouts, two strict grouping failures per linkage, current-runtime sustained fuzzing and an installed PBS trial remain outstanding.

## Separate results

The initial native epoch takes 0.9674998083× the selected control and 1.2947884042× Expat across 24 real conditions; all 24 improve. Its four generated conditions take 1.0008890305× control and 3.5249863483× Expat; both rare-declaration regressions remain in full rows. Initial CPython time decreases 1.50%, to 1.1165x Expat. Confirmation decreases real native time 3.36% and CPython time 1.57%, remaining 1.2969x and 1.1197x Expat. Native improves in 24/24 real conditions and CPython in 23/24. The pyexpat aggregate is 1.0705x and ElementTree is 1.1711x Expat; 6/24 individual Python conditions meet the 1.10x goal. All 7 adverse main condition rows remain in the accompanying CSV. Both aggregates remain above the goal.

Every adverse main condition:

- buffered-delivery-initial-native: generated-rare-declarations, 4 KiB, false: 3.906% more time.
- buffered-delivery-initial-native: generated-rare-declarations, 64 KiB, false: 3.423% more time.
- buffered-delivery-initial-python: wayland, 4 KiB, elementtree: 0.027% more time.
- buffered-delivery-initial-python: batik, 64 KiB, pyexpat-events: 0.467% more time.
- buffered-delivery-confirmation-native: generated-rare-declarations, 4 KiB, false: 2.408% more time.
- buffered-delivery-confirmation-native: generated-rare-declarations, 64 KiB, false: 2.514% more time.
- buffered-delivery-confirmation-python: vulkan, 4 KiB, pyexpat-events: 1.008% more time.

Four main campaigns contain 104 condition rows, 2,496 workers and 265,080 samples. We never pool epochs, raw observations, process medians or paired medians.

| Project XML | Oriole (normal) | Expat (normal) | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 41.218 ms | 28.941 ms | 1.420× |
| Wayland protocol | 0.944 ms | 1.076 ms | 0.877× |
| Maven POM | 0.685 ms | 0.445 ms | 1.544× |
| Batik SVG | 0.128 ms | 0.134 ms | 0.957× |
| GTK UI | 0.282 ms | 0.216 ms | 1.307× |
| DocBook XSL | 0.235 ms | 0.187 ms | 1.251× |

The table derives only from 126 confirmation workers at 4 KiB/namespaces off; times and paired ratios are computed separately. See the archived table derivation and hashes.

## Arena whitespace diagnostic

This separate two-condition experiment has 48 workers and 1,398 samples. Each valid file contains a root, an eight-attribute lexical primer, a 3,999-byte warmup tag and 1,000 literal siblings. Both use the same attribute names/values and equal callback digests/counts. Tokens stay below 4 KiB and attribute count is eight. A single 4 MiB feed deliberately keeps the current C frame live across all elements; under the ordinary 4/64 KiB protocol it is dropped at each feed. No cross-feed warm-capacity claim follows.

The packed case takes 0.8902837809× control and 1.0603285887× Expat. The whitespace-heavy case takes 0.9918776808× control and 0.6304429315× Expat; two of its seven paired control comparisons regress. Its Expat paired ratios span 0.6181–0.9762, so the median is not a universal speed claim. All paired values and process medians remain in the diagnostic readback. Source-derived extra copied bytes are 16 versus 3,784 per target, without runtime hit counters. No threshold is tuned and these rows are excluded from main CSVs/aggregates. The known-UTF8 feed optimization is not asserted to activate on this single first feed.

## Method and host

Normal generic x86-64 Oriole uses Ohm rustc 1.98.1-dev / ohm-1.98.1-1 with O3, ThinLTO and one codegen unit; normal Expat 2.8.4 uses GCC 13.3 O3 without LTO. PGO work remains stopped. Native tests use the unchanged C driver, six pinned projects at 4/64 KiB with namespaces off/on, plus four generated conditions. Seven pairs shuffle engine order deterministically; one warmup is excluded from each process median. Ratios are medians of paired process-median ratios; aggregate ratios equally weight conditions geometrically. Parser construction, callbacks and destruction remain in the original measured lifecycle. Driver digests normalize Text fragmentation and cannot prove strict callback grouping.

The initial CPython epoch uses unmodified 3.12.13 sources: two fresh candidate modules built O2, four reused selected-control/Expat modules. Confirmation reuses all six modules with no compiles and performs fresh preflights. Its 24 conditions use the same projects/chunks and ElementTree/pyexpat consumers. Full canonical callback preflights and every sample remain.

Elapsed targets use CPU0 on the shared Linux AMD EPYC-Milan host. Before-run observations and only fully interior during-run sample intervals are reported independently. The initial native other-CPU idle+iowait fraction is 94.409%; this is observed activity, not enforced isolation or per-condition attribution. All four before/during host records are retained. The independent readbacks derive only fully interior intervals; CPU0 affinity does not reserve the host. The confirmation native pre-observation recorded only 80.89% CPU0 idle+iowait, and the initial Python pre-observation 88.65%; neither observation was retried or excluded. Weighted other-CPU idle+iowait fractions for the initial native, initial Python, confirmation native and confirmation Python windows are 94.409%, 96.173%, 92.526% and 98.047%. Per-epoch readers retain the full counter samples and weighted interval arithmetic. No statistical-significance or whole-application claim is made.

## Correctness and reproducibility

The ordinary build passes 465 Rust tests in 35 groups and all 7 recorded tool/check/build commands. The original API suite preserves 4,347 passing / 391 failing / two timed-out configurations, with all 4,740 ordered outcomes unchanged. All six C consumers pass. Shared/static strict CPython each retain 802 methods, 809 rendered outcomes and the same two grouping failures (raw exit 2). Both supplemental semantic tests pass for each linkage. The original API assertions and ceilings are unchanged. Strict CPython consumers include the pinned upstream allocation-cleanup backport; timed consumers use unmodified sources. Six C ASan/UBSan consumers link uninstrumented Rust with leak detection disabled. Current-source sustained fuzzing and installed PBS validation remain outstanding.

Agents authored/reviewed the separate source proposals and prepared controllers. Root owned targets and primary execution; separate saved audits reconstructed raw outcomes, medians, host intervals and source identities. The arena diagnostic preparer also ran its saved readback; a different source reviewer independently reconstructed its fixtures before execution. Only completed receipts support final claims. The archive includes both control/current source maps, unchanged 156 dependency source/license files, explicit raw inputs, scripts, command vectors, provenance, preparation attempts and all adverse rows. Binaries are identities only; historical workers are excluded. A single final independent packet readback precedes installation.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 5,971 indexed files (11,034,871 compressed bytes). [index.json](index.json) records each member hash, original path, size and excluded artifact identity. Every member and original byte was read back. [conditions.csv](conditions.csv) retains all 104 conditions, and [adverse-conditions.csv](adverse-conditions.csv) retains all 7 adverse rows. Four main campaigns remain separate; no observations or medians are pooled. A separate two-condition arena diagnostic retains all 48 workers/1,398 samples and seven paired ratios in report.json and the archive; it is excluded from these main CSVs and aggregates. [report.json](report.json) retains full precision.

Archive SHA-256: `748493300c8fb5c686c01daae44c48f959ba7bd2595670e2cc423b102f0eac0d`. Libraries, binaries, compiler profiles, unrelated historical worker records and machine configuration are excluded. Rebuilding the pinned sources does not promise byte-identical compiler output. Original readers use historical absolute paths; the member index maps archived inputs to their original names, and excluded artifact checks still require separately retained exact files.
