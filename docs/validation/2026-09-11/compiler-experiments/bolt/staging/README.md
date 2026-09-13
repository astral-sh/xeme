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

## Publication preparation

This directory currently contains the report and a bounded packaging plan.
Archive assembly, compression, member readback, and portable numerical replay
are **pending the end of both v3 timing campaigns**. No archive completeness
claim is made by this staging receipt.

The plan retains original controllers and disabled preparations, all first
failures and warning stops, exact source and generated inputs, compiler/profile
provenance, fdata, smoke/ELF records, all native records and reviews, and the
Oriole, LLVM, Expat, and held-out corpus notices. The report author independently
audited the root-collected native samples and previously authored the allocator
correctness fix; the author did not collect or transform the BOLT measurements.
