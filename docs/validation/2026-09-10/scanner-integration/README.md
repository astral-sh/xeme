# Short positions and fallible string operations

The parser advances through ordinary ASCII in eight-byte words for short position
updates, then retains the original scalar suffix for CR, LF and Unicode. Four
existing fallible string append/reserve wrappers receive ordinary inline hints;
their function bodies, allocation order and error handling are unchanged.

The combined runtime commit is `64a270e58f14a4b74e25ffcf25f7832a5216b2d1`.
All 70 frozen source entries exactly match the measured build. The build began
on `d53dc6c` with the two component files; assembly advanced through the doc-only
`dfa6cd3` parent without changing runtime or test bytes.

## Compatibility

| Check | Result |
| --- | --- |
| Workspace | 395 tests across 33 binaries, plus one separate doc test; all pass |
| Formatting and strict Clippy | Pass |
| Original public API matrix | 4,347 pass / 393 fail; all 4,740 outcomes unchanged |
| Native C consumers | Six shared/static ASan/UBSan programs pass; original 327 allocation scenarios per linkage |
| Strict callback/status/error/position comparison | 3,318 exact baseline traces |
| Malformed text and encodings | 2,392 exact baseline comparisons |
| Custom aliases and external multibyte payloads | 36,456 exact baseline comparisons |
| Callback publication, raw context and suspension | 1,304 cases / 3,912 library parses; exact baseline records |
| Unchanged CPython 3.12.13 suites | Shared and static each run 802 tests, with two failures, 14 skips and three expected failures |

The API harness retains its original 3-second per-configuration timeout, 1 GiB
address-space bound, 768 MiB RSS bound and 240-second total timeout. Its original
exit remains 1. The shared and static CPython suite/gate exits remain 2, with the
same `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` text-grouping
failures. Every method status matches the earlier strict suites. There is no
consumer adaptation or text-fragmentation waiver. Fresh accelerator imports and
library origins are checked separately from the suite status.

The compatibility controls include the published `5bc806e` library. They retain
existing differences from Expat, including Default and ExternalEntityReference
positions. Native sanitizer results instrument the C consumers and use the normal
Rust release library, with leak detection disabled. The earlier sustained Rust
ASan and PBS reports keep their own source scopes; they do not test this build.
See the [per-test API failure classification](../detached-frames/api-classification/).

## Integrated performance

| Cohort | Conditions | Candidate / published | Candidate / Expat | Faster than published | Faster than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native project XML | 24 | 0.982× | 2.010× | 19 | 0 |
| Actual CPython consumers | 24 | 0.978× | 1.419× | 23 | 2 |
| ElementTree | 12 | 0.978× | 1.534× | 12 | 0 |
| pyexpat callbacks | 12 | 0.978× | 1.313× | 11 | 2 |
| Native generated adverse fixtures | 4 | 1.024× | 5.469× | 0 | 0 |

These are geometric means of all per-condition paired median time ratios; lower
is faster. The current `193e1278` / `50580931` performance control already includes
the preceding bounded text search. The combined change reduces native project
time by **1.8%** and actual CPython time by **2.2%** against that control.

All regressions are retained. Native Vulkan regresses in all four feed/namespace
conditions by 0.80–1.96%, and namespace-disabled Maven at 4 KiB by 0.50%.
Generated rare declarations regress by 5.76% at 4 KiB and 0.44% at 64 KiB;
generated entities regress by 2.47% and 1.20%. The Python regression is Vulkan
pyexpat at 64 KiB (+0.249%). Only the two Wayland pyexpat conditions beat Expat.
The current parser still takes about twice Expat's native time on this corpus;
these measurements do not establish broad faster-than-Expat performance.

The integrated instruction screen retains all 32 normal preflights and 32
Callgrind profiles: six projects with both namespace modes at 4 KiB, plus both
generated controls at 4 KiB and 64 KiB. All 12 project conditions use fewer
instructions, with a 5.85% geometric mean reduction. All four generated instruction
counts also decrease. Those counts cover XML_Parse and callbacks, exclude parser
creation/destruction and file loading, and are distinct from elapsed time.

## Protocol and selection

The six pinned original project files, both 4 KiB/64 KiB feeds, both native
namespace modes and both actual CPython consumers are unchanged. These workloads
parse project XML; they do not build or execute the complete projects. Batik's
external DTD is not loaded. Both Python consumers enable namespaces.

Fresh normal Oriole builds use identical compiler settings and private
intermediate directories, with actual compiler invocations for all three library
crates. No allocator substitution, PGO or LTO setting change is part of this
comparison. The measured candidate shared/static hashes are:

- Shared: `3694b68326939778481e42e810f53facc2ebcb8b8ad75b599704dbfef8f37926`.
- Static: `39b1a6e450814a57cfe8fa724d21f0a4cadb124da3674ecb0d5d9de508f76f38`.
- Published performance control: `5058093123b486f7b6163ca055eb8726a5441f120bb4e74fcf7b892398ceab79`.
- Expat 2.8.4 reference: `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

Both cohorts use the coordinated CPU0 lane on the shared Linux AMD EPYC-Milan
host, seven seeded shuffled cohorts and the previous fixed iteration counts.
Other cores run validation and compilation. Native results retain 84 preflights,
588 workers and 105,420 measured parses plus 588 warmups. Python retains 72
preflights, 504 workers and 25,788 measured parses plus 504 warmups. Each worker
has one excluded warmup. Seeds are `2026091003` and `202609104412` respectively.
Creation, feed, finalization, callbacks and destruction are timed.

All six Python extensions compile identical unmodified CPython 3.12.13 sources
with matching headers and `-O2` flags. Workers verify loaded paths, hashes,
XML_Parse origin and canonical outputs. No measured worker failed, timed out,
was rerun or was discarded.

The isolated position and string studies use the earlier `5bc806e` / `5af2406b`
performance control. Their native ratios are 0.968× and 0.981×; their Python
ratios are 0.982× and 0.991×. They retain every original condition, build and
regression. Those gains cannot be added to predict the combined build. The
[unselected ASCII name scan](unselected-ascii-name/README.md) reduces instructions
but has no material native throughput gain and regresses in all four generated
conditions. It remains unselected; its six Python builds and 72 preflights have
no timed Python cohort.

## Retained setup failures and evidence

The isolated string native preparation initially scheduled CPU4 while its
unchanged controller required CPU3. It failed before creating worker output.
Rescheduling the same controller to CPU3 passed all 84 preflights; the original
failure remains recorded. Timing still uses CPU0.

An integrated test-inventory helper initially added three expected tests to both
binaries named `oriole`, including the CLI. It failed before writing the count
index. The original logs show precisely one changed row: the core library rises
from 51 to 54 tests; the other 32 binaries are unchanged. The corrected helper
checks that exact difference. The initial package consequently failed copying the
missing index after archive readback. Its receipt and the original helper are
retained, and the partial package remains locally archived. No compilation, test,
parser, preflight or timed worker was rerun for these postprocessing corrections.

[evidence.tar.gz](evidence.tar.gz) retains the integrated and isolated source,
builds, gates, instruction profiles, raw native/Python samples and controllers,
plus the completed timing/consumer reviews. Its SHA-256 is
`a040d01c9b0b36adb2e0d2bdcf9aab9859081bdd0b8138f970fa5a6094780b6e`.
The archive contains 8,332 regular members; 53 compiled binaries are excluded and
hashed. Every member was read back and compared with its recorded origin.
[archive-members.json](archive-members.json) records the complete mapping.
The ASCII name experiment keeps its own separate archive and readback index.
Outer reports and later independent review receipts are indexed by `files.json`.
Exact aggregate results are in [native-summary.json](native-summary.json) and
[python-summary.json](python-summary.json).
