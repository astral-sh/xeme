# Current parser sanitizer validation

Six fresh AddressSanitizer campaigns completed **10,496,706 executions without findings** on the accepted `be22a27` runtime, built from PR118 commit `c1776c9`. All six retained-corpus replays and six campaigns exited successfully. This brings sustained sanitizer coverage through the native byte-count optimization; the separate attribute and text-error experiments are excluded.

## Results

| Target | Initial files | Replay executions | Campaign executions | Final disk files |
| --- | ---: | ---: | ---: | ---: |
| parse | 4,316 | 4,316 | 4,090,775 | 4,479 |
| multibyte | 11,282 | 11,282 | 1,137,365 | 1,531 |
| streaming | 9,000 | 9,001 | 1,890,912 | 2,781 |
| value_family | 3,686 | 3,687 | 673,319 | 769 |
| ffi | 5,144 | 5,145 | 2,222,031 | 3,539 |
| ffi_family | 9,533 | 9,533 | 482,304 | 1,058 |
| Total | 42,961 | 42,964 | 10,496,706 | 14,157 |

The initial corpus combines all 26,689 initial and 16,272 evolved files from the [preceding campaigns](../../2026-09-10/detached-frames-fuzz/), retaining every origin and all three empty inputs. LibFuzzer executes empty input once per target, giving 42,964 separate replay executions. Campaign counts include initialization and repeated inputs. Final disk files are retained separately from the original corpus and from libFuzzer's live corpus count; these are not global unique-input counts.

## Build and protocol

All 357 tracked source files match the frozen Git revision, worktree and source archive. The 70 runtime files match the accepted source. The fresh build records AddressSanitizer in all nine compiler invocations: three workspace libraries and six fuzz targets. Independent inspection verifies representative instrumented code in `XML_Parse`, `Parser::parse_text` and `AdapterFrame::text_bytes`.

The build uses Ohm Rust 1.98.1-dev with LLVM 22 and Clang 18.1.3's external AddressSanitizer runtime. Experimental Ohm defaults and trust opt-ins are disabled for the build. Each target replays every retained input, then runs with a 600-second budget, seed `20260911`, a 64 KiB input limit, a 10-second per-input timeout and a 1,536 MiB RSS limit. Observed campaign durations are 601.312–601.710 seconds.

Three CPU lanes each run two targets: CPU1 parse/multibyte, CPU2 streaming/value_family and CPU4 ffi/ffi_family. Replay, campaign and aggregate outer bounds are 180, 690 and 1,920 seconds. The controller owns each target process group and cleans it on completion, interruption or timeout. Other CPUs perform correctness checks on this shared host; no performance timing overlaps these campaigns.

Leak detection remains disabled (`detect_leaks=0:abort_on_error=1`). This is bounded AddressSanitizer evidence, with no new LeakSanitizer, UndefinedBehaviorSanitizer, whole-standard-library instrumentation or exhaustive safety claim. There were no target crashes, timeout artifacts or reruns.

Three preparation failures remain recorded: an ambient Python import hook, an unsupported worktree-setup flag, and a redundant copy over an identical read-only provenance file after materialization. Isolated system Python, a separate setup correction, and a complete read-only input audit resolved them before target execution. No failed target campaign was discarded or retried.

## Evidence and scope

The [report](report.json) and [complete producer handoff](producer-handoff.tar.gz) retain commands, sources, compiler records, binary identities, complete logs, original and final inputs, provenance and preparation failures. Compiled executables are excluded and separately hashed. Every original handoff file is archived unchanged, including its guide and member indexes. The [publication index](files.json) records file hashes, and the [archive member index](archive-members.json) records member hashes. Large per-input maps remain compressed.

The [independent readiness review](readiness-review.json) verifies source, instrumentation and all input origins. Its scope precedes campaign outcomes. The [independent final review](independent-review.json) reconstructs all 12 executions, totals and package origins from saved evidence: 676,701 checks across 57,597 unchanged inputs, with no findings. Its [complete receipt and audit](independent-review.tar.gz) are retained separately. The producer guide's pending-review sentence describes its earlier sealing time; this final review supersedes that status.

The [draft prose review](draft-prose-review.tar.gz) and [draft structure review](draft-structure-review.tar.gz) retain their exact earlier document snapshots. They exclude this final packaging and review-link update; the [final publication review](publication-review.json) covers those changes.

The existing 393 upstream API failures and two strict CPython callback-grouping failures remain in the [compatibility report](../native-byte-count/). These sanitizer runs establish neither new compatibility nor speed. A separate [full PBS build](https://github.com/astral-sh/oriole/actions/runs/34563056904) uses stable Rust on the same accepted revision; this report makes no PBS outcome claim.
