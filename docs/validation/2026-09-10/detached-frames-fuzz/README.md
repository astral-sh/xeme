# Sustained sanitizer campaigns for detached callback frames

Six fresh Rust AddressSanitizer campaigns on `5bc806e5fc2a545f22c75f2a1bcbef45cfa3632b`
completed **14,617,069 executions without findings**. All six retained-corpus replays
and six campaigns exited successfully. This is the runtime published in PR #106;
these campaigns precede the later bounded text-search and other scanner changes.

## Results

| Target | Initial raw files | Campaign executions | Final disk corpus files |
| --- | ---: | ---: | ---: |
| ffi | 1,001 | 2,913,671 | 4,143 |
| ffi_family | 8,189 | 471,576 | 1,344 |
| multibyte | 9,484 | 1,218,138 | 1,798 |
| parse | 250 | 6,317,751 | 4,066 |
| streaming | 5,000 | 3,011,008 | 4,000 |
| value_family | 2,765 | 684,925 | 921 |
| Total | 26,689 | 14,617,069 | 16,272 |

Executions include corpus initialization and repeated inputs. The initial files
contain 26,686 nonempty entries and three empty entries across target directories.
LibFuzzer loads the nonempty entries and tests empty input once per target, giving
**26,692 separate replay executions**. These replay counts are separate from the
campaign total. Every original file is retained, including the empty files.

The final writable directories contain 16,272 files. This disk corpus differs
from the live corpus count printed by libFuzzer and from the original seed corpus;
neither raw file total is a global count of unique inputs. The final corpus is
archived separately and never substituted for the original input corpus.

## Build and execution

The build records all three workspace library compilations and all six fuzz
target compilations with `-Zsanitizer=address` in private intermediate directories.
The 357 tracked source entries match the Git commit, live worktree and source
archive. The toolchain is Ohm Rust 1.98.1-dev with LLVM 22; Clang 18.1.3 supplies
the external AddressSanitizer runtime. The recorded build disables experimental
Ohm defaults and clears inherited wrappers and encoded Rust flags.

Non-mutating machine-code inspection confirms AddressSanitizer checks in
`XML_Parse`, `Parser::parse_text` and `AdapterFrame::text_bytes`. This is
instrumented Rust storage, core, FFI and fuzz-target evidence; it does not
establish instrumentation of the entire standard library or libc.

Each target replays its complete retained corpus, then runs with a 600-second
libFuzzer budget, seed `20260911`, 64 KiB maximum input, 10-second per-input timeout
and 1,536 MiB RSS limit. The separate replay has a 180-second outer wait and each
campaign has a 690-second outer wait. Observed campaign durations are
601.141–601.192 seconds; initialization is included in the campaign budget.
Three pinned lanes run two targets each: CPU1 parse/multibyte, CPU2
streaming/value_family and CPU4 ffi/ffi_family. Other cores also perform project
validation on this shared Linux host.

`ASAN_OPTIONS=detect_leaks=0:abort_on_error=1` disables leak detection in the
ptrace sandbox. These are AddressSanitizer campaigns, with no new LeakSanitizer
or UndefinedBehaviorSanitizer claim. There were no crashes, timeout artifacts,
nonzero exits or dropped cases. No campaign reruns are recorded in this evidence.
Per-target cleanup handles the
running child process group on timeout or interruption; the build and outer
controller do not provide complete process-tree cleanup coverage. Their observed
runs completed normally.

## Evidence and scope

[evidence.tar.gz](evidence.tar.gz) preserves 55 regular members: controllers,
source and initial-corpus archives, build logs, original command lines, binary
hashes, six campaign records and representative instrumentation. Its SHA-256 is
`3bdd18ae72a22f770c653b9a32270da23f454d4f6fa9bd31eb5b6b2d244fedda`.
Six compiled executables are excluded from the archive and separately hashed.

[final-corpus.tar.gz](final-corpus.tar.gz) retains all 16,272 resulting disk files;
its SHA-256 is `d8154f6185e1a39f054962184f503edfe9529e204ce46fbf6271fed236e7ef85`.
[Member indexes](members.json), [final corpus index](final-corpus-members.json)
and [independent review](independent-review/review.json) preserve full readbacks
of every outer member and all nested source, initial and final corpus files.
The package controller is included. [The full review archive](independent-review.tar.gz)
retains both independent audit generators and their complete hash maps; its
[member index](independent-review-members.json) records every file.

The initial corpus comes from the earlier
[version-consistent campaigns](../version-consistent-fuzz/); the exact provenance
archives and hashes are recorded. Bounded fuzzing provides additional evidence,
not a proof of memory safety or general compatibility. The unchanged public API
and CPython failures remain in the [compatibility report](../coalesced-search/).
No throughput, new API compatibility or full PBS distribution claim follows from
these sanitizer executions.
