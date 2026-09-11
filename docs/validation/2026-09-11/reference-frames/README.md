# Direct scalar-reference adapter frames

**Selected, with an ordinary native performance cost.** The candidate improves PGO native real-project time by 2.03% and substantially improves the generated reference workload. Ordinary native time regresses by 0.79%, with 18 of 24 real conditions adverse. Ordinary CPython time improves by 1.03%, with 22 of 24 conditions faster. PGO CPython time is effectively flat (0.17% faster). These results do not meet the overall goal of staying within roughly 10% of Expat.

Measured candidate: `0f66d54ac8418f0a6e628ad18570677a19c9ed45`; source-equivalent restack: `8b00d4cbb3f540647e1d7d795d0a04df00ed92f4`. All 72 measured source hashes remain exact after restacking onto the preceding documentation layer. Selected comparison runtime: `a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9`. The selected runtime change is confined to `crates/oriole/src/lib.rs`, with focused adapter tests. The full source comparison also includes the inherited long-default C integration regression; that fixture is not benchmark-driver code.

## Measurements

Ratios above one mean slower. Ordinary and PGO campaigns use their matching Oriole and Expat controls and remain separate.

| Build | Native real / selected | Native real / Expat | Generated / selected | Adverse real conditions |
| --- | ---: | ---: | ---: | ---: |
| Ordinary | 1.007887× | 1.594148× | 0.838334× | 18 / 24 |
| PGO | 0.979686× | 1.326116× | 0.773533× | 2 / 24 |

The largest ordinary real regression is Wayland at 64 KiB with namespaces, +3.32%. Both PGO real regressions are Wayland at 4 KiB, about +0.21–0.22%. The generated entity/reference fixture improves by 30.75–31.43% ordinary and 39.43–39.63% PGO; both ordinary rare-declaration controls regress by about 2%. Generated gains do not establish faster real projects generally.

| Build | CPython consumer | Candidate / selected | Candidate / Expat | Adverse conditions |
| --- | --- | ---: | ---: | ---: |
| Ordinary | All | 0.989673× | 1.264743× | 2 / 24 |
| Ordinary | ElementTree | 0.988155× | 1.347298× | 1 / 12 |
| Ordinary | pyexpat | 0.991194× | 1.187246× | 1 / 12 |
| PGO | All | 0.998270× | 1.117937× | 10 / 24 |
| PGO | ElementTree | 0.994852× | 1.162278× | 3 / 12 |
| PGO | pyexpat | 1.001700× | 1.075288× | 7 / 12 |

The packet retains all 56 native and 48 Python condition rows, including every regression. The two ordinary Python regressions are Maven at 4 KiB in pyexpat and Batik at 64 KiB in ElementTree. The PGO pyexpat subgroup is within 1.10× Expat; aggregate CPython and native performance remain outside that target. These are parser/consumer timings on one machine, not whole-application benchmarks or an identified hardware mechanism.

Selection accepts the disclosed ordinary native cost because this small change reuses an existing frame, improves the other measured aggregates or leaves them effectively flat, and preserves the measured compatibility and allocation behavior. This is a judgment about the complete tradeoff, not a predeclared numerical acceptance rule.

## Change and validation

Successfully decoded predefined and numeric scalar references now publish the existing detached text frame directly. This avoids constructing and queueing a pending owned event. Scalar validation, raw reference spelling, positions, accounting order and callback boundaries remain in place. General/external references and ordinary owned-event parsing retain their existing paths.

Both text representations already store these small scalars inline. This change does not establish one fewer heap allocation per reference. The reused frame copies its one-to-four decoded bytes into existing inline storage; it does not retain a temporary buffer or represent decoded bytes as an original-input range.

The committed source passes 440 Rust checks across 34 groups, formatting, all-target checking and Clippy. Focused tests compare owned/adapted events, positions, raw spellings and errors across chunk widths, UTF-8/UTF-16, token limits, valid and malformed references, illegal scalars and following text. They also check detached ownership after parser destruction and no frame publication on accounting failure. Independent review found no actionable source defect; existing callback lifecycle tests cover the reused dispatch path.

All 4,740 upstream API outcomes match the selected suffix: 4,347 pass, 391 assertion failures and two timeouts. The API suite exits 1. Six C consumers pass, including the 327-scenario allocation-failure sweep for each linkage. Their sanitizer coverage is C ASan/UBSan; the optimized Rust library is uninstrumented and leak detection is disabled.

Strict CPython remains unchanged: each shared/static suite reports 802 tests, two failures and 14 skips and exits 2. All 809 rendered outcome lines match the corresponding suffix baseline, including expected failures; rendered lines are not distinct test counts. The remaining failures are `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Only strict consumers receive the pinned pyexpat allocation-failure cleanup backport. There is no text-fragmentation allowance, and performance extensions remain unmodified.

All twelve fixed allocation diagnostics match the selected raw traces except library-origin paths: histograms, callbacks, peaks, retained bytes/blocks, all 74 feed records per engine, and zero live allocations after destruction. These fixed 4 KiB fixtures do not establish identical allocation schedules on other inputs or measure RSS.

## Method and evidence

Both Oriole modes use O3, ThinLTO and one codegen unit with the original selected allocator. Fresh PGO training uses only the original generated corpus: 288 matching records in normal, instrumented and optimized builds. Held-out real inputs do not train profiles. CPython extensions use identical O2 compilation; Expat uses its matching pinned ordinary/PGO controls. No alternate allocator or additional optimizer configuration is composed into this change.

Each condition uses seven seeded, interleaved process pairs. Ratios are medians of paired process-median ratios; aggregates are geometric means across conditions. Python creation, feed, finalization, callbacks and destruction are timed; file reads, imports, canonical validation and explicit garbage collection are outside, with automatic collection enabled. Canonical checking coalesces adjacent text callbacks. Native workers retain their original explicit library loading and hash checks; Python workers additionally record module and parser-library origins.

The archive retains source/check/build records, all raw workers, original audit reports, source/compatibility reviews, API and strict logs, allocation traces, and copied corpus licenses. It preserves initial source-review metadata and the corrected committed patch, the preparer's descriptive correction, and the review note correcting an assumption about ElementTree's direct Expat linkage. Original receipts remain distinguishable from their corrections.

The completed bundle contains 2,496 worker records and 265,080 samples: 1,344 native workers / 212,352 samples and 1,152 Python workers / 52,728 samples. These totals include preflights and warmups. All 104 condition rows appear in the two condition CSVs. `allocations.csv` contains the fixed allocation comparisons.

`evidence.tar.gz` contains saved records; `index.json` identifies every member by original path, size and SHA-256. Compiled libraries, executables and binary profiles are excluded. The original audits checked library bytes and recorded their identities; archive readback verifies the saved evidence, not an absent binary. Original absolute paths describe the measuring machine, and archived controllers/readers remain historical records rather than a newly portable verification framework.

The compatibility review note preserves the corrected ElementTree linkage assumption; the initial failed ad-hoc tool output remains in the conversation and is not a separate archive member. `_elementtree` obtains Expat through pyexpat’s `expat_CAPI` capsule. Generic C consumers retain compile/ELF binding evidence rather than an added in-process `dladdr` probe. No parser, compiler or benchmark target was executed while packaging.
