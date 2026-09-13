# Expanded-first element names: CPython measurements

The selected candidate reduces CPython PGO time by 0.61% against the prior runtime. It takes **1.1281× Expat's time** across the 24 conditions. The target of roughly 1.10× remains unmet overall; the pyexpat-event aggregate is within the target, while ElementTree remains slower. The [native study](../README.md) measures 1.3516× Expat with PGO.

## Results

| Parser build | Candidate / prior runtime | Candidate / Expat | Slower than prior runtime |
| --- | ---: | ---: | ---: |
| Normal | 0.988394× | 1.265576× | 4/24 |
| Generated-corpus PGO | 0.993886× | 1.128126× | 10/24 |

| CPython consumer | Normal / Expat | PGO / Expat |
| --- | ---: | ---: |
| ElementTree | 1.347968× | 1.181573× |
| pyexpat events | 1.188220× | 1.077096× |

Lower is faster. [conditions.csv](conditions.csv) retains all 48 comparisons and every adverse condition. Seven of the 24 individual PGO conditions are within 1.10× Expat. The PGO pyexpat-event ratio against the prior runtime is 0.999774×; that small difference does not establish an improvement in that consumer.

The measured candidate is `a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9`, compared with the prior selected runtime `de859c89577b2982d59b6923d682032f6a7c7800`. We select the expanded-first representation as a modest improvement for the PR stack. It remains experimental, and these measurements do not establish production readiness or completion of the performance goal.

## Measurement

Each build measures six original XML project files: Vulkan, Wayland, Maven, Batik, GTK and DocBook. Each uses 4 KiB and 64 KiB feeds and two actual CPython consumers. Seven seeded, interleaved pairs per condition use seed `202609104412`. Condition ratios are medians of paired process-median ratios; aggregate ratios are geometric means. All **1,152 workers and 52,728 samples** are retained, including 51,576 measured samples and predetermined warmups.

Timing includes parser creation, feeding, finalization, callbacks and explicit destruction. Input reads, imports, canonical validation and explicit garbage collection are outside timing; automatic garbage collection remains enabled. Canonical checks coalesce adjacent text callbacks. These runs parse project XML fixtures through CPython; they do not execute whole applications or check exact callback fragmentation.

Fresh extensions compile unmodified CPython 3.12.13 `pyexpat.c` and `_elementtree.c` at revision `3bb231a6a5dc02b95658877318bf61501a7209e9`, using identical O2 flags. These performance consumers have no cleanup backport or allocator override. Both Oriole builds use O3, ThinLTO and one codegen unit, with independently generated profiles trained on the original generated corpus. Project XML remains held out. Expat 2.8.4 uses the matching normal or generated-corpus PGO control.

Independent saved-data review checked all 12 extension compiler commands, actual module/library artifacts and 144 preflight identities, then reconstructed every raw warmup, measured sample, output hash, worker order, median and ratio. Both elapsed audits passed on their first attempts. Results describe this shared Linux AMD EPYC-Milan machine and protocol.

## Strict CPython compatibility

Fresh candidate PGO runs with shared and static linkage each report **802 tests, two failures and 14 skips**, with suite exit 2. The failures remain `test1` in `test.test_pyexpat.BufferTextTest` and `test_handlers` in `test.test_sax.CDATAHandlerTest`. No text-fragmentation allowance is enabled.

These strict consumers use the pinned upstream allocation-failure cleanup backport to `pyexpat.c`; benchmark consumers above use the original source. All 806 rendered pass/failure/skip lines per linkage match the same-linkage **historical Context** record. This is not a fresh strict comparison against the de859 runtime. The 806 rendered lines are not 806 tests; full original logs and their 802-test aggregate remain in the packet.

Strict import checks verify initial and fresh module identities and the accelerator imports. They do not record the runtime `XML_Parse` symbol origin. Benchmark preflights verify that symbol separately for their own consumers; those checks do not establish its origin in the strict-test subprocess. The strict suite remains non-green.

## Portable evidence

The packet contains complete original consumer-build and elapsed records, every raw compressed worker stream and specification, controllers, original independent reviews, both 72-file source snapshots and build metadata, and fresh strict logs alongside the historical Context logs. It omits 24 compiled binaries while recording their identities. Actual binary bytes were verified in the original local reviews.

From a copied package directory, with Python 3.12:

```console
python3.12 -I -S verify.py --package . --output readback-new.json
```

The verifier checks the archive, source inventories and benchmark bindings, validates saved extension compiler vectors, and replays all worker arithmetic from relocated records. It also reconstructs the strict rendered outcomes and compares the original aggregate lines. It imports no parser or compiled binary and does not read the original absolute paths. Those paths remain identity labels inside saved records. [index.json](index.json) lists all 3702 archived members and excluded binaries; [files.json](files.json) pins package files. This portable check does not reproduce the original compiler/library byte verification or rerun the parsers.
