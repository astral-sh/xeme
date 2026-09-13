# Lower literal start tags directly into adapter frames

Validated literal tags now fill the detached callback frame before entering the
generic owned-event and namespace machinery. The eligibility rules are unchanged:
a native UTF-8 root source, no DTD or conversion metadata, bounded arena storage,
and names whose namespace expansion is identity. Other tags retain the generic
path. Duplicate detection, fallible attribute copies, stack ownership and
empty-element end events keep their existing order.

The selected runtime is `50c20c1e73eeaacabb9df90e413f502361fb45b3`.
Its source combines the exact three-file isolated patch with the preceding
matching closing-tag optimization. All 70 frozen entries match the committed
source. The build began on `503a559`; advancing through the doc-only `382891b`
parent changed none of those bytes.

## Compatibility

| Check | Result |
| --- | --- |
| Workspace | 398 tests across 33 binaries, plus one separate doc test; all pass |
| Formatting and strict Clippy | Pass |
| Original public API matrix | 4,347 pass / 393 fail; all 4,740 outcomes unchanged |
| Native C consumers | Six shared/static ASan/UBSan programs pass; original 327 allocation scenarios per linkage |
| Strict callback/status/error/position comparison | 3,318 exact baseline traces |
| Malformed text and encodings | 2,392 exact baseline comparisons |
| Custom aliases and external multibyte payloads | 36,456 baseline comparisons with zero differences |
| Callback publication, raw context and suspension | 1,304 cases / 3,912 library parses; exact baseline records |
| Additional End-handler lifecycle cases | 38 cases / 114 library parses; exact candidate/control records |
| Additional End-handler allocation cases | Six per library: four forced failures and two successes; exact candidate/control records |
| Unchanged CPython 3.12.13 suites | Shared and static each run 802 tests, with two failures, 14 skips and three expected failures |

The direct control is the preceding `503a559` / `dd16d23e` build. The original
API bounds remain 3 seconds per configuration, 1 GiB address space, 768 MiB RSS
and 240 seconds overall. Its original exit remains 1. Both strict CPython suites
retain exit 2 and the same `BufferTextTest.test1` and
`CDATAHandlerTest.test_handlers` text-grouping failures. The 14 reported skips
comprise five whole methods, eight subtests and one class setup. Fresh accelerator
imports and library origins are verified; no consumer adaptation or fragmentation
waiver changes the results.

Focused tests compare detached and owned events across the linear/hash duplicate
threshold, both XML name editions, namespace modes and incremental feeds. The
selected-allocation failure workload now also covers nine-attribute literal tags.
The existing source, raw-token, budget, namespace and callback tests remain active.
The new path retains value-before-name copies and publishes the Start frame only
after the empty tag's fallible End work succeeds. It adds no unsafe code.

Existing differences from Expat remain explicit, including Default and external
entity positions and UTF-16 namespace callback coordinates. Post-parse context
snapshots are outside the header's handler-scoped context contract. Successful
custom-alias raw traces are not individually retained; executed controllers,
generated templates and zero-difference reports are. C sanitizers instrument the
consumers, with normal-release Rust and leak detection disabled. Earlier sustained
Rust ASan and PBS reports retain their original source scopes. See the
[API failure classification](../detached-frames/api-classification/).

## Performance

| Cohort | Conditions | Candidate / published | Candidate / Expat | Faster than published | Faster than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native project XML | 24 | 0.907× | 1.750× | 24 | 0 |
| Actual CPython consumers | 24 | 0.956× | 1.321× | 24 | 3 |
| ElementTree | 12 | 0.948× | 1.403× | 12 | 1 |
| pyexpat callbacks | 12 | 0.964× | 1.243× | 12 | 2 |
| Native generated adverse fixtures | 4 | 0.961× | 5.244× | 4 | 0 |

These are geometric means of all per-condition paired median time ratios; lower
is faster. Native project time decreases by **9.3%** and actual CPython time by
**4.4%** against the preceding runtime. Every measured condition improves against
that control. Three Wayland consumer conditions beat Expat: both pyexpat feed
sizes and ElementTree at 64 KiB. The latter is a small 0.5% measured margin on a
shared host. Overall native and CPython performance remains slower than Expat.

The six pinned original project inputs, 4 KiB/64 KiB feeds, both native namespace
modes and both actual CPython consumers are unchanged. These workloads parse
project XML; they do not build or execute entire projects. Batik's external DTD
is not loaded. Both Python consumers enable namespaces.

Fresh normal Oriole builds use matching compiler settings and private intermediate
directories, with three actual workspace compiler invocations. This comparison
introduces no allocator, PGO or LTO setting change. The measured identities are:

- Candidate shared: `9815cc852327d455c094b0fb72c1d33fb90a4571e67cf15965e96e2300376828`.
- Candidate static: `92eafd42ffe68dc30db920ff5ff8d67357b2c6c4273f51b0e3e841b2bfb9ea69`.
- Published control: `dd16d23e4ab67938c195970bd845b2a159d7f062beda265bb673c85b79f73fc6`.
- Expat 2.8.4 reference: `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

Both cohorts use CPU0 on the shared Linux AMD EPYC-Milan host. Other cores run
validation and compilation. Seven seeded shuffled cohorts retain every condition
and sample. Native results include 84 preflights, 588 timed workers and 105,420
measured parses plus 588 warmups. Python retains 72 preflights, 504 timed workers
and 25,788 measured parses plus 504 warmups. Each worker excludes one warmup;
seeds are `2026091003` and `202609104412`. Creation, feed, finalization, callbacks
and destruction are timed. Each condition's ratio is the median of its paired
ratios, not the quotient of rounded table times.

Six Python extensions compile identical unmodified CPython 3.12.13 sources with
matching headers and `-O2` flags. Workers verify loaded paths, hashes, XML_Parse
origin and canonical output. No measured worker failed, timed out, was rerun or
was discarded.

## Isolated study and evidence

The isolated `c43531f7` build uses `64a270e` / `3694b683` as its control.
It reduces native time by 8.2% across all 24 project conditions and Python time
by 3.8% across all 24 consumer conditions. All native conditions and 23 Python
conditions improve. DocBook ElementTree at 4 KiB regresses by 0.052%; that result
remains included. Its full workspace count is 396 tests plus one doc test, and
its original API and strict CPython outcomes remain unchanged.

The isolated instruction screen retains 32 profiles and 32 preflights across
12 project conditions and four generated conditions. All project instruction
counts decrease, with a 4.11% geometric mean reduction; all four generated
counts also decrease slightly. These instruction counts exclude creation,
destruction and file loading and do not establish elapsed-time gains. Isolated
and integrated gains cannot be added. The initial isolated compile failed due
to an incorrect test enum variant; the original attempt is retained separately
from the successful frozen source and build.

[evidence.tar.gz](evidence.tar.gz) contains 4,949 regular members. Its 32 direct
compiled artifacts are excluded and hashed; original native binaries remain in
the nested standalone handoff. Its SHA-256 is
`5b3f33a1c08befe2e1b45a214003ba3962694846a0015430cc5d428c91e05217`.
Every member was read back and compared with its recorded origin.
[archive-members.json](archive-members.json) records that mapping. The isolated
source/build/gates/profiles and first compile failure are retained in a nested,
individually indexed handoff archive. Later selected-result and final-documentation
reviews remain separate outer receipts. Full aggregates are in
[native-summary.json](native-summary.json) and [python-summary.json](python-summary.json).
