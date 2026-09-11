"""Assemble the completed sanitizer handoff without changing its evidence."""
from pathlib import Path
import hashlib
import json
import shutil

worktree = Path('/home/dev-user/code/oss/oriole-current-asan')
handoff = Path('/tmp/oriole-be22-asan-handoff')
destination = worktree / 'docs/validation/2026-09-11/current-asan'
report = json.loads((handoff / 'report.json').read_text())
assert report['status'] == 'passed'
assert report['total']['all_processes_exit_zero']
assert report['total']['artifacts'] == report['total']['harness_timeouts'] == 0
assert report['source']['source_unchanged']
assert not report['source']['literal_attribute_candidate_included']
assert not destination.exists()
for name, entry in json.loads((handoff / 'files.json').read_text()).items():
    assert hashlib.sha256((handoff / name).read_bytes()).hexdigest() == entry['sha256']
shutil.copytree(handoff, destination / 'data')
total = report['total']
rows = '\n'.join(
    f"| {row['target']} | {row['initial_files']:,} | {row['replay_exec']:,} | {row['campaign_exec']:,} | {row['final_disk_files']:,} |"
    for row in report['targets']
)
text = f'''# Current parser sanitizer validation

Six fresh AddressSanitizer campaigns completed **{total['campaign_exec']:,} executions without findings** on the accepted `be22a27` runtime, built from PR118 commit `c1776c9`. All six retained-corpus replays and six campaigns exited successfully. This brings sustained sanitizer coverage through the native byte-count optimization; the separate attribute and text-error experiments are excluded.

## Results

| Target | Initial files | Replay executions | Campaign executions | Final disk files |
| --- | ---: | ---: | ---: | ---: |
{rows}
| Total | {total['initial_files']:,} | {total['replay_exec']:,} | {total['campaign_exec']:,} | {total['final_disk_files']:,} |

The initial corpus combines all 26,689 initial and 16,272 evolved files from the [preceding campaigns](../../2026-09-10/detached-frames-fuzz/), retaining every origin and all three empty inputs. LibFuzzer executes empty input once per target, giving 42,964 separate replay executions. Campaign counts include initialization and repeated inputs. Final disk files are retained separately from the original corpus and from libFuzzer's live corpus count; these are not global unique-input counts.

## Build and protocol

All 357 tracked source files match the frozen Git revision, worktree and source archive. The 70 runtime files match the accepted source. The fresh build records AddressSanitizer in all nine compiler invocations: three workspace libraries and six fuzz targets. Independent inspection verifies representative instrumented code in `XML_Parse`, `Parser::parse_text` and `AdapterFrame::text_bytes`.

The build uses Ohm Rust 1.98.1-dev with LLVM 22 and Clang 18.1.3's external AddressSanitizer runtime. Experimental Ohm defaults and trust opt-ins are disabled for the build. Each target replays every retained input, then runs with a 600-second budget, seed `20260911`, a 64 KiB input limit, a 10-second per-input timeout and a 1,536 MiB RSS limit. Observed campaign durations are {total['campaign_duration_min_seconds']:.3f}–{total['campaign_duration_max_seconds']:.3f} seconds.

Three CPU lanes each run two targets: CPU1 parse/multibyte, CPU2 streaming/value_family and CPU4 ffi/ffi_family. Replay, campaign and aggregate outer bounds are 180, 690 and 1,920 seconds. The controller owns each target process group and cleans it on completion, interruption or timeout. Other CPUs perform correctness checks on this shared host; no performance timing overlaps these campaigns.

Leak detection remains disabled (`detect_leaks=0:abort_on_error=1`). This is bounded AddressSanitizer evidence, with no new LeakSanitizer, UndefinedBehaviorSanitizer, whole-standard-library instrumentation or exhaustive safety claim. There were no target crashes, timeout artifacts or reruns.

Three preparation failures remain recorded: an ambient Python import hook, an unsupported worktree-setup flag, and a redundant copy over an identical read-only provenance file after materialization. Isolated system Python, a separate setup correction, and a complete read-only input audit resolved them before target execution. No failed target campaign was discarded or retried.

## Evidence and scope

The [report](data/report.json), [complete evidence](data/evidence.tar.gz), [final corpus](data/final-corpus.tar.gz) and [member indexes](data/files.json) retain commands, sources, compiler records, binary identities, complete logs, inputs, provenance and preparation failures. Compiled executables are excluded and separately hashed. The [handoff guide](data/README.md) describes archive membership and deduplication.

The [independent readiness review](data/readiness-review.json) verifies source, instrumentation and all input origins. Its scope precedes campaign outcomes. Final campaign and package review is recorded separately before publication.

The existing 393 upstream API failures and two strict CPython callback-grouping failures remain in the [compatibility report](../native-byte-count/). These sanitizer runs establish neither new compatibility nor speed. A separate [full PBS build](https://github.com/astral-sh/oriole/actions/runs/34563056904) uses stable Rust on the same accepted revision; this report makes no PBS outcome claim.
'''
(destination / 'README.md').write_text(text)
readme = worktree / 'README.md'
old = 'Six [sustained ASan campaigns](docs/validation/2026-09-10/detached-frames-fuzz/) completed 14.62 million executions on `5bc806e` without findings; these precede the latest parser changes.'
new = f"Six [current ASan campaigns](docs/validation/2026-09-11/current-asan/) completed {total['campaign_exec']:,} executions on the accepted `be22a27` runtime without findings."
value = readme.read_text()
assert value.count(old) == 1
readme.write_text(value.replace(old, new))
review = worktree / 'docs/review.md'
review.write_text(review.read_text() + f"\nThe [current ASan campaigns](validation/2026-09-11/current-asan/) cover the accepted `be22a27` runtime with {total['campaign_exec']:,} campaign executions and 42,964 retained-corpus replays. Fresh instrumented builds and all corpus origins are retained, along with the preparation corrections. Candidate optimizations, performance measurements and the separate full PBS workflow retain their own scope.\n")
print(destination)
