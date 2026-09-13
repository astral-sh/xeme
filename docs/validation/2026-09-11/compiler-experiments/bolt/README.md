# BOLT layout experiment on the allocator-corrected Oriole build

**Experimental and unselected.** BOLT reduced elapsed time by **1.47%** across
the 24 real XML conditions; all 24 were lower than the original PGO library.
The four generated conditions were **2.00% slower** overall. Entity-heavy input
regressed **5.62% at 4 KiB** and **3.90% at 64 KiB**.

This study uses unchanged allocator-corrected source
`e1d263a711e862e4f0f0018b15de9ac83a87a342`. It contains no ContextText,
additional-real-training, bulk-training, or ISA variant. The original generated
G corpus supplied both the existing Rust PGO profile and the separate BOLT
instrumentation run. Held-out benchmark XML did not train BOLT.

## Native results

Each ratio uses medians of seven same-cohort worker ratios, then a geometric
mean across conditions. Lower is faster. These independently computed ratios
must not be multiplied or divided to derive another aggregate.

| Comparison | 24 real conditions | Lower | 4 generated conditions | Lower |
| --- | ---: | ---: | ---: | ---: |
| BOLT / original | 0.985293241 | 24 | 1.019985887 | 2 |
| BOLT / relocation relink | 0.951543823 | 24 | 1.022460166 | 1 |
| Relocation relink / original | 1.036910349 | 1 | 0.994037841 | 3 |
| BOLT / Expat | 1.371178211 | 7 | 5.091510119 | 0 |
| Original / Expat | 1.389819306 | 5 | 5.017736822 | 0 |

The first native campaign completed with 112 preflight workers and 784 timed
workers: 141,568 samples including preflight and warmup, 196 cohorts, 28
conditions, and seed `2026091003`. Independent saved-data reconstruction
checked every worker, seeded order, callback observation, sample count, median,
and all five ratios. The unchanged driver measures create, handler registration,
parse, callbacks, and free; it excludes process startup and library loading.
Explicit paths and before/after hashes identify its libraries; this legacy
driver does not record a fresh same-process `dladdr` origin.

## Warnings and bounded correctness evidence

The original instrumentation command exited successfully, but its controller
stopped before training after seeing the retained warning
`Failed to analyze 261 relocations`. The second warning changed the runtime
initialization hook to `init` because this shared library has no INTERP header.
Neither warning was removed from the evidence or silently accepted.

The follow-up classified all 261 relocations against LLVM revision
`ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`: 136 GD/LD displacement sites and 125
DTPOFF32 sites. It retained the exact LLVM source, relocation/symbol records,
TLS/GOT metadata, and review limitations. The already instrumented image was
then reused for one original-G 288-parse training run. Its fdata has 8,423
nonzero records with counter sum 1,205,334,940. The fixed `ext-tsp`/`cdsort`
rewrite passed generated replay and smoke checks. Source classification and
these checks do not prove every rewritten TLS instruction correct.

**Full API, CPython, PBS, and sanitizer gates have not run on the BOLT image.**
Callback equality in training, smoke, and native samples is scoped evidence.
Existing correctness results for the original library do not transfer to the
rewritten binary. This result is a measured lead awaiting those gates, not a
rejection for lack of a native gain.

## Artifact sizes

| Shared library | File bytes |
| --- | ---: |
| Original PGO | 1,291,616 |
| Relocation-preserving relink | 1,683,240 |
| BOLT output | 6,558,720 |

These are on-disk file sizes, **not RSS or runtime memory measurements**.
Binary and tool bytes are excluded from publication; their recorded hashes and
sizes remain in the evidence. Profiling data is retained separately from ELF
payloads.

## Evidence package and replay

The [archive](evidence.tar.gz) is 4,432,145 bytes with 1,205 stored payloads.
The [member index](members.json.gz) maps all 1,274 origins, including 69 explicit
same-byte aliases. It retains source, compiler/profile provenance, original
G inputs and replay, fdata, smoke/ELF metadata, native raw records, independent
reviews, and the required Oriole, LLVM, Expat, and held-out corpus notices.
There are no target caches, ELF/static-library bytes, or nested source archives.

Run this saved-data check from any location with Python 3.11 or newer:

```sh
python3 /path/to/package/verify.py /path/to/package
```

The [final readback](verification-attempt03.stdout) validates every stored and
alias hash, the 70-file source/67 Cargo inputs/six helper bindings, and replays
the original native auditor through archive-backed paths. It reproduces the
original detailed and compact numerical audit bytes exactly: 896 workers,
141,568 samples, all orders, callbacks, medians, and five ratio maps. It performs
no target execution and cannot revalidate omitted library/tool bytes. Its
output explicitly lists the five recorded-only binary hashes used by replay.

The [independent adapter review](independent-review/oriole-allocator-bolt-portable-replay-source-review.json)
checks source and AST preservation, not archive execution. The archived auditor
remains unchanged; the adapter changes only path I/O, recorded hash handling for
omitted binaries, and CPU affinity, with a fixed auditor SHA256 guard.

### Retained publication corrections

Assembly passed first attempt. The first portable replay failed because an
inherited `run.py` byte-comparison dependency was missing from the original
auditor's hash map. Its identical bytes were already stored. A
[one-alias index correction](index-correction.json) fixed that omission; the
archive never changed. Both corrected replays passed. The original index and
first failure logs remain, as does a separate review-helper AST-count mistake.

The final verifier adds a [one-line source identity guard](verifier-correction.json).
Its archived first version is retained, and an
[origin caption correction](origin-correction.json) points that historical
version to its exact saved copy. This changes no archived payload, path key,
hash, or numerical assertion. The independent source review's earlier index is
also retained under `attempts/`.

[report.json](report.json) records scope, all adverse conditions, command
outcomes, and final hashes. [files.json](files.json) hashes the complete published
directory except itself. `staging/` preserves the original pre-assembly report;
the archive contains that historical snapshot, while this top-level report
records completion. `assemble.py`, `repair-index.py`, `retarget-origin.py`, and
`finalize.py` retain the saved-only packaging steps. Reconstructing the first
archive uses its staged report and original archived verifier version; no new
measurement is implied by packaging reproduction.

The report author independently audited the root-collected native samples and
previously authored the allocator correctness fix. The author did not collect
or transform the BOLT measurements. This package is experimental and unselected;
the current ContextText candidate was not measured with BOLT, and no default
build recipe changes follow from this report.
