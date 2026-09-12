# Native ASCII Text plan

The combined Text plan reduces normal native elapsed time by **1.87%** and CPython time by **2.14%** against the published fused attribute scanner. The measured source matches all 74 files committed at `ea52004a6590a224bfc0ec96c8e80a3159920db5`. It remains experimental: native and Python aggregates are **1.4799×** and **1.1906× Expat**, above the roughly 1.10× goal. Current-source sustained fuzzing and an installed PBS trial are still outstanding.

## Implementation and source identity

A transient plan uses eight-byte integer classification (SWAR, not a claim of SIMD vector instructions) to find an eligible root native UTF-8 text boundary, prove ASCII XML validity and record an eager line/column delta. TAB, LF and DEL are accepted. CR, `]`, non-ASCII, invalid controls and ambiguous long spans use the unchanged fallback. Delimiters at the 65,536-byte cutoff retain the original precedence. Raw ownership, event publication, fallible accounting and source compaction keep their existing order; no persistent parser fields, unsafe code, allocator changes or dependencies are added.

Only `encoding.rs`, `lib.rs`, new `text.rs` and `text_coalescing.rs` change from parent `168b5f57`; published fused `tag.rs` stays byte-identical. [committed-source.json](committed-source.json) binds the measured precommit map to the later runtime commit, without claiming a rebuild after commit. Independent source reviews cover cutoff/fallback behavior, prior-CR handling, publication before late accounting errors, compaction and split-feed equivalence.

## Normal measurements

| Campaign | Candidate / control | Candidate / Expat | Favorable conditions |
| --- | ---: | ---: | ---: |
| Combined native, real XML | 0.981292× | 1.479945× | 20 / 24 |
| Combined native, generated | 0.973496× | 3.958944× | 2 / 4 |
| Combined CPython, all | 0.978588× | 1.190554× | 24 / 24 |
| Combined ElementTree | 0.973843× | 1.250605× | 12 / 12 |
| Combined pyexpat events | 0.983355× | 1.133386× | 12 / 12 |
| Isolated Text native, real XML | 0.983090× | 1.513906× | 22 / 24 |
| Isolated Text native, generated | 0.970045× | 3.911943× | 3 / 4 |

The isolated experiment used publication `4815cf78` / library `a240099f` as its control and kept the original attribute scanner. Its native campaign is retained separately; isolated Python was not run. The combined experiment uses the published fused control `dca62e60`. Cross-campaign ratios are not paired measurements and gains should not be added.

Combined real-input regressions are DocBook 4 KiB namespaces off **+3.29%**, DocBook 4 KiB namespaces on **+2.83%**, DocBook 64 KiB namespaces off **+1.68%**, and Batik 64 KiB namespaces on **+0.24%**. Generated rare declarations regress **+1.95%** at 4 KiB and **+0.70%** at 64 KiB. Both generated entity conditions improve, so the generated aggregate improves while remaining much slower than Expat. All six adverse combined rows, every favorable row and all three isolated adverse rows are retained in CSV and raw records.

The README sample uses 4 KiB feeds and namespaces off:

| Project XML | Oriole (normal) | Expat (normal) | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 47.516 ms | 28.622 ms | 1.645× |
| Wayland protocol | 1.097 ms | 1.064 ms | 1.033× |
| Maven POM | 0.824 ms | 0.440 ms | 1.846× |
| Batik SVG | 0.133 ms | 0.135 ms | 0.984× |
| GTK UI | 0.350 ms | 0.212 ms | 1.633× |
| DocBook XSL | 0.271 ms | 0.184 ms | 1.464× |

Times are medians of seven process medians; displayed ratios are medians of seven paired process ratios. Ratios need not equal division of the displayed times. Full aggregates are equally weighted geometric means of condition ratios.

The same shared Linux AMD EPYC-Milan host and normal generic release recipe are used: local Ohm rustc 1.98.1-dev (ohm-1.98.1-1), `-Zohm-defaults=no`, O3/ThinLTO/one codegen unit; Expat 2.8.4 uses GCC 13.3 O3 without LTO. CPython 3.12.13 extension modules use the unchanged O2 recipe and unmodified sources. Four control/Expat modules are reused with their original bytes/vectors/origins pinned; two candidate modules are freshly built. Every engine gets fresh preflights and timed workers.

Native preserves 28 conditions, seven seeded pairs, 84 preflights and 588 timed workers: 672 workers / 106,176 samples per campaign. Python preserves 24 conditions, seven seeded cohorts, 72 preflights and 504 timed workers: 576 workers / 26,364 samples. Combined totals are 1,248 workers / 132,540 samples; the isolated native campaign adds 672 / 106,176. The faithful C driver times full parser lifecycle. Python times construction/feed/finalization/callbacks and explicit destruction, with imports, input reads, canonical checks and gc.collect outside; automatic GC stays enabled. All original timeout/reap rules, input hashes and measurement boundaries remain recorded.

## Compatibility and limits

All **449 workspace tests** across 35 groups, formatting and strict Clippy pass. Original **4,740 API configurations** remain byte-for-byte unchanged: **4,347 pass / 391 assertion failures / two timeouts**, raw matrix exit 1. The same original resource limits and assertions apply. Six dynamic/static C integration, adversarial and allocation consumers pass under C ASan/UBSan; the normal Rust libraries are uninstrumented and leak detection is disabled.

Each strict CPython linkage retains **802 method outcomes and 809 rendered outcome lines**, raw exit 2, including `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. These strict consumers contain the explicit pinned upstream pyexpat allocation-failure cleanup backport, unlike the unmodified benchmark consumers. Two existing supplemental text-semantic tests pass per linkage; these do not turn strict failures green. Historical diagnostics that relax allocation schedules and prior sustained sanitizer/PBS evidence retain their older source identities and do not certify this runtime.

## Preparation history and review

The packet preserves the pre-format Text source and the one test-formatting change; an independent source-pin read encountered that formatting before the new manifest was available. The isolated build reader initially retained a stale fused target-directory path, then passed after that reader-only correction; actual libraries were unchanged. The combined correctness materializer initially relocated an old top-level author-script filename pin, then passed after preserving the original path. The Python preparation reviewer corrected a cached-versus-physical CPython source-path spelling. These were saved-data/preparation issues, not parser test failures; first records remain available. No target result was discarded or rerun to remove a regression.

The Text author prepared source/controllers and this packet. Root executed targets and the adapted saved readers. Other agents independently reviewed source/protocols and replayed all combined raw workers; final packet/README review is separate. Every stage preserves its original pending labels alongside later completion records. No current-source Callgrind profile, sustained fuzz campaign or installed PBS outcome is claimed.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 3,735 indexed files (7,701,073 compressed bytes). [index.json](index.json) records every member hash, original path, size and excluded executable/library identity. Every archived member and original was read back after writing. [conditions.csv](conditions.csv) includes all 80 combined/isolated rows; [adverse-conditions.csv](adverse-conditions.csv) includes six combined and three isolated regressions. [report.json](report.json) retains full precision and limits.

Archive SHA-256: `c32a50267365fe26ed182adcd0aa2bb8b03c08aa7f3c82cb9acdd8afef02c21c`. Executables, libraries, compiler profiles and machine configuration are excluded. The source member map reconstructs all four candidate/control snapshots without treating earlier source evidence as current.
