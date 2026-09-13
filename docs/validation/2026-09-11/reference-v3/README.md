# Optional x86-64-v3 compiler experiment

The optional v3 build reduces PGO time by 1.82% in native consumers and 1.17% through CPython. Its CPython aggregate is **1.0943× generic Expat**, within the approximate 1.10× target on this host. Only 11 of 24 individual Python conditions meet that threshold; ElementTree remains 1.1278× Expat. Native PGO remains 1.3011× Expat.

These measurements support evaluating an explicit CPU-specific option. The generic default remains unchanged. Normal Python improves by 1.70% but remains 1.2391× Expat. An actual stable python-build-standalone distribution requires separate validation. No adoption decision is recorded here.

## Results

Ratios measure elapsed time; lower is better. Generic Expat was declared as the primary comparator before collection. The native experiment also retains v3 Expat as a separate comparator, without choosing the faster Expat build after observing results.

| Campaign | v3 Oriole / generic Oriole | v3 Oriole / generic Expat | Conditions slower than generic Oriole |
| --- | ---: | ---: | ---: |
| Normal native, 24 real conditions | 0.969181× | 1.546263× | 1 / 24 |
| PGO native, 24 real conditions | 0.981833× | 1.301143× | 2 / 24 |
| Normal native, 4 generated conditions | 0.951509× | 3.914396× | 0 / 4 |
| PGO native, 4 generated conditions | 0.972193× | 3.748858× | 2 / 4 |
| PGO CPython, 24 conditions | 0.988273× | 1.094265× | 3 / 24 |
| Normal CPython, 24 conditions | 0.982954× | 1.239097× | 2 / 24 |

| PGO Python consumer | v3 Oriole / generic Expat | Conditions within 1.10× Expat |
| --- | ---: | ---: |
| ElementTree | 1.127756× | 5 / 12 |
| pyexpat callbacks | 1.061768× | 6 / 12 |
| Combined | 1.094265× | 11 / 24 |

All 56 native conditions and 48 Python conditions are retained in [native-conditions.csv](native-conditions.csv) and [python-conditions.csv](python-conditions.csv). [report.json](report.json) includes every adverse condition. Python regresses against generic Oriole in Wayland with 64 KiB chunks (+0.55%) and GTK with 4 KiB (+0.39%) and 64 KiB (+0.20%) chunks, all through pyexpat. Nineteen Python conditions are slower than Expat; thirteen exceed 1.10×. Maven with 4 KiB chunks through ElementTree is the largest gap, at 1.5261× Expat.

Normal Python regresses against generic Oriole only in Maven through pyexpat, at both chunk sizes. Twenty normal Python conditions exceed 1.10× Expat; only four meet that threshold. Its ElementTree and pyexpat aggregates are 1.3116× and 1.1706× Expat.

Native v3 Expat is 1.0011× its generic build normally and 1.0428× with PGO across real conditions. The corresponding v3 Oriole / v3 Expat ratios are 1.5484× and 1.2494×. Those comparisons are included for fairness; the primary results above continue to use generic Expat. No v3 Expat Python comparison was run.

## Build and measurement

Both Oriole arms use the same 72 source files: measured commit `0f66d54ac8418f0a6e628ad18570677a19c9ed45`, published unchanged as `4064b0653534aff690c0e0f395a6b3b48da878fc`. The single intended compiler change is `-Ctarget-cpu=x86-64-v3`. O3, ThinLTO, one codegen unit, allocator and original generated training remain unchanged. Fresh normal and PGO libraries preserve nine actual workspace compiler vectors and 864 generated parse records: 288 each for normal, training and profile use. PGO uses fresh Oriole profiles. Historical Expat v3 libraries were separately reverified from their exact source, compiler vectors, training records, profiles and bytes.

Each native mode uses four engines, 28 conditions and seven seeded process pairs: 112 preflights followed by 784 timed workers. Each Python mode uses three engines and 24 conditions: 72 preflights followed by 504 timed workers. Conditions use six original project XML inputs and 4 KiB and 64 KiB chunks on an AMD EPYC-Milan host. Python consumers are unmodified CPython 3.12.13 pyexpat and ElementTree extensions, compiled with identical O2 flags. Parser PGO does not imply PGO for the interpreter or extensions.

A condition ratio is the median of seven paired process-median ratios; group aggregates are geometric means across conditions. Native timing includes parser creation, callback registration, parsing and destruction. Python timing includes creation, feeding, finalization, callbacks and explicit destruction; imports, input reads and canonical checks remain outside. Python workers record actual module and parser-library origins. Native workers retain explicit library paths and hashes without a fresh same-process `dladdr` record.

The [bundle validation](bundle-validation/) builds normal generic, normal v3 and PGO v3 archives through the actual PBS preparation command. It checks all twelve workspace compiler vectors and the packaged PGO archive identity. These local Ohm builds validate the packaging option; the stable-toolchain distribution trial remains separate.

## Correctness and remaining scope

Fresh candidate PGO checks preserve all 4,740 original API outcomes: 4,347 passes, 391 assertion failures and two timeouts. All six C harnesses pass with ASan/UBSan; the linked Rust PGO libraries are not instrumented, and leak detection is disabled. Shared and static strict CPython runs preserve all 802 method outcomes and supplementary skips/failure headers, including `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`; both raw suite exits remain 2. These unchanged failures are not passes.

Strict CPython uses the existing explicit upstream `Modules/pyexpat.c` cleanup backport. The elapsed extensions use the unmodified source. The normal library passed generated, native and Python preflights. The full API/C/strict gate here covers the PGO artifacts. Prior generic-runtime sanitizer or PBS campaigns do not certify the v3 artifact.

This is one fixed campaign per completed mode on one host, using real XML files through small consumers, not whole applications. CPU portability, a stable PBS distribution and broad performance guarantees remain separate. The saved arithmetic reviewer also adapted the controllers and authored earlier focused C tests; no parser runtime changes were authored for this experiment.

## Saved evidence

All four campaigns completed before packaging. [evidence.tar.gz](evidence.tar.gz) retains source/build/training records, both native and Python campaigns, complete API/strict results, readers and failed preparation attempts. Its 6,163 members include all 1,792 native and 1,152 Python workers. [index.json](index.json) records original paths, sizes and SHA-256 digests; [package-readback.json](package-readback.json) verifies every archived member. The archive is 11.58 MB.

No compression ran during elapsed measurements. Executables, shared/static libraries, binary profiles and machine Cargo configuration are excluded with their recorded identities retained. Relative archive member names make the saved records portable; absolute paths identify their original machine. This is an auditable record bundle, not a self-contained executable benchmark.

The original build freezer inventoried its initially empty own stdout before printing success. Its original 153-entry inventory remains intact; the final stdout is verified against the printed count and original freeze digest, and all other 152 entries match. Failed saved-reader/preparation attempts are retained separately from actual target outcomes.
