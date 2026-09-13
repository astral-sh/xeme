# Reuse validated opening names for closing tags

For a complete native UTF-8 closing tag, we compare its name with the validated
opening name on the element stack. A match avoids repeating lexical and name
validation. Incomplete tags, whitespace before `>`, mismatches, converted
encodings, entity sources and fragments retain the existing scanner. Checking
the expected closing delimiter before comparing a long name prevents repeated
prefix comparisons on one-byte feeds with reparse deferral disabled.

The selected runtime is `503a55957f7cdebcc0b3fb3ada2751ffdb8cb9c9`.
It combines the reviewed closing-tag change with the preceding text search,
short position updates and string inline hints. All 70 frozen entries match
the committed source. The build began on `64a270e`; advancing through the
doc-only `71bf6bc` parent changed none of those bytes.

## Compatibility

| Check | Result |
| --- | --- |
| Workspace | 397 tests across 33 binaries, plus one separate doc test; all pass |
| Formatting and strict Clippy | Pass |
| Original public API matrix | 4,347 pass / 393 fail; all 4,740 outcomes unchanged |
| Native C consumers | Six shared/static ASan/UBSan programs pass; original 327 allocation scenarios per linkage |
| Strict callback/status/error/position comparison | 3,318 exact baseline traces |
| Malformed text and encodings | 2,392 exact baseline comparisons |
| Custom aliases and external multibyte payloads | 36,456 baseline comparisons with zero differences |
| Callback publication, raw context and suspension | 1,304 cases / 3,912 library parses; exact baseline records |
| Additional End-handler lifecycle cases | 38 cases / 114 library parses; exact candidate/control records |
| Additional End-handler allocation cases | Six per library: four forced failures and two successes; all 37 records match |
| Unchanged CPython 3.12.13 suites | Shared and static each run 802 tests, with two failures, 14 skips and three expected failures |

The API harness keeps its original 3-second per-configuration timeout, 1 GiB
address-space bound, 768 MiB RSS bound and 240-second total timeout. Its original
exit remains 1. Both strict CPython suites retain exit 2 and the same
`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` text-grouping
failures. The 14 reported skips comprise five whole methods, eight subtests and
one class setup. Fresh accelerator imports and library origins are verified. No
consumer adaptation or text-fragmentation waiver changes these outcomes.

The shortcut retains the original accounting, owned-token allocation, raw-token
publication, namespace restoration, queueing and source-consumption order.
Focused tests cover every closing-tag byte split, Unicode names, long names,
disabled deferral, token limits and fallback syntax. Additional C controls cover
End-handler mutation, namespace undo, DefaultCurrent, suspension, allocation
failure, terminal retry and selected-suite cleanup.

The direct behavior control is the preceding `64a270e` / `3694b683` build.
Existing differences from Expat remain explicit, including Default and external
entity positions and UTF-16 namespace callback coordinates. Post-parse context
snapshots are outside the header’s handler-scoped context contract. Successful
custom-alias raw traces are not individually retained;
their executed controllers, generated templates and zero-difference reports are.
C sanitizers instrument the consumers; Rust uses its normal release build and
leak detection is disabled. Earlier sustained Rust ASan and PBS reports keep
their original source scopes. See the
[API failure classification](../detached-frames/api-classification/).

## Performance

| Cohort | Conditions | Candidate / published | Candidate / Expat | Faster than published | Faster than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native project XML | 24 | 0.949× | 1.906× | 24 | 0 |
| Actual CPython consumers | 24 | 0.972× | 1.384× | 24 | 2 |
| ElementTree | 12 | 0.970× | 1.492× | 12 | 0 |
| pyexpat callbacks | 12 | 0.974× | 1.283× | 12 | 2 |
| Native generated adverse fixtures | 4 | 0.967× | 5.300× | 4 | 0 |

These are geometric means of all per-condition paired median time ratios; lower
is faster. Compared with the preceding runtime, native project time decreases
by **5.1%** and actual CPython time by **2.8%**. Every measured condition improves
against that control. Only the two Wayland pyexpat conditions beat Expat;
the overall native and CPython results remain slower than Expat.

The six pinned original project files, 4 KiB/64 KiB feeds, both native namespace
modes and both actual CPython consumers are unchanged. These workloads parse
project XML; they do not build or execute entire projects. Batik's external DTD
is not loaded. Both Python consumers enable namespaces.

Fresh normal Oriole builds use matching compiler settings and private intermediate
directories, with three actual workspace compiler invocations. This comparison
introduces no allocator, PGO or LTO setting change. The measured identities are:

- Candidate shared: `dd16d23e4ab67938c195970bd845b2a159d7f062beda265bb673c85b79f73fc6`.
- Candidate static: `fbbef92d1c22b71850016a12e6c71c2ba1adcf20430e8934696d3dee07485a28`.
- Published control: `3694b68326939778481e42e810f53facc2ebcb8b8ad75b599704dbfef8f37926`.
- Expat 2.8.4 reference: `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

Both cohorts use the coordinated CPU0 lane on the shared Linux AMD EPYC-Milan
host, seven seeded shuffled cohorts and unchanged iteration counts. Other cores
run validation and compilation. Native results retain 84 preflights, 588 timed
workers and 105,420 measured parses plus 588 warmups. Python retains 72
preflights, 504 timed workers and 25,788 measured parses plus 504 warmups. Each
worker excludes one warmup. Seeds are `2026091003` and `202609104412`.
Creation, feed, finalization, callbacks and destruction are timed.

All six Python extensions compile identical unmodified CPython 3.12.13 sources
with matching headers and `-O2` flags. Workers check loaded paths, hashes,
XML_Parse origin and canonical output. No measured worker failed, timed out,
was rerun or was discarded.

## Isolated study and retained evidence

The isolated `fdea997d` build uses the earlier `5bc806e` / `5af2406b` control.
It reduces native time by 2.7% and Python time by 1.8%, improving 19/24 and
22/24 conditions respectively. Its five native regressions are all four Batik
conditions and namespace-enabled DocBook at 64 KiB; its two Python regressions
are Wayland pyexpat and Batik ElementTree at 4 KiB. All remain in the evidence.
Its instruction screen retains 32 profiles and 32 preflights: all 12 project
conditions improve, with a 3.82% geometric mean reduction; all four generated
counts also decrease. Instruction counts exclude creation/destruction and file
loading, and do not establish elapsed-time gains. Isolated gains cannot be
added to predict the combined build.

The first isolated supplemental allocation helper assumed every retry after
an allocation failure would return NO_MEMORY. Its baseline run showed the
existing resumable-stop state can return SUSPENDED (33) instead. This happened
before the candidate ran. The failed helper and output are preserved; the final
helper compares exact baseline/candidate errors and asserts no extra callback
or allocation on retry and zero live selected blocks after destruction. No
original upstream assertion or parser behavior was changed for this correction.

[evidence.tar.gz](evidence.tar.gz) retains both source/build studies, original
gates, supplemental controls, isolated instruction profiles, raw native/Python
samples and controllers. Its SHA-256 is
`bf518d52a5a1c5a2e47d209e1e2f1b33f5f2c1d257cc928300783668f322ecbb`.
The archive contains 5,205 regular members; 34 compiled binaries are excluded
and hashed. Every member was read back and compared with its recorded origin.
[archive-members.json](archive-members.json) records that mapping. Later
independent timing, consumer, assembly and documentation audits remain separate
outer receipts in `files.json`. Complete aggregates are in
[native-summary.json](native-summary.json) and [python-summary.json](python-summary.json).
