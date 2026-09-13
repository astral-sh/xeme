# CPU targeting and BOLT experiments

Both experiments improved native parsing on the real-project corpus, but neither closes the performance gap with Expat. They remain experimental and are not selected for the default build.

| Experiment | Real-project time vs. its control | Generated time vs. its control | Decision |
| --- | ---: | ---: | --- |
| `x86-64-v3`, normal | −2.26% | −7.98% | Retain as an opt-in lead |
| `x86-64-v3`, PGO | −4.16% | −8.57% | Retain as an opt-in lead |
| BOLT after PGO | −1.47% | +2.00% | Retain as an opt-in lead; TLS warnings and generated regressions remain |

Each study uses the allocator-corrected `e1d263a` runtime, before the [Context Text change](../context-text-frame/). The variants were measured separately: these percentages cannot be combined or transferred to the current runtime. Both use the original generated training corpus, with real-project XML held out of training.

## CPU targeting

The [fixed `x86-64-v3` study](x86-64-v3/) uses `-Ctarget-cpu=x86-64-v3` for Rust and `-march=x86-64-v3` for Expat, in every corresponding build phase. It includes both normal and PGO builds, with fresh PGO profiles from the unchanged training schedule. Rust retains O3, ThinLTO and one codegen unit; Expat retains GCC 13.3, O3 and no LTO. This measures one ISA baseline, not a sweep of host-specific flags.

PGO makes the choice of Expat control material:

| Real-project comparison | Time ratio |
| --- | ---: |
| Generic Oriole / generic Expat | 1.3820× |
| v3 Oriole / v3 Expat | 1.2599× |
| v3 Oriole / generic Expat, post-hoc | **1.3263×** |

Expat's v3 PGO build was 5.36% slower than its generic PGO build in this campaign. The post-hoc comparison therefore uses the faster Expat control. It is reconstructed from the original paired worker samples, not derived by multiplying aggregate ratios. Oriole wins six of 24 conditions against that control and remains 32.63% slower overall.

The narrower CPU requirement needs an explicit distribution target. These results do not establish performance or compatibility for another processor, a combined Context Text build, or a PBS distribution. Full API, CPython and sanitizer gates have not run on the v3 artifacts.

## BOLT

The [BOLT study](bolt/) applies LLVM 22.1.8 `ext-tsp` block ordering and `cdsort` function ordering to a relocation-preserving relink of the original PGO library. A separate instrumentation run uses the same generated XML training corpus. The native benchmark compares the original library, the relink, the rewritten library and PGO Expat within each cohort.

BOLT time is 4.85% lower than the relocation-preserving relink and 1.47% lower than the original library. The relink measured 3.69% higher than the original; these paired-median ratios are computed independently. The rewritten library remains 1.3712× Expat's time on the real corpus. Generated entity input regresses 5.62% at 4 KiB and 3.90% at 64 KiB.

The original warning stop, analysis of 261 TLS relocations, generated replays and threaded allocator smoke checks are retained. Those checks do not establish correctness for every rewritten TLS instruction. Full API, CPython, PBS and sanitizer gates have not run on the BOLT image. Its file grows from 1.29 MB to 6.56 MB; runtime memory was not measured.

## Evidence and selected settings

The two packages retain all three native campaigns: 2,688 workers and 424,704 samples, including warmups and preflights. Each campaign covers 24 real-project and four generated conditions, seven paired cohorts and every adverse result. Independent readers reconstruct the saved sample medians, callback observations, seeded orders and ratios. Portable replay checks published data without loading a parser. Omitted libraries and tools have recorded identities; their bytes are not reproduced by replay.

These follow the earlier [PGO/LTO](../../../../benchmarks/results/2026-09-11/pgo-lto/), [C allocator](../../../../benchmarks/results/2026-09-11/c-allocators/) and [O2](../cdata-finder/rejected-o2/) experiments. The selected recipe remains O3 with ThinLTO and one codegen unit, with PGO opt-in. Fat LTO plus PGO and O2 plus PGO regressed. Alternate C allocators changed real-project time by less than 0.2% and increased peak resident memory.
