# CPython line-break callbacks — 2026-09-13

The C interface now preserves the line-break character-data callbacks required by
CPython's `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Rust event
consumers retain coalesced text by default. This fixes those two assertions;
other exact fragmentation and position differences remain documented.

## Merge-rebase qualification

These tests and measurements predate the merge rebase onto `03c21b1`. Upstream
changes include bounded prolog whitespace scanning and an index of attributes
with default values. Upstream also removed the PBS integration and moved the
cleanup backport to
[`tools/cpython/consumer-fix`](../../tools/cpython/consumer-fix/README.md).
The source and artifact hashes below identify the measured pre-rebase builds;
these results do not qualify a rebuilt, rebased artifact.

## Strict consumer results

The pinned CPython 3.12.13 source (`3bb231a6a5dc02b95658877318bf61501a7209e9`)
was clean. `tools/cpython/run.py` compiled its real `pyexpat` and `_elementtree`
extensions and verified their loaded origins. All six unchanged upstream XML
suites passed with both shared and static Xeme libraries, without the historical
text-fragmentation allowance or consumer adaptations. Each run discovered 803
cases, reported 802 run and 14 skips, and accounted for the full inventory
including the class-setup skip. The additional text/order/buffering checks passed.

The frozen baseline reproduced exactly the two historical failures, with strict
upstream and gate exit codes 2. Both candidate runs returned 0. Separate candidate
shared/static runs with the disclosed allocation-cleanup backport also returned 0.
The backport remains an independent consumer ownership fix.

CI and installed-distribution validation now require strict upstream success,
pinned fixtures and complete inventory. The explicit historical
`--allow-text-fragmentation` option remains available for old-source diagnostics.

## Tested source and artifacts

These local Linux x86-64 development builds used base
`56e7194b4e5f2fc300cf926db3dd0d9c4bf47b9f` plus the callback-boundary changes.
They used `cargo +ohm build -p xeme_expat` with Rust
`1.98.1-dev (f62703110 2026-09-08)`, Ohm `1.98.1-1`, in the unoptimized dev profile.
Formatting after compilation changed source layout only.

| Item | SHA-256 |
| --- | --- |
| Baseline shared library | `ffd08e0728dddff820a051a34475a4000a873899e3879d2ac0ecb111f106f0d1` |
| Candidate shared library | `437f574f8a8125302b36ffdc804c6587901df33a26c46dd6cfa12e5fb630f2cf` |
| Candidate static library | `677e6b2cc1fe05555c2fda1046d19f3111f933cb2c25d14a70ebe2fe227736b2` |
| Formatted runtime source diff | `5d8be0a9802e9e4c9e9bea27cdd860298a242ef755e77f5d10dd89933d813af5` |

The diff is `git diff --binary HEAD -- crates/xeme/src/encoding.rs
crates/xeme/src/lib.rs crates/xeme/src/text.rs crates/xeme_expat/src/lib.rs`
against the base above. Local raw artifacts are under
`/tmp/xeme-c037-cpython-grouping/`: `build-record.json` records the complete build
command and source hashes; `runtime.diff` binds the source changes; and the
`baseline`, `candidate-shared`, `candidate-static`, and separate `consumer-fix`
run directories retain frozen libraries, test logs, fixture hashes and summaries.
These artifacts have not been published as a durable release asset.

The CPython runs above qualify these extension builds. They do not requalify an
installed PBS distribution or another platform.

## Additional regression gates

All 523 Rust workspace tests and doctests passed. Native integration and
adversarial checks passed, including 356 allocation-failure scenarios. Formatting,
Clippy, Ruff, ty and the nine CPython gate tests passed.

The full Expat 2.8.4 API gate passed against the release candidate below: Expat
passed all 4,740 test/configuration rows; Xeme passed 4,231 and retained the 509 reviewed
failures without new failures. The W3C gate checked 6,003 rows per engine and
retained the same 960 known failures in each, with no new differences.

The differential gate passed all 2,730 semantic cases. Its separate strict
observations still report eight text-grouping differences and 84 position
differences. Passing the CPython assertions does not establish identical callback
fragmentation for all XML input. Raw gate reports remain under the local artifact
directory above.

## Performance cost screen

A matched before/after run of the six existing tuning projects measured
**13.9% more native time and 17.6% more CPython time** after restoring the callback
boundaries. The implementation was selected for correctness before these timings.
These measurements establish a performance regression in the initial
implementation. The [performance follow-up](2026-09-13-cpython-grouping-performance.md)
investigates repeated per-callback work while preserving the corrected boundaries.

The [benchmark guide](../../benchmarks/HILLCLIMB.md) screen protocol used three
randomized paired rounds, with three measured parses per process after a discarded
warmup. Native conditions cover namespaces on/off and 4/64 KiB feeds; CPython
conditions cover ElementTree and pyexpat with both feed sizes. Ratios are medians
of paired process-median ratios; each group below is their equal-weight geometric
mean. Every timed output passed its canonical or normalized callback checks.

| Group | Conditions | Candidate / baseline | Candidate / Expat | Slower than baseline |
| --- | ---: | ---: | ---: | ---: |
| Native, tuning projects | 24 | 1.1392× | 1.3672× | 22 |
| CPython, tuning projects | 24 | 1.1764× | 1.2565× | 23 |
| Native, generated controls | 16 | 1.4178× | 2.0342× | 15 |

The worst real native condition was Wayland with 64 KiB feeds and namespaces,
at **1.4345× baseline time**. The worst CPython condition was Wayland's pyexpat
consumer with 64 KiB feeds, at **1.7286×**. The worst generated condition was text
with 4 KiB feeds and namespaces, at **3.4473×**. Generated controls remain separate
from the real-project aggregates. All 64 conditions, including improvements and
regressions, are retained in `performance/screen/conditions.csv`.

Both Xeme builds used the same compiler as above, ordinary O3, ThinLTO, one codegen
unit and generic x86-64 through `hillclimb.py build --toolchain ohm`, which disables
Ohm's experimental defaults. Each source snapshot used its own target and fresh
intermediate directory. The baseline is the base revision above; the candidate's
runtime diff matches the recorded formatted diff exactly. The unrelated lockfile
ordering change was excluded. All three engines used the clean, unmodified pinned
CPython 3.12.13 sources, compiled with identical O2 settings.

| Frozen release artifact | SHA-256 |
| --- | --- |
| Baseline shared library | `547ed8f1779b8f424a968d2191ccf82ee7361a9199912ff1aafd531b2e866cdd` |
| Candidate shared library | `93443b3d344eb97d2e079eb831a07abd09d886f3b88ea6fbab33c43241bfde1c` |
| Pinned normal Expat 2.8.4 control | `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478` |
| Screen report | `fb12b601414cbcce4bdcb00ef6f816d51d3e82bb4efcba35db78b700755c215f` |
| Complete condition CSV | `3382aeed51520e1a8f141c829fac4cfa04a13556b9f482aa7e9fb3b5c408eb13` |

Local artifacts under `/tmp/xeme-c037-cpython-grouping/performance/` retain both
source snapshots, the candidate patch, validated build records and compiler logs,
the consumer bundle, exact screen command, raw workers and an independent
reconstruction of all 1,728 measured samples. These files have not been published
as durable release assets.

Timing ran on CPU 0 from 16:13:58 to 16:16:10 UTC after this task's builds and
validation jobs finished. Other users' activity, CPU frequency, caches and memory
bandwidth remained uncontrolled on the shared Linux host. This coarse screen uses
known tuning files and local extension consumers. Repeatability, holdout and
installed-distribution performance require separate measurements.
