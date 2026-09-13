# Selected-runtime validation refresh

Fresh sanitizer campaigns and a stable PGO python-build-standalone distribution now cover the selected reference-frame runtime, measured as `0f66d54ac8418f0a6e628ad18570677a19c9ed45` and published as `4064b0653534aff690c0e0f395a6b3b48da878fc` in [PR #144](https://github.com/astral-sh/oriole/pull/144). This layer changes documentation and retained evidence only.

| Validation | Result |
| --- | --- |
| Fresh Rust ASan builds | All three workspace crates and seven harnesses instrumented |
| Focused instrumented regressions | 15 passed |
| Retained-corpus replay | 71,823 executions |
| Seven 600-second fuzz campaigns | 9,939,415 executions, no findings |
| Stable PGO PBS build and distribution validator | Passed |
| Installed parser and glibc 2.17 threaded probe | Passed, 1,024 parses |
| Strict host and glibc 2.17 XML suites | Same two callback assertions fail |

## Sanitizers

The [ASan report](asan/ASAN.md) retains exact source, compiler vectors, instrumented machine-code readbacks, complete logs, every initial/final corpus and its ancestry, and independent reviews. The seven campaigns use seed 20260911, 65,536-byte maximum inputs, ten-second input timeouts and a 1,536 MiB RSS limit per process. The local ASan workers completed and were reaped before subsequent local compiler or parser runs.

Leak detection is disabled. These results do not establish LSan, UBSan, whole-standard-library coverage or performance. Historical campaign counts are ancestry, and are not added to the current execution total.

## CPython distribution

The [PBS report](pbs/) records a freshly trained stable Rust 1.98.1 / LLVM 22.1.8 PGO build, the exact shipped static archive, source inventories, generated training records and profiles. The [workflow](https://github.com/astral-sh/oriole/actions/runs/34653585245) remains failed because both strict XML gates retain `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Its standalone distribution validator, installed identity and glibc 2.17 threaded probe pass.

Host totals include 802 initial methods and two failures, followed by four retry executions repeating both failures. Old-glibc results contain 802 methods and two failures without those retries. The report preserves the resource/skip differences and does not infer a complete method outcome map from aggregate logs. The explicit upstream `Modules/pyexpat.c` cleanup backport is unchanged; CPython tests are unchanged.

The compact PBS packet preserves the original validation ZIP, raw logs, profiles and licenses, and excludes compiled binaries and the full distribution. The [package readback](pbs/package-readback.json) and [root readback](pbs/root-review.json) separately verify archived data. Root's [first reader failure](pbs/root-review-first-failure.json) came from comparing four explicitly compressed members directly with their uncompressed origins; the corrected reader checks their gzip transformations. No artifact or producer was rerun.

## Remaining gates

Oriole remains experimental. The original API matrix still contains 4,347 passes, 391 assertion failures and two timeouts; the [failure census](../api-failure-census/) explains their reached assertions. Current [PGO benchmark aggregates](../reference-frames/) remain 1.3261× Expat natively and 1.1179× through CPython, above the roughly 1.10× target. This refresh adds no elapsed benchmarks and does not select the unmeasured coordinate prototype or extend deployment coverage beyond this opt-in Linux x86-64 trial.
