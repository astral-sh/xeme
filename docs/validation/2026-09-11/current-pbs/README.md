# Current parser PBS integration

The [full PBS workflow](https://github.com/astral-sh/oriole/actions/runs/34563056904) built CPython 3.12.13 with the accepted `be22a27` runtime from commit `c1776c9`. The distribution loads on glibc 2.17, including both built-in XML accelerators. **The workflow and XML suites remain failed on two character-data callback-grouping assertions.**

## Results

| Check | Result |
| --- | --- |
| Full distribution build and archive validator | Passed |
| Two custom-suite invocations in CI | Each: 17 successes, five skips |
| Installed identity and fresh accelerator import, local | Passed; Oriole 2.8.4; both XML accelerators built in |
| Threaded loading on glibc 2.17, local | 1,024 parses passed |
| Host and glibc 2.17 verbose XML suites, local | Each: 802 tests, two failures, 12 skipped outcomes, three expected failures |
| Original glibc 2.17 script, local | Threaded check passed; XML failed: 802 tests, two failures, 13 skips |

All 802 method outcomes, fixture outcomes and subtest outcomes match between the host and container verbose runs. Both fail `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`, with the same grouping assertions found in this workflow's raw log. CI ran 802 initial tests and four automatically selected reruns, producing four failure occurrences across those two methods.

Collection finds 803 methods; the no-accelerator class skips with accelerators present. The verbose runs' 12 skipped outcomes comprise three methods, eight subtests and one class. The original `-m test` script adds a CPU-resource skip; the corresponding Minidom test passes under both verbose `unittest` runs. No test or assertion was changed.

## Build and scope

The 69 bundle source hashes and 97 launch source hashes match the Git revision; their 101-file union is retained. Three actual compiler invocations record stable Rust 1.98.1, C-only ThinLTO, one codegen unit, PIC and unwind. These sysroot-built binaries have no PGO or sanitizer instrumentation and are distinct from the local Ohm benchmark binaries. PBS retains the upstream bounded-allocation cleanup backport in `pyexpat.c`; its XML tests are unchanged.

The distribution archive is SHA-256 `d7033afb5a01b88737c2e57500269f0078d9237af01455506e5343f46cba539e`. Its static Expat library matches the Oriole bundle. All 6,546 archive members were inspected and all 5,497 original regular members rehashed after local checks. Source, test and native binary bytes remained exact; interpreter startup rewrote three existing encoding bytecode caches. The downloaded archive is unchanged.

CI skipped installed identity and glibc validation after XML validation failed. The five local diagnostics supplement those skipped steps. They do not turn the workflow or original glibc script green. The pinned CentOS image was already present; both containers disabled networking, mounted the distribution and checks read-only, and were cleaned up by their owned IDs. A Docker socket permission denial occurred before container execution; its receipt is preserved, and authorized access resumed only the two unexecuted container diagnostics. There were no target retries or timeouts.

## Evidence

The [report](report.json) and [complete producer handoff](producer-handoff.tar.gz) retain raw CI and local logs, reviewed command arrays, source snapshots, compiler vectors, archive/source/binary hashes, every method outcome, cleanup receipts and the failed Docker prerequisite. Every handoff file is archived unchanged; its pre-execution plan retains its historical held status. Compiled distribution files are excluded from the compact package with exact hashes, and the complete original downloads remain in the recorded local study directory.

The [archive member index](archive-members.json) records the producer wrapper's exact members. The [independent review](independent-review.json), [complete review receipt](independent-review.tar.gz) and [publication index](files.json) provide the final review and file identities. This report supplies current-source integration evidence; it makes no speed, exhaustive compatibility or production-readiness claim.
