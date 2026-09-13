# Rejected serialized accounting experiment

**Decision: rejected.** The candidate improves PGO native time by 1.34% and PGO CPython time by 1.10%, but its ordinary release build regresses every real-project native condition, by 4.48% overall, and 23 of 24 CPython conditions, by 3.22% overall. The existing suffix-sharing runtime remains selected. These results do not justify adopting the additional accounting mode.

Candidate: `bcf15427e861b832f2ec53a57a765c2c4ecb7165`. Selected comparison: `a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9`. The source snapshots and build records identify the measured runtimes; publication adds documentation only.

## Measurements

Ratios above one mean slower. Normal and PGO are separate, interleaved three-engine campaigns against their matching selected Oriole and Expat libraries.

| Build | Native real XML / selected | Native real XML / Expat | Generated / selected | Adverse real conditions |
| --- | ---: | ---: | ---: | ---: |
| NORMAL | 1.044827× | 1.656989× | 1.027132× | 24/24 |
| PGO | 0.986563× | 1.333447× | 1.017436× | 5/24 |

| Build | CPython consumer | Candidate / selected | Candidate / Expat | Adverse conditions |
| --- | --- | ---: | ---: | ---: |
| NORMAL | All | 1.032197× | 1.307691× | 23/24 |
| NORMAL | ElementTree | 1.031164× | 1.393158× | 11/12 |
| NORMAL | pyexpat | 1.033230× | 1.227467× | 12/12 |
| PGO | All | 0.989043× | 1.117173× | 7/24 |
| PGO | ElementTree | 0.996034× | 1.173634× | 5/12 |
| PGO | pyexpat | 0.982100× | 1.063429× | 2/12 |

[Native conditions](conditions.csv) retain all 56 rows. [CPython conditions](cpython/conditions.csv) retain all 48 rows. Neither aggregate hides individual regressions or generated-input results. The PGO candidate still misses the roughly 1.10× Expat aggregate target in both native and CPython measurements; the faster pyexpat subgroup does not establish the goal for all consumers.

## What was tested

The C adapter uses a serialized accounting mode with `Cell` counters where parser-family access is already serialized. The public Rust parser keeps atomic accounting and its existing thread bounds. The modes share checked arithmetic, resource limits, child ownership and DTD table handling. Factor and threshold setters remain atomic. The implementation changes nine source files and the nominal core parser type; compiler recipes and the embedding allocator are unchanged.

Independent source review checked overflow behavior, failed charges, tightened limits, family lifetimes and DTD scope restoration. Follow-up coverage exercises the existing child-ownership and error/unwind restoration assertions in both accounting modes, retaining atomic thread-contention coverage. Source-review notes distinguish the initial review from final documentation and test changes.

The final source passes 440 Rust checks across 34 groups, Clippy and formatting. Final check attempt 03 is bound to the compiled source; preceding checks and preparation mistakes are preserved. Those earlier records are not represented as final-source validation.

All twelve fixed C allocation diagnostics match the selected implementation: allocation/reallocation/free counts, size histograms, each feed's live bytes and blocks, peak and final retained bytes, callback hashes and feed progression. Every tracked allocation is released after parser destruction. [allocations.csv](allocations.csv) retains each pair. These are embedding-allocator observations, not process RSS or evidence of an elapsed speedup.

Static inspection of the measured normal and PGO libraries confirms that `account_source` and `charge_expansion` replace their counter compare-and-swap instructions with ordinary stores. Reference-count and allocation-tracking atomics remain; their storage source files are unchanged. Inlining and outlined helpers differ, so these regions do not establish that callees avoid atomics or explain elapsed results. PGO `.text` grows from 953,277 to 964,829 bytes (+11,552 bytes, +1.21%); the ordinary release `.text` shrinks by 560 bytes.

Layout probes using the checked debug artifacts report an unchanged 3,160-byte C handle and 2,408-byte core parser, each aligned to eight bytes. The candidate's serialized core is also 2,408 bytes. These probes do not measure the private heap `EntityBudget` layout. Saved assembly, symbol tables, probe source and raw outputs preserve the scope of these checks.

## Compatibility

All 4,740 upstream API configurations preserve the selected outcomes: 4,347 pass, 391 assertion failures and two timeouts. Original assertions, retry ceilings and resource limits remain unchanged. Matching failures are not passing compatibility results; an early assertion can hide later behavior.

Six C consumers pass: integration, adversarial and allocation checks with dynamic and static linking. Their C code uses ASan and UBSan; the Rust release library is uninstrumented and leak checking is disabled.

Strict CPython reports 802 tests, two failures and 14 skips for each linkage; both suites exit 2. All 809 rendered outcome lines match the selected suffix run, including three expected-failure lines omitted by the earlier 806-line comparison. These rendered lines are not a one-to-one test count. The remaining failures are `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Only these strict consumers receive the explicit CPython allocation-failure cleanup backport; measured performance extensions remain unmodified. No text-fragmentation allowance is enabled.

## Method and limits

Native conditions use six real XML projects, 4 KiB and 64 KiB chunks, and namespaces off/on, plus four generated controls. CPython conditions use the same six projects and chunk widths through ElementTree and pyexpat. Every condition uses seven seeded, interleaved process pairs. Condition ratios are medians of paired process-median ratios; aggregates are geometric means across conditions.

All Oriole builds use O3, ThinLTO and one codegen unit. Each PGO runtime is freshly trained on the original generated corpus, with 288 matching parse records in each normal, instrumented and optimized build. Held-out real inputs do not train the profiles. Expat uses the matching GCC O3 normal/PGO controls without LTO. CPython extensions use identical O2 builds. No alternate allocator, CPU-specific configuration or BOLT optimization is composed into this candidate.

Across both modes, independent saved-data reviews reconstruct 1,344 native workers and 212,352 samples, and 1,152 CPython workers and 52,728 samples. Creation, parsing, finalization, callbacks and destruction are timed; CPython input reads, imports, canonical output validation and explicit garbage collection are outside the interval. Automatic garbage collection stays enabled. These are parser/consumer measurements on this machine, not whole-application benchmarks or a hardware-mechanism claim.

## Saved evidence

[evidence.tar.gz](evidence.tar.gz) contains source snapshots, compiler/training records, raw native workers, allocation/API records, assembly/layout reports and original independent reviews. [files.json](files.json) indexes every archive member. The [CPython packet](cpython/README.md) retains all raw consumer workers and strict logs separately.

```console
python3 verify.py > recomputed.json
```

The portable verifier checks the archive, source snapshots, final check records and saved native arithmetic, allocation traces, API outcomes and recorded assembly/layout checks. The CPython packet has its own portable replay. Neither verifier imports parser binaries or reads original absolute machine paths. Compiler/library identities remain recorded evidence: the original independent reviews checked those binary bytes, which are not copied into Git. Replaying saved records does not rebuild or rerun a parser.
