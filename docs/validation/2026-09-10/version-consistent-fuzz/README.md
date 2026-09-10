# Sustained fuzzing of the version-consistent runtime

Six ASan libFuzzer campaigns exercised runtime
`4b11ace3d89fe6b7bf66ca55290eb23444f11de2` for **14,502,453 executions**.
Each ran for about ten minutes. All six seed replays and campaigns exited
successfully, with no crash artifacts or harness timeouts. Independent review
reconstructed the counters from the raw logs and rehashed the saved evidence.

| Target | Executions | Initial seed files | Evolved corpus files | Duration | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| `parse` | 5,705,335 | 250 | 3,893 | 601.14 s | 518 MiB |
| `streaming` | 3,069,106 | 5,000 | 3,928 | 601.16 s | 516 MiB |
| `ffi` | 3,178,013 | 1,001 | 4,055 | 601.16 s | 490 MiB |
| `ffi_family` | 507,652 | 8,189 | 1,331 | 601.24 s | 466 MiB |
| `multibyte` | 1,313,667 | 9,484 | 1,769 | 601.16 s | 512 MiB |
| `value_family` | 728,680 | 2,765 | 900 | 601.15 s | 459 MiB |

Execution counts come from libFuzzer's final campaign statistics and include
campaign seed initialization; they are not counts of unique inputs. The archive
retains every byte of 26,689 initial seed files, including three empty files, and
15,876 evolved corpus files. All 26,937 recorded origins were reconstructed from
named seeds, 250 differential documents and six earlier corpus archives. Multiple
origins can produce the same captured seed file.

## Build and campaign scope

The 349-file source manifest matches the applicable subset of the frozen
358-file runtime snapshot. Builds used Ohm Rust 1.98.1-dev, libFuzzer coverage and
the external Clang ASan runtime. The build review checked representative ASan
guards in Rust storage and `XML_Parse` disassembly. It does not establish
instrumentation of every standard-library function. The six fuzz executables
have their own [recorded hashes](binaries.json); they are distinct from the
release libraries in the [compatibility report](../version-consistent-runtime/).

Each process used seed `20260910`, a 65,536-byte maximum input, a ten-second
per-input timeout and a 1,536 MiB RSS limit. Seed replay had a 180-second outer
limit. Each campaign requested 600 seconds with a 690-second outer limit.
Four CPU lanes ran `parse` then `multibyte`, `streaming` then `value_family`,
`ffi`, and `ffi_family`. The orchestration processes were not uniformly pinned,
and shared-host load was uncontrolled.

**LeakSanitizer was disabled** with `detect_leaks=0`. These campaigns make no
UBSan claim. Selected-allocation and ownership assertions are separate checks.
The cargo-fuzz executable's version and hash were captured retrospectively
during review; the build commands, logs and disassembly provide separate
evidence of how these binaries were instrumented.

## Completion and review

The initial runner review recorded interruption/exception cleanup gaps and
incomplete final evidence checks. No runner was edited during its campaign and
no campaign was interrupted. The separate completion verifier requires all six
builds, both runs per target, zero exits, final statistics, the requested
durations, empty artifact directories and unchanged source, binary, runner,
manifest and corpus bytes. The independent final review rehashed 42,973 files
and reached the same result.

The earlier orchestration review observed four completed campaigns and two
pending campaigns; it remains unchanged in the archive. The final review checks
all six. The root's controller receipt records tool session `46928` with exit
zero. The reviewer compared that receipt with the saved controller and child
records, without attaching to the original tool session.

These bounded campaigns cover the recorded inputs, source, builds and host.
They do not establish exhaustive safety, semantic compatibility or production
readiness. The [API and consumer failures](../version-consistent-runtime/) remain
visible separately.

## Retained evidence

[evidence.tar.gz](evidence.tar.gz) contains the immutable handoff, including full
source, initial and evolved corpora, origin inputs, raw logs, build and runner
scripts, manifests, reviews and readback tools. Its SHA-256 is
`467a3b793cc1c08489cd718693bbb77b6c83e399e802a0a38fd460455acf3869`.
The six built executables are excluded; their hashes are retained.

[files.json](files.json) records all 20 outer members. The embedded handoff
manifest records 18 artifacts, and `archive-members.json.gz` records the sizes
and hashes of 42,978 regular members in six nested archives. Root packaging
verified these bytes against the handoff and readback records. Packaging and
verification did not rerun a compiler or parser.

Readable copies of the [summary](summary.json),
[completion verification](completion-verification.json),
[origin verification](origin-verification.json) and
[independent review](independent-review.json) accompany the archive.
