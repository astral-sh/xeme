# Qualified native runtime for an opt-in CPython trial

Runtime `5d983f7e` is selected for a **controlled, opt-in Linux x86-64 CPython 3.12.13 trial**. The performance and bounded qualification milestone is complete: the current 20% aggregate target versus Expat is met in two native and two CPython epochs. Confirmation takes **1.1841× Expat’s native real-project time and 1.0558× its CPython time**, improving 6.84% and 3.91% against the raw-view baseline. Individual outliers and all failed compatibility outcomes remain explicit below. The [final adoption decision](final-adoption.json) records this trial scope separately from full Expat compatibility.

## Tested source

The [normal source binding](source/publication-source-binding.json) verifies all 74 tested source files and four auxiliary manifests against `5d983f7e09a095fc6c9a94c6ce05ef5a8ec95c8d`. This is the exact source used for the [ordinary build](combined/build.json), [normal libraries](combined/build-binding.json) and the measurements in this report. The separate measurement/qualification worktree contains the same tested snapshot. The [final source binding](source/qualified-source-binding.json) additionally verifies all 330 original source, auxiliary, fuzz-harness and seed files against this commit, including the ASan campaign executed after source publication. Original records retain their own execution dates and identities.

The [three-commit chain](source/publication-commits.json) adds [direct-source Start / PR169](https://github.com/astral-sh/oriole/pull/169), [default namespace storage / PR170](https://github.com/astral-sh/oriole/pull/170) and [declaration grammar / PR171](https://github.com/astral-sh/oriole/pull/171) after PR168’s matched-End change. [Exact step patches](source/steps/), [independent source review](source/independent-review.json), [integration resolution](source/namespace-resolution.json) and [test inventory](source/test-inventory.json) are retained. Eligible Start and End paths avoid complete raw-token copies; a dedicated owner avoids empty-prefix hash lookups; declaration grammar is checked before eligible unknown-encoding callbacks. Owned fallbacks, selected allocator routing, limits, raw errors and callback ownership remain covered. The public Rust `ErrorKind::TextDeclaration` variant requires downstream exhaustive matches to handle the new variant; the C ABI is unchanged.

The measured normal library is `c3e65339…` (static `b64051b1…`), compared with raw-view baseline `67c704c` / library `e59d89d6…` and normal Expat `7a333bc8…`. [Standalone records](STANDALONE.md) remain separate: neither intermediate stack binaries nor additive per-change gains are inferred from this final composition.

## Performance

Ratios are candidate time divided by the comparison implementation; lower is faster. Each epoch retains every condition and its original paired arrays.

| Group | Initial / raw-view | Confirmation / raw-view | Initial / Expat | Confirmation / Expat |
|---|---:|---:|---:|---:|
| Native real projects, 24 conditions | 0.9314 | 0.9316 | 1.1822 | 1.1841 |
| Native namespaces off, 12 | 0.9346 | 0.9367 | 1.1337 | 1.1351 |
| Native namespaces on, 12 | 0.9282 | 0.9265 | 1.2328 | 1.2352 |
| Native generated controls, 4 | 1.0378 | 1.0288 | 3.5324 | 3.5118 |
| CPython all consumers, 24 | 0.9625 | 0.9609 | 1.0554 | 1.0558 |
| ElementTree, 12 | 0.9560 | 0.9521 | 1.0871 | 1.0914 |
| pyexpat events, 12 | 0.9691 | 0.9698 | 1.0245 | 1.0214 |

Complete native [initial](combined/native/review.json) / [confirmation](combined/native-confirmation/review.json) and Python [initial](combined/python/normal-review.json) / [confirmation](combined/python-confirmation/normal-review.json) readbacks have corresponding all-condition CSVs: [native initial](combined/native/conditions.csv), [native confirmation](combined/native-confirmation/conditions.csv), [Python initial](combined/python/normal-conditions.csv), [Python confirmation](combined/python-confirmation/normal-conditions.csv). The [combined count and adverse-condition summary](combined/performance-summary.json) retains **104 epoch-condition rows, 2,496 workers and 265,080 samples**. Epochs are not pooled.

All **12 adverse epoch rows across nine distinct conditions** remain. Native real projects win 24/24 initially and 22/24 in confirmation; confirmation’s Batik 4 KiB without namespaces and DocBook 4 KiB with namespaces regress 0.10% and 0.16%. Python wins 21/24 then 23/24: both initial DocBook ElementTree rows and Batik 64 KiB pyexpat remain adverse; the latter repeats at +0.034% in confirmation. Generated entity cases regress 7.52–8.13% initially and 5.85–5.90% in confirmation, with a different rare-declaration chunk size adverse in each epoch.

The 20% target applies to the native real-project and Python consumer aggregates. It is not an individual-condition waiver: 14/24 native real rows exceed 1.20× Expat initially and 13/24 in confirmation, as do six Python rows in each epoch. The native namespace-on aggregate remains 1.2352×, Maven reaches 1.6645×, and confirmation DocBook ElementTree reaches 1.3331× Expat.

## Six README rows

These confirmation 4 KiB, namespaces-off values are [independently reconstructed from 126 raw workers](combined/native-confirmation/readme-benchmarks.json). Times are medians of seven process medians; ratios are medians of seven paired ratios, so they need not equal the quotient of the displayed rounded times.

| Project | Candidate (ms) | Expat (ms) | Paired candidate / Expat |
|---|---:|---:|---:|
| Vulkan | 36.366 | 28.906 | 1.266 |
| Wayland | 0.887 | 1.069 | 0.840 |
| Maven | 0.589 | 0.440 | 1.332 |
| Batik | 0.124 | 0.134 | 0.931 |
| GTK | 0.248 | 0.212 | 1.165 |
| DocBook | 0.225 | 0.189 | 1.192 |

## Method and reproducibility

Measurements use a shared Linux x86-64 host with elapsed work pinned to CPU0. Oriole uses generic x86-64 Ohm O3/ThinLTO/one codegen unit; Expat 2.8.4 uses GCC 13.3 O3/shared/no-LTO. CPython 3.12.13 benchmark consumers are unchanged and built with O2; confirmation reuses all six modules. Medians of seven paired process-median ratios form equally weighted geometric means. No PGO, statistical significance, host isolation or whole-application speed claim is made. Native callback digests and Python canonical checks coalesce adjacent text; strict grouping assertions remain separate.

Host observations are retained for native [initial before](combined/native/host-before-native.json) / [during](combined/native/host-during-native.json), native [confirmation before](combined/native-confirmation/epoch-host-before-native.json) / [during](combined/native-confirmation/epoch-host-during-native.json), Python [initial before](combined/python/host-before-python.json) / [during](combined/python/host-during-python.json), and Python [confirmation before](combined/python-confirmation/host-before-python.json) / [during](combined/python-confirmation/host-during-python.json). The initial native monitor starts 1.23 seconds after elapsed begins. [COPY_INDEX.json](COPY_INDEX.json) records exact source paths and hashes for the copied reports/controllers/readers. [LOCAL_RAW.md](LOCAL_RAW.md) identifies retained full workers; no public raw download, binary, corpus or cache is implied.

## Readiness and remaining limits

The [completed normal qualification summary](combined/qualification/summary.json) binds 483 passing Rust tests in 35 result groups and all seven ordinary commands. [Original API and C checks](combined/qualification/api-c-readback.json) retain all [4,740 API rows](combined/qualification/api/results.json): **4,349 passes, 391 failures, no timeouts**, including both original 2 GiB cases at their unchanged three-second bound. All six C consumers pass. Their C ASan/UBSan instrumentation links uninstrumented normal Rust libraries and disables leak detection.

[Strict shared/static CPython](combined/qualification/strict-semantic-readback.json) each retain 802 methods, 809 rendered outcomes, raw exit 2 and the same `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` grouping failures. Two separate semantic checks pass per linkage. Strict consumers include the pinned cleanup backport; benchmark consumers do not. [Retry diagnostics](combined/qualification/allocation-retry/saved-readback.json) pass 300 rows with [25 local ceilings raised to 512](combined/qualification/allocation-retry/retry-ceilings.patch), stopping at first success. [Buffer-tail diagnostics](combined/qualification/buffer-tail/saved-readback.json) pass 12 rows with [two allocation-result assertions observed instead of required](combined/qualification/buffer-tail/diagnostic.patch); all remaining assertions are unchanged. These diagnostics neither reclassify the 391 failures nor prove allocation schedules or exhaustive OOM coverage.

All [18 declaration callback rows](combined/qualification/callbacks/readback.json) match the qualified standalone declaration semantics. [W3C](combined/qualification/w3c/readback.json) retains all [6,003 rows per engine](combined/qualification/w3c/summary.json): 4,962 mandatory passes, **960 mandatory failures** and 81 optional observations each, raw exit 1. Acceptance matches Expat on every row. The [960 shared catalog failures](combined/qualification/W3C-classification.md) comprise 954 Fifth Edition name-profile fixtures against Fourth Edition C rules and six version/BOM/declaration cases. The 48 diagnostic corrections versus raw-view are preserved; 198 error-code and 1,695 byte-index differences remain. This is acceptance-focused evidence, not full payload equivalence or conformance.

[Current-source Rust ASan/fuzz](combined/qualification/asan/readback.json) passes all six harnesses: **69,201 initial inputs, 69,204 replay executions and 8,206,283 exploration executions**, with zero failure artifacts. All 22 commands exited zero and were reaped; compiler evidence verifies the three Oriole crates and six harnesses were instrumented. Each exploration has a 600-second bound, ten-second input timeout, 1,536 MiB RSS limit and 65,536-byte input cap. Leak detection is disabled. The preserved [first launch](combined/qualification/asan/launch-attempt01.json) failed on an interpreter-path typo before the runner or any target; the authorized correction changed only that path.

[CI](combined/qualification/ci/readback.json) passes all 15 ordinary jobs on final `5d983f7e`, with PGO skipped; both Miri callback modes pass their 15 focused checks. PR169 and PR170 also pass 15 jobs each. The [final CI run](https://github.com/astral-sh/oriole/actions/runs/34722470510) is tied to the exact source commit.

The [installed PBS readback](combined/qualification/pbs/readback.json) verifies a generic, non-PGO CPython 3.12.13 distribution built from 76 source files matching `5d983f7e`. Archive structure, installed identity, 22 custom checks (five skipped) and **1,024 threaded parses on glibc 2.17** pass. The [PBS workflow remains failed](https://github.com/astral-sh/oriole/actions/runs/34722487535): host XML has 806 executions/four failures including retries/12 skips; glibc has 802 executions/two failures/13 skips. Every failure is one of the same two grouping methods, with exact prior/local assertions. The installed recipe contains only the disclosed upstream CPython cleanup backport; no installed speed comparison was made.

The accepted trial permits the documented callback-grouping and diagnostic/position differences. Oriole remains experimental, with intentional allocation/resource-policy differences and individual performance outliers. These completed bounded checks support this controlled opt-in trial; they do not certify exhaustive memory safety, complete conformance or a fully interchangeable production replacement.
