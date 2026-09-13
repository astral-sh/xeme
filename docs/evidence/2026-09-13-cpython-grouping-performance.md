# CPython callback performance follow-up — 2026-09-13

The [initial grouping fix](2026-09-13-cpython-grouping.md#performance-cost-screen)
restored CPython's expected callbacks but increased parser work for every line.
This follow-up reduces repeated parser work while keeping those callback boundaries.

## Merge-rebase qualification

These tests and measurements predate the merge rebase onto `03c21b1`. Upstream
changes include bounded prolog whitespace scanning and an index of attributes
with default values. Upstream also removed the PBS integration and moved the
cleanup backport to
[`tools/cpython/consumer-fix`](../../tools/cpython/consumer-fix/README.md).
The source and artifact hashes below identify the measured pre-rebase builds;
these results do not qualify a rebuilt, rebased artifact.

## Implementation and profile findings

The initial implementation sends each text token through the general parser event
path. Callgrind attributed 24.38 million self instructions to `parse_text` on the
generated text control. On Wayland, `parse_text`, general event dispatch, text
preparation and per-token accounting also appeared among the largest costs.

The final implementation delivers eligible ASCII text directly into the existing
C adapter frame. It uses the same validated text span, original input range,
position update and accounting operation as the ordinary path. It still publishes
each callback separately. Pending events, parser continuations, converted input,
external/internal entity sources and uncertain text retain the ordinary path.

| Callgrind input | Initial corrected implementation | Selected runtime | Instruction change |
| --- | ---: | ---: | ---: |
| Generated text | 94,121,571 | 63,128,104 | −32.9% |
| Wayland | 46,218,711 | 37,587,198 | −18.7% |

These are instrumented instruction counts for four parses with the same O2 native
driver and inputs, including consumer work, not elapsed-time measurements. `perf`
sampling was unavailable under the host's `perf_event_paranoid=4` policy. Before
the accounting changes, the native-text prototype's text profile still called
`Source::accounting_bytes` twice per text token: 160,856 calls
and 2,573,696 self instructions. The final implementation inlines that accounting
query. It also avoids an atomic update when the budget has exactly one shared
owner, using exclusive mutable access after an Acquire ownership check. Budgets
shared with external parsers keep the atomic path. Both paths use the same limit
validator and preserve each charge, overflow check and rejected-charge state.

## Compatibility checks

The selected runtime with native text delivery and exclusive accounting passed
all six unchanged CPython XML suites with both shared and static parser libraries
and the strict gate: 803 discovered cases, 802 run, 14 skips, complete inventory and
passing semantic checks. The CPython sources had no consumer adaptations, and
the runner verified the loaded extension origins. An independent local C driver
also compared 8,144 configurations with the initial corrected implementation and
found no changes to exact event payloads, callback positions and byte counts,
`XML_DefaultCurrent` raw text, or parse/suspension results. Its matrix included
UTF-8, UTF-16, custom encodings, input chunking, `XML_ParseBuffer`, handler
replacement, malformed suffixes, entities and CDATA. This comparison establishes
preservation of the corrected behavior on those inputs, not universal agreement
with Expat's exact callback fragmentation.

All 529 workspace tests and doctests passed across 38 suites. Workspace Clippy
with warnings denied, Rust formatting, Ruff, ty and the nine CPython gate tests
also passed. Focused tests check unique/shared ownership transitions, allocation
failure prefixes and identical accounting updates on limit rejection or overflow.

The final candidate also passed native integration and adversarial checks,
including 356 allocation-failure scenarios. The Expat API gate retained 4,231
passes and the same 509 known failures across 4,740 rows; reference Expat passed
every row. The W3C gate checked 6,003 rows per engine and retained the same 960
known failures in both. All 2,730 differential semantic comparisons passed; the
separate strict observations retained eight grouping and 84 position differences.
`final-runtime-validation/summary.json` under the local grouping artifact directory
records these gates and the tested library.

## Tested source

The selected runtime is local `experiment4`, built from base
`56e7194b4e5f2fc300cf926db3dd0d9c4bf47b9f` plus the
callback fix, native-text path, accounting-query inlining and exclusive budget
updates. All 37 checked source and header files match the final workspace
validation byte-for-byte; the unrelated lockfile ordering change was excluded.

| Frozen artifact | SHA-256 |
| --- | --- |
| Pre-fix Xeme shared library | `547ed8f1779b8f424a968d2191ccf82ee7361a9199912ff1aafd531b2e866cdd` |
| Initial corrected shared library | `93443b3d344eb97d2e079eb831a07abd09d886f3b88ea6fbab33c43241bfde1c` |
| Initial native-text prototype shared library (`experiment1`) | `44830bc45da71a1b88836efe0de2bc267d40ccf2d51e4051b6bc1026a71118c6` |
| Selected shared library (`experiment4`) | `f5b943bd5d7aa0a66090759577d67405543d3be2b8cf83a1604573a64f83767e` |
| Matching selected static library (`experiment4`) | `780b6d4e6b4376224b78c90bced2d6f6f76c18aefd599e12c32331c7f1e866f6` |
| Pinned normal Expat 2.8.4 control | `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478` |

The shared and static libraries came from the same fresh compiler-artifact record,
using ordinary O3, ThinLTO, one codegen unit and generic x86-64. The build command
used `cargo +ohm -Zohm-defaults=no rustc --locked --release` through the benchmark
builder; `build.json` and `static-artifact.json` retain full commands and source
hashes.

## Confirmatory timings

The final CPU 6 comparisons use two fresh epochs: the selected runtime against
the initial corrected implementation, then against pre-fix Xeme. Each also
includes the same pinned Expat control. Both 64-condition epochs passed, with
21,840 measured samples independently reconstructed in each. The epochs remain
separate.

### Compared with the initial corrected implementation

The selected runtime took **11.0% less native time and 6.0% less CPython time** on
the six tuning projects. All 48 real-project conditions improved in this epoch.
An independent reconstruction verified all 21,840 measured samples.

| Group | Conditions | Selected / initial corrected | Selected / Expat | Slower conditions |
| --- | ---: | ---: | ---: | ---: |
| Native, tuning projects | 24 | 0.8897× | 1.2153× | 0 |
| CPython, tuning projects | 24 | 0.9400× | 1.1737× | 0 |
| Native, generated controls | 16 | 0.8753× | 1.7693× | 2 |

The two slower generated conditions were elements without namespaces: **0.49%
more time with 4 KiB feeds and 2.21% more with 64 KiB feeds**. These remain in the
complete condition report.

### Compared with pre-fix Xeme

Preserving the corrected callbacks still took **2.04% more native time and 11.84%
more CPython time** than pre-fix Xeme on the tuning projects. The optimization
reduces the initial correction's overhead; it does not eliminate that cost.

| Group | Conditions | Selected / pre-fix Xeme | Selected / Expat | Slower conditions |
| --- | ---: | ---: | ---: | ---: |
| Native, tuning projects | 24 | 1.0204× | 1.2158× | 10 |
| CPython, tuning projects | 24 | 1.1184× | 1.1765× | 23 |
| Native, generated controls | 16 | 1.1936× | 1.7685× | 12 |

The largest real native regression was Wayland with 64 KiB feeds and namespaces
disabled, at **1.1899× pre-fix time**. The largest CPython regression was Wayland
with 64 KiB feeds and pyexpat events, at **1.5770×**. The largest generated native
regression was text with 4 KiB feeds and namespaces, at **1.9488×**. All conditions
and project-level summaries remain in the raw reports and reconstruction.

Untimed checks verified the additional text callbacks directly: Wayland at 64 KiB
feeds produced 5,947 callbacks with both the selected runtime and Expat, versus
1,136 with pre-fix Xeme. All engines preserved the same text characters and
canonical output on that fixture.

### Focused generated Python control

A separate CPU 6 control using generated text, 4 KiB feeds and pyexpat events
took **14.39× pre-fix Xeme's time**, despite taking 19.4% less time than
the initial corrected implementation. This fixture produces many more callback
invocations after the line-boundary correction, and that consumer cost remains:
the selected runtime and Expat produced 20,107 text callbacks versus pre-fix
Xeme's 108, preserving the same text and canonical output.
The control used seven paired rounds and 30 measured parses per process after a
warmup. It is excluded from both the canonical native-generated and Python
real-project aggregates; its results are in `experiment4-targeted-cpu6/report.json`.

### Protocol and limitations

Each epoch follows the [benchmark guide](../../benchmarks/HILLCLIMB.md) confirmation
protocol with all 64 canonical conditions: 24 native conditions from the six
existing tuning projects, 24 CPython conditions from those same projects, and 16
generated controls. Native conditions cover 4/64 KiB feeds and namespaces on/off;
CPython conditions cover ElementTree and pyexpat with both feed sizes. Generated
controls remain separate from real-project aggregates.

There are seven randomized paired rounds, with 20 native or 10 CPython measured
parses per process after a discarded warmup. Each condition uses the median of
paired process-median ratios; group summaries are equal-weight geometric means
of those condition ratios. All three engines' consumers use clean, unmodified
pinned CPython 3.12.13 sources compiled with identical O2 settings. The runner
checks each timed output against its canonical or normalized callback contract.

The earlier CPU 0 epochs showed competing host activity and large process
outliers, including roughly 1.1 ms and 4.1 ms for the same library and native
Wayland condition. They remain in `experiment1-screen-corrected` and
`experiment1-screen-original`, with their commands in `full-screens-run.json`.
They are not pooled with the CPU 6 confirmations or used for the final performance
claims. This task's builds and validation jobs finished before the CPU 6 timings;
other users' activity, CPU frequency, caches and memory bandwidth remained
uncontrolled on the shared Linux host. The corrected epoch ran from 16:57:21 to
17:05:24 UTC; the pre-fix epoch ran from 17:05:24 to 17:13:19 UTC on September 13.

Local artifacts under
`/tmp/xeme-c037-cpython-grouping/performance-followup/` retain source snapshots,
build records, the profiling driver and raw Callgrind data. Final timing outputs
are `experiment4-confirm-corrected` and `experiment4-confirm-original`;
`full-confirmations-run.json` records exact commands and timestamps. The strict
CPython and exact-trace comparison reports are under `/tmp/xeme-c037-grouping-opt/`.

| Confirmation artifact | SHA-256 |
| --- | --- |
| Initial-corrected comparison report | `36cd8e93f4e2fbad926eb97101bb4e5aa6e2cbdb7ce45e18640e142d44b7ffdb` |
| Initial-corrected complete condition CSV | `8f1d2c1aa3716c0239571dd81a89ad77e7f6ab750a1df7da63b5cc88c9ee6646` |
| Pre-fix comparison report | `89637073ddd0a02c557f373f33db6baa74f8bd6db828796a83d76e97f7cf7e60` |
| Pre-fix complete condition CSV | `6ca8fce9371c767e5cacc3d83e72bdd65fcd0ad45de1679068b97186c9c75a80` |
| Independent reconstruction of both epochs | `f07b2a2f41fe6f575007b539a33eec7323c6860d5ddc0f2426a48bf9ee825f38` |
| Final callback-count checks | `3dffd1390b4fd0dc880b721c57865eb9a971379e15bc42c40e8f355965f9a7c9` |
| Focused CPU 6 control report | `9550ec1fe9192a12cb50b580a769b8ef1fc0ad7c41865943c747dc86444db8a8` |

These files have not been published as durable release assets. Installed PBS
distributions, independent holdout performance and other platforms require
separate qualification.
