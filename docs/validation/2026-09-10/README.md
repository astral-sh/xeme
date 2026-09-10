# Compatibility and validation: 10 September 2026

## Current evidence

The [final runtime package](final-runtime/) identifies parser revision `1262888` and fuzz harness `338dcd0`. It retains source and binary hashes, commands, raw observations, skipped tests, failures and independent reviews. Earlier checkpoints remain available with their own source identities.

| Gate | Result | Evidence |
| --- | --- | --- |
| Full Expat public API matrix | 3,753 pass, 987 fail; no signals/timeouts; 12 improvements over PR56, zero regressions | [Full matrix](external-grammar/), [allocation diagnosis](final-runtime/) |
| CPython 3.12.13 | Six XML modules succeed in four shared/static, original/fixed configurations; 803 reported tests, 31 skips per run | [Final consumers](final-runtime/) |
| Native callback and allocation probes | Six shared/static runs pass, including 353 allocation-failure scenarios per linkage | [Final native report](final-runtime/) |
| Generated differential corpus | 12,318 semantic comparisons pass; six exact-fragment and 450 final-position differences remain | [Final generated corpus](final-runtime/) |
| Three ten-minute Rust ASan campaigns | 3,912,477 executions without findings; 15,713 corpus input files and 84 stress replays | [Frozen source, corpora and audits](final-runtime/) |
| W3C acceptance corpus | 5,916 required checks pass, six fail; 81 optional observations, no resolver errors | [Full results](external-grammar/) |
| Final full PBS distribution | Build and installed-distribution gates in progress | [Dedicated build](https://github.com/astral-sh/oriole/actions/runs/34456274540) |

[Final benchmarks](../../../benchmarks/results/2026-09-10/final-runtime/) measure the same runtime through the C and safe Rust APIs, with namespace modes, alternate allocators and DTD scaling. Oriole remains about 5–11 times slower than Expat on the 4 KiB generated C workloads. Raw runs, semantic preflights, allocation counts and limitations are retained.

The [previous successful PBS archive](pbs-values/) identifies the earlier `b68bdca` runtime. Its archive validator, installed XML suites and actual glibc 2.17 baseline all pass. It is not relabeled as a result for the final runtime. The [earlier combined evidence package](combined-evidence/) retains its complete source and campaign history.

## Earlier validation layers

Each report identifies its own frozen source and binaries. Results below come from
successive implementations and separate test contracts; they are not combined into
one claim of Expat equivalence.

| Layer | Result | Evidence |
|---|---|---|
| Conditional DTD and multibyte input | 3,455 upstream API configurations pass; 1,285 fail | [Complete API matrix](multibyte/) |
| Actual consumers for that same runtime | Four CPython shared/static, original/fixed runs succeed with 803 reported tests and 31 skips each; six native C suites pass | [Consumer report](newfeatures-consumers/) |
| Malformed references, prolog, and deferral | 60,159 cases have no acceptance or normalized-callback differences; 39 error, three exact-fragment, and 2,307 position differences remain | [Expanded replay](prolog-deferral/) |
| Independent stop/resume review | 80 prior differences fixed; two new malformed-input callback differences and 242 existing differences retained | [Suspension review](prolog-suspension/) |
| Internal parameter entities in values | 256 status/error, 12 inheritance, and 23,328 encoding/chunk summaries match; focused upstream matrix gains 24 configurations | [Parameter-value report](parameter-values/) |
| C callback allocation optimization | 166 workspace tests and shared/static patched CPython consumers pass; isolated element benchmark improves about 12% | [Allocation and consumer evidence](../../../benchmarks/results/2026-09-10/callback-allocation/) |
| Ten-minute sanitizer campaigns | 1,165,436 family and 2,564,903 converter executions, plus corpus replays, finish without findings | [Frozen source, corpora, and logs](../../../fuzz/results/2026-09-10/converter-and-family/) |
| First complete PBS build | Distribution archive produced; validator setup failed before installed-interpreter gates ran | [Build evidence and fix](pbs-distribution-build/) |

[Namespace-enabled and disabled measurements](../../../benchmarks/results/2026-09-10/namespaces/)
retain complete preflights, seven paired runs, and separate mode results. Oriole
remains slower than Expat. The earlier PBS failures and their corrections remain
available alongside the subsequent successful run above. Current API limitations remain in the
[C interface documentation](../../../crates/oriole_expat/).

## Earlier checkpoint (PR #22)

These results identify the runtime at PR #22 (`72dbb2a`), after allocator,
callback-lifecycle, entity-budget, and lexical-diagnostic fixes. They are evidence
for a bounded implementation, not a complete Expat replacement claim.
The [benchmark build manifests and source archive](../../../benchmarks/results/2026-09-10/checkpoint/)
identify the tested library. Compressed reports retain exact inputs, commands,
library/source hashes, failures, and logs; `index.json` records their file hashes.

| Gate | Result | Scope |
|---|---|---|
| Generated and named differential corpus | 6,159 semantic comparisons pass | Acceptance, error codes, and normalized callbacks; chunks 1, 7, and 4,096 |
| Pinned libxml2 corpus and named cases | 807 comparisons pass | Acceptance/error/location only; callbacks disabled |
| Actual CPython XML consumers | 803 reported tests, 31 skips; suites succeed | Each of shared and static unmodified consumers, CPython 3.12.13 |
| Native C ABI/lifecycle | Pass | C callbacks instrumented with ASan/UBSan |
| Custom allocator failure sweep | 386 scenarios pass | Every successful custom allocation released |
| C API sanitizer fuzzing | 5,422 replay and 591,789 campaign executions pass | Rust ASan, 121-second campaign; leak sanitizer disabled under ptrace |
| Adapted upstream Expat API matrix | 3,323 pass; 1,417 fail | 126 distinct failing tests across chunk/deferral contexts |
| Reference Expat API matrix | 4,740 pass | Same adapted public test bodies, Expat 2.8.4 |

The generated differential run still has 34 exact callback-fragmentation differences
and 357 final-location differences. Its semantic gate coalesces adjacent text
fragments; its strict gate remains failed. The libxml2 run does not establish
callback equivalence. That earlier checkpoint did not run the W3C corpus.

The upstream matrix uses 12 chunk/deferral contexts. It excludes 12 explicitly
listed tests of private Expat implementation details and corrects a separately
reproduced bug in Expat's test allocator. It preserves the public assertions and
runs each test in a child process to avoid unwinding C test failures through Rust.
The final matrix has no crashes or timeouts. Failures remain failures: allocation
retry ceilings and internal caching assumptions account for many, while unsupported
DTD/encoding modes, diagnostics, and callback contracts require compatibility work.
See the [upstream harness](../../../tools/upstream-expat/) for the adaptation audit.

The unmodified CPython suites do not reach a known partial-child cleanup bug.
The [explicit upstream backport and fault probes](../../../integration/python-build-standalone/consumer-fix/)
show that both reference-linked and Oriole-linked original consumers crash when
child creation returns NULL. Patched reference, shared-Oriole, and static-Oriole
consumers raise `MemoryError` and preserve parent ownership. Patched shared/static
consumers each report the same 803 tests, including 31 skips, with successful suites. The opt-in PBS bundle includes this fix.

The separate [external-family fuzz target](../../../fuzz/results/2026-09-10/ffi-family/)
passed 16 initial-seed replays and 266,749 executions in 121 seconds, exercising
parent reset/free, surviving children, DTD merge, and allocator failures. Fuzzing
and allocation checks supplement independent parser and FFI reviews; short campaigns
do not establish exhaustive memory safety or conformance.

Linux x86_64, Linux ARM64, macOS, Windows, and Rust 1.96/stable CI gates pass at
[this checkpoint](https://github.com/astral-sh/oriole/actions/runs/34433931193).
The macOS gate caught hidden standard-mutex allocations, now removed by inline
atomic lifetime tokens. Related parser operations still require serialization.

## Remaining deployment gates

The [PBS recipe](../../../integration/python-build-standalone/) has a real PIC
archive, native static consumer checks, and complete-archive shared-link validation.
The [earlier `b68bdca` PBS job](pbs-values/) passed the complete Linux x86-64 distribution, its
metadata/linkage validator, and the resulting interpreter's XML tests, including
the actual glibc 2.17 baseline. The final-runtime build is tracked above. Other
distribution targets need separate validation.

Complete Expat compatibility, unsupported DTD/custom-encoding modes, strict callback
and position equivalence, and broader target distribution validation remain open.
The README keeps the experimental status while these gates
remain open. The [benchmark report](../../../benchmarks/results/2026-09-10/checkpoint/)
also records the remaining C interface performance gap.
