# Bounded search for coalesced character data

The parser now searches up to 65,537 bytes for `<` or `&` before using its existing
scalar text-boundary routine. A delimiter in that prefix, or a complete span of
at most 65,536 bytes, resolves the same boundary directly. Longer unresolved spans
keep the original scan. This preserves markup precedence at the cutoff, CRLF,
Unicode boundaries, converted-buffer limits and existing callback grouping.

The runtime commit is `193e1278ae059bad3a7890b4e34b9a91be5296e8`. All 69 frozen
source entries match the measured candidate exactly. It is based on the doc-only
`acc0fc6` layer above `5bc806e`; no runtime or test bytes changed during assembly.

## Compatibility

| Check | Result |
| --- | --- |
| Workspace tests, formatting and strict Clippy | 394 tests pass; both checks pass |
| Pure selector design comparisons | 5,449,334 match the original boundary algorithm |
| Strict callback/status/error/position comparison | 1,518 cases, no baseline differences |
| Malformed text and encoding boundaries | 2,392 cases, no baseline differences |
| Custom aliases and external multibyte payloads | 36,456 cases, no baseline differences |
| Coordinate/context/default/suspension probes | 32 conditions; 16 exact baseline pairs |
| Original public API matrix, canonical bounds | 4,347 pass / 393 fail; all 4,740 outcomes unchanged |
| Unchanged CPython 3.12.13 XML suites | Shared and static each run 802 tests: two failures, 14 skips, three expected failures |

The original CPython failures remain `BufferTextTest.test1` and
`CDATAHandlerTest.test_handlers`, which assert Expat's text grouping. Their raw
suite and gate exit codes remain 2. The persistent import finder and separate
origin probe verify initial, fresh and blocked accelerator imports. There is no
consumer source adaptation or text-fragmentation waiver in these suite runs.

The first API run used relaxed bounds: 15 seconds per configuration, 4 GiB
address space, 3 GiB RSS and 900 seconds total. Its identical outcomes did not
establish compatibility under the original bounds. Independent review caught
that evidence gap. A separate complete rerun uses the original **3 seconds,
1 GiB address space, 768 MiB RSS and 240 seconds total**. It independently verifies
the library origin and records the same 4,347/393 results, with original exit 1.
Both runs and their actual manifests are retained; no failing assertion is
reclassified. See [the per-test failure mapping](../detached-frames/api-classification/).

## Performance

All comparisons use fresh normal Oriole builds with identical compiler settings,
private intermediate directories and actual compiler invocations for all three
library crates. The candidate is `5058093123b486f7b6163ca055eb8726a5441f120bb4e74fcf7b892398ceab79`;
the published `5bc806e` control is `5af2406b17092dad35aa61c086cdfe39e24f51e653441fd9baf50cd12da3b2fe`.
The pinned Expat 2.8.4 reference is `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.
The candidate static archive used by the compatibility suite is
`16773ed1c48163905967698b1162a9d3a880c952a1ec3ac11cba9215f6520c6e`.

| Cohort | Conditions | Candidate / published | Candidate / Expat | Faster than published | Faster than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native real-project XML | 24 | 0.967× | 2.041× | 20 | 0 |
| Actual CPython consumers | 24 | 0.976× | 1.446× | 22 | 2 |
| ElementTree | 12 | 0.972× | 1.563× | 11 | 0 |
| pyexpat callbacks | 12 | 0.980× | 1.338× | 11 | 2 |
| Native generated adverse fixtures | 4 | 0.996× | 5.358× | 2 | 0 |

Aggregates are geometric means of all per-condition paired median time ratios;
lower is faster. The change reduces native project time by **3.3%** and CPython
time by **2.4%**. Every condition is retained. Native namespace-enabled Batik and
DocBook regress by 0.10–1.70%; rare declarations regress by 0.097% at 4 KiB and
1.020% at 64 KiB. Python regressions are Batik pyexpat at 4 KiB (+0.221%) and
DocBook ElementTree at 4 KiB (+0.943%). Only the two Wayland pyexpat conditions
beat Expat. This does not establish broad faster-than-Expat performance.

The six pinned project inputs, both 4 KiB/64 KiB feeds, both native namespace
modes and both actual CPython consumers are unchanged. Batik's external DTD is
not loaded. These workloads parse original project XML; they do not build or run
the full projects. The shared host is Linux on AMD EPYC-Milan. Both cohorts use
the coordinated CPU0 lane, seven seeded shuffled cohorts and the previous fixed
iteration counts. Other pinned cores ran sanitizer or validation work.

Native measurements retain 84 preflights, 588 workers and 105,420 measured parses
plus 588 warmups. Python retains 72 preflights, 504 workers and 25,788 measured
parses plus 504 warmups. Python compiles identical unmodified CPython 3.12.13
extension sources with identical `-O2` flags and headers for all three engines;
workers check loaded paths, hashes, XML_Parse origin and canonical outputs.
Creation, feed, finalization, callbacks and destruction are timed. One warmup per
worker is excluded. Seeds are `2026091003` and `202609104412`, respectively.
No measured worker failed, timed out, was rerun or was discarded.

The separate instruction screen retains 32 preflights and 32 Callgrind runs.
All 12 project conditions at 4 KiB use fewer instructions, with a 3.55% geometric
mean reduction. Generated controls cover both feed sizes, including the
64 KiB rare-declaration increase. Instruction counts measure XML_Parse and its
callbacks; constructor/free/input loading are outside that collection scope.
They are separate from elapsed-time results.

## Retained failures and evidence

Strict Clippy initially requested two test-only byte-string spellings. The
corrected source retains the same runtime bytes; both check histories remain.
The Python preparation initially scheduled CPU3 while the copied controller
required CPU2. It exited before loading a build or starting a worker. Only the
preflight affinity guard was corrected; timing still requires CPU0. The complete
initial failure and subsequent 72 successful preflights remain separate.

No allocator substitution, PGO or LTO setting change is part of this comparison.
The longer Rust ASan and full PBS distribution reports on earlier runtimes remain
separately scoped; they are not new campaigns or distribution builds of this
candidate. Existing callback/position differences from Expat remain documented.

`evidence.tar.gz` preserves the source/build/gate/profile study, both API runs,
strict consumer suites, all native/Python samples and controllers, exact source
assembly and independent reviews. `archive-members.json` lists every member and
hash, with compiled binaries excluded and their hashes retained. Every included
member was read back and compared with its recorded origin. `files.json` indexes
the outer reports, including the separately retained selector-design oracle. Exact aggregates are in [native-summary.json](native-summary.json)
and [python-summary.json](python-summary.json).
