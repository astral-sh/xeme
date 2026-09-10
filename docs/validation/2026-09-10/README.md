# Compatibility checkpoint: 10 September 2026

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
| Actual CPython XML consumers | 803 tests pass, 31 skips | Each of shared and static unmodified consumers, CPython 3.12.13 |
| Native C ABI/lifecycle | Pass | C callbacks instrumented with ASan/UBSan |
| Custom allocator failure sweep | 386 scenarios pass | Every successful custom allocation released |
| C API sanitizer fuzzing | 5,422 replay and 591,789 campaign executions pass | Rust ASan, 121-second campaign; leak sanitizer disabled under ptrace |
| Adapted upstream Expat API matrix | 3,323 pass; 1,417 fail | 126 distinct failing tests across chunk/deferral contexts |
| Reference Expat API matrix | 4,740 pass | Same adapted public test bodies, Expat 2.8.4 |

The generated differential run still has 34 exact callback-fragmentation differences
and 357 final-location differences. Its semantic gate coalesces adjacent text
fragments; its strict gate remains failed. The libxml2 run does not establish
callback equivalence. No W3C conformance run is claimed.

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
consumers each pass the same 803 tests. The opt-in PBS bundle includes this fix.

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
A dedicated CI job now attempts the complete distribution, its metadata/linkage
validator, and the resulting interpreter's XML tests. A successful local archive
link does not establish PBS's older-glibc target baseline.

Complete Expat compatibility, unsupported DTD/custom-encoding modes, strict callback
and position equivalence, sustained sanitizer campaigns, and target distribution
validation remain open. The README keeps the experimental status while these gates
remain open. The [benchmark report](../../../benchmarks/results/2026-09-10/checkpoint/)
also records the remaining C interface performance gap.
