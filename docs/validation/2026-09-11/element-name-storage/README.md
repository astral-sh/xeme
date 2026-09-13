# Element-name storage

The selected candidate is measured runtime `a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9`. It reduces PGO time by 1.64% across the real-project native suite and by 0.61% through CPython, compared with prior runtime `de859c89577b2982d59b6923d682032f6a7c7800`. It takes 1.3516× Expat time natively and 1.1281× through CPython. Both overall results remain above our target of roughly 1.10×; the project remains experimental.

| Candidate | Build | Real XML / prior runtime | Generated / prior runtime | Real XML / Expat | Adverse real conditions |
| --- | --- | ---: | ---: | ---: | ---: |
| Raw first (`1e6c707`) | Normal | 0.9971× | 1.0126× | 1.5838× | 12/24 |
| Raw first (`1e6c707`) | PGO | 0.9962× | 1.0139× | 1.3644× | 13/24 |
| Expanded first (`a0acaf1`) | Normal | 0.9982× | 1.0050× | 1.5821× | 15/24 |
| Expanded first (`a0acaf1`) | PGO | 0.9836× | 1.0027× | 1.3516× | 6/24 |

Ratios above one mean slower. The first representation remains rejected: its small aggregate improvement and mixed condition results did not justify selection. These are separate campaigns paired against the same prior-runtime libraries and Expat controls; they are not direct paired comparisons between candidates. [conditions.csv](conditions.csv) retains all 112 rows, including regressions and generated controls.

The measured source was restacked as publication runtime `00f0338e8eff185f72ee7304a6c8f8f66860cd04`. The [publication source receipt](publication-source.json) records all 72 source files as byte-identical to measured commit `a0acaf1`; the native and CPython measurements remain unchanged.

## Changes

An open element previously kept its raw name and a separate expanded name when namespace expansion changed its spelling. The expanded-first candidate keeps one owned string on the element stack. The expanded name comes first. If the raw name already forms its suffix, both names share those bytes; otherwise, the raw name is appended. Start callbacks retain their own event name. End-tag matching reads the raw slice, and both owned and detached end events take the stack string after truncating its length to the expanded name. Truncation avoids allocating or moving the name bytes.

For example, the default namespace spelling `urn|item` already ends with raw name `item`. A prefixed raw name such as `p:item` is appended when it is not a suffix. Equal raw and expanded spellings keep a single spelling with a zero raw offset. The implementation reserves checked expanded length, optional raw length and a spare terminator byte before copying. Original custom-encoding byte identity remains a separate check, so different source bytes that decode to the same character still produce a tag mismatch.

The earlier representation stored raw bytes first and expanded bytes second. End events then moved the expanded bytes to the front. Its complete native records remain in this packet as the rejected comparison.

The two existing name-recycling slots and their 4 KiB retention limit remain. Combined spellings can exceed that limit, and actual allocated capacity matters. The candidate adds a suffix comparison on differing spellings and can copy a decoded custom-encoding name into the stack owner. It trades fewer malloc calls for a different allocation and reallocation schedule. No elapsed benefit follows from allocation counts alone, and this packet does not establish an exact Rust struct-size change.

## Allocation observations

These are calls to the embedding allocator through a fixed C diagnostic, not process RSS. All twelve candidate/selected fixture pairs preserve callback hashes and feed progression, finish with balanced element callbacks, and release every tracked allocation after parser destruction. [allocations.csv](allocations.csv) includes every pair.

| Fixture | Selected malloc / realloc | Candidate malloc / realloc | Selected peak tracked bytes | Candidate peak tracked bytes |
| --- | ---: | ---: | ---: | ---: |
| Maven, namespaces enabled | 1,297 / 26 | 380 / 204 | 93,501 | 92,359 |
| Flat repeated children, namespaces enabled | 127 / 3 | 0 / 3 | 16,124 | 15,906 |
| Empty repeated children, namespaces enabled | 127 / 3 | 0 / 3 | 14,783 | 14,565 |
| Nested children, namespaces enabled | 465 / 3 | 225 / 3 | 29,999 | 27,946 |
| 2,044-byte raw names | 15 / 4 | 0 / 4 | 97,472 | 95,162 |
| 2,045-byte raw names | 15 / 5 | 0 / 5 | 163,013 | 160,702 |

Counts cover phase 2: the whole parse for Maven, and parsing after an initial warmup for generated fixtures. Peaks cover the whole diagnostic. Maven's total malloc/realloc requests decrease from 1,323 to 584, while realloc calls increase from 26 to 204. The equal-spelling NUL-separator fixture already needed no phase-2 malloc calls in the selected implementation. Triplet callbacks also remove all 127 post-warmup malloc calls.

The two long-name fixtures straddle an input-buffer growth boundary. Compare each candidate only against the identical selected fixture; the difference between their absolute peaks does not isolate the name-cache cutoff. Tracked retained and peak bytes come from the inspected allocator tracker, not an address-level trace or an RSS measurement.

## Correctness and compatibility

Both representations pass 438 Rust checks across 34 groups, Clippy and formatting. The expanded-first test extension exercises 96 combinations: four original-byte spellings for each name, namespace processing disabled or enabled with prefixed/default names, and one-byte or whole-document input. It asserts Start and End names and rejects unequal original encodings even when they decode to the same character.

The expanded-first candidate reproduces all 4,740 original upstream API outcomes: 4,347 pass, 391 assertion failures and two timeouts. The original assertions, retry ceilings and resource limits remain in force. Unchanged failures are not successful compatibility results, and an early assertion can still hide later behavior.

Six C consumer runs pass: integration, adversarial and allocation checks, each linked dynamically and statically. Their C code uses AddressSanitizer and UndefinedBehaviorSanitizer; the Rust release library is uninstrumented, and leak checking is disabled. These runs do not establish full Rust sanitizer coverage. The earlier raw-first candidate has no CPython or full API/sanitizer result in this packet.

## CPython

The [separate CPython packet](cpython/README.md) retains all 48 consumer benchmark conditions and fresh strict shared/static runs. PGO takes 1.1281× Expat time overall: 1.1816× for ElementTree and 1.0771× for pyexpat events. Seven of 24 conditions are within 1.10×. Both strict linkages still report 802 tests, two failures and 14 skips; their rendered outcomes match the historical Context record. The expanded-first candidate is selected as a modest improvement for the PR stack, with the overall performance and production-readiness goals still unmet.

## Measurement

Each mode contains 24 real-project conditions: Vulkan, Wayland, Maven, Batik, GTK and DocBook, each with 4 KiB and 64 KiB chunks and namespaces disabled or enabled. Four generated conditions remain separate. Each condition uses seven seeded, interleaved paired comparisons, with one warmup per worker. Condition ratios are medians of paired process-median ratios; aggregate ratios are geometric means across conditions.

All Oriole builds use O3, ThinLTO and one codegen unit. Expat uses GCC O3 without LTO. Each candidate has a fresh profile trained on the original generated corpus, with 288 matching parses in each normal, instrumented and optimized build. Real-project XML remains outside training. Alternative allocators, CPU-specific builds and other compiler experiments are not composed into these candidates.

All 2,688 native workers completed and all 424,704 samples were retained. Timing workers ran on CPU 0 after build targets completed. Independent saved-data reviews reconstructed worker samples, callback observations, seeded order and arithmetic. These results describe this machine and protocol; they do not establish a portable speedup across hardware.

## Recompute the saved results

```console
python3 verify.py > recomputed.json
```

The portable reader checks the archive and all member hashes, verifies the 72-file source snapshots and their two-file differences from the selected source, checks local test records, reconstructs all native workers and ratios, rebuilds allocation phase counts and histograms, and checks every upstream API outcome plus the six C consumer receipts. It does not extract or execute archived files. The same command works after copying this directory elsewhere.

[evidence.tar.gz](evidence.tar.gz) retains native records and controllers for both representations, source snapshots, build logs and profile manifests, local checks and independent reviews, selected/candidate allocation diagnostics, and expanded-first API/C consumer records. [files.json](files.json) indexes its 3,275 members. Compiled libraries, benchmark executables, compiler installations and build caches are represented by their recorded identities rather than copied into Git. The portable check verifies saved evidence; it does not rehash the original binaries, rebuild them, or rerun parsers. Historical paths describe the original machine and are not portable execution instructions.
