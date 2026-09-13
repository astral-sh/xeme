# Current-source Rust ASan/fuzz

Six current-source Rust ASan/fuzz harnesses pass: 69,199 replay executions and 7,978,060 exploration executions, with no recorded failure artifacts. The replay starts with 69,196 retained/source-derived inputs across six harnesses; counts include the fuzz engine's execution accounting. This campaign qualifies the exact uncommitted nine-file source snapshot in the [source map](../../source/source.json), with PR165 base `2c4d21647efdefe0b7da977ca206b8552c32f0f3`. The [later source commit binding](../../source/published-source-binding.json) records `67c704c123b8661a3ad1f91bd48c2f0be4996096` with the tested bytes unchanged; campaign commands ran before that commit.

| Harness | Initial inputs | Replay executions | Exploration executions | Peak RSS (MiB) |
| --- | ---: | ---: | ---: | ---: |
| parse | 12,319 | 12,319 | 2,965,127 | 577 |
| streaming | 14,068 | 14,069 | 1,217,346 | 532 |
| ffi | 11,562 | 11,563 | 1,700,554 | 510 |
| ffi_family | 11,684 | 11,684 | 474,230 | 472 |
| multibyte | 14,257 | 14,257 | 1,099,933 | 489 |
| value_family | 5,306 | 5,307 | 520,870 | 463 |

## Instrumentation and bounds

The three Oriole crates and six harnesses have nine required Rust ASan compiler invocations, verified from the build log and [compiler proof](compiler-proof.json). External Clang supplies the ASan runtime. Each harness has 600 seconds of fresh exploration, a ten-second per-input timeout, a 1,536 MiB RSS limit and a 65,536-byte maximum input. Replay is bounded at 180 seconds, exploration at 690 seconds externally, build at 1,200 seconds and the whole campaign at 3,600 seconds. CPU affinities are 1, 2 and 4, with two sequential harnesses per lane. The build uses CPU3. These are affinity settings, not exclusive host isolation.

All 22 recorded commands exited zero and were reaped. The normal generic libraries serve only as provenance controls; this campaign executes separately built ASan harnesses. Leak detection is disabled (`detect_leaks=0`). The Rust standard library and system libraries are not claimed instrumented. A successful bounded campaign is not exhaustive memory-safety, OOM, callback-equivalence, coverage or installed-distribution proof; it makes no speed claim. Successful completion does not exercise timeout/signal cleanup or independently establish global absence of orphan processes.

## Saved evidence

- [Completed report](report.json), [saved reconstruction](readback.json), [released manifest](command-manifest.json) and [original held manifest](command-manifest-held.json).
- [Runner](runner.py), [reader](read.py), [command templates](commands.json) and [compiler proof](compiler-proof.json).
- [Rust compiler version](rustc.log), [cargo-fuzz version](cargo-fuzz.log), [Clang version](clang.log) and [build log](build.log).

Six instrumentation logs and all twelve replay/exploration logs are copied at the paths recorded in report/readback. Initial/final corpus contents and sanitizer binaries remain in the local study, outside this compact packet. The source/tool/corpus hashes and final identities remain in the manifest/report/readback. The reader was adapted by the protocol author and reconstructs saved results independently of root target execution; no independent runtime-authorship claim is made. The saved scripts retain original local paths and require the separately retained exact files to rerun their checks.
