# Streaming input and work limits

The C interface now accepts ordinary incremental XML streams beyond the former
256 MiB lifetime input limit. Cumulative indirect work and event payloads can grow
with **consumed root input**, while per-request, live-memory, entity and structural
bounds remain enforced. The Rust interface retains its default absolute limits.

On the tested 64-bit Linux build, normal and PGO Oriole completed all stream
shapes through 257 MiB with the same checked output and final positions as Expat.
A separate 2,049 MiB text stream also completed, including 128 suspension/resume
cycles and positions beyond signed 32-bit range. Strict shared/static CPython
checks retain the same two callback-boundary failures with the cleanup-patched
consumer described below. The completed benchmarks show nearly unchanged
real-project performance relative to the preceding parser, while the goal of
beating Expat remains unmet. These results do not establish production substitution.

## Final behavior

| Limit | C interface |
| --- | --- |
| One input call or buffer request | At most 256 MiB |
| Cumulative input to each original source | At most `min(c_long::MAX, isize::MAX)`; external children have independent source positions |
| Cumulative indirect work | `max(8 MiB, 100 × consumed original root bytes)` |
| Cumulative event payload | `max(64 MiB, 100 × consumed original root bytes)` |
| Live/reserved family allocations | At most 512 MiB, plus the existing allocation-amplification policy |

Buffered but unconsumed bytes and external-child input do not increase work
credit. Counters reject overflow even when a threshold saturates. Root reset
starts a new budget; retained children keep their original budget. Input-bound
rejection occurs before family accounting, allocator input credit or input-context
mutation. Token, depth, attribute, entity, cycle and child-construction limits
remain enabled.

Rust's new `Limits.max_work_amplification` defaults to `None`, preserving its
absolute 8 MiB work allowance and 256 MiB lifetime input limit. An explicit factor
makes the work allowance input-relative. See [policy and arithmetic details](POLICY.md)
for charge sites, source/encoding bounds, child/reset ownership and tests.

## Actual streaming results

The fixed harness used reusable chunks of at most 4 KiB and a caller-supplied
memory suite. **45/45 parser executions passed:** five stream shapes × three body
sizes (1, 65 and 257 MiB) × Expat, normal Oriole and PGO Oriole. This includes 30
Oriole executions and 15 reference executions. Repeated units round the requested
body size upward; XML wrappers add a few bytes.

| Stream shape | Input and checks | Oriole peak requested bytes | Expat peak requested bytes |
| --- | --- | ---: | ---: |
| Text | Direct input, text hash/count, element events, suspension/resumption | 92,483 | 15,084 |
| Repeated tags | ParseBuffer, explicit attributes, text and element events | 155,826 | 18,968 |
| Namespaces | ParseBuffer, expanded names, attributes, text and element events | 157,760 | 17,496 |
| Default attributes | Direct input, handlers absent, success and positions | 156,081 | 16,484 |
| Padded tags | Direct input, handlers absent, success and positions | 87,322 | 15,084 |

Each peak was constant across all three sizes. Normal and PGO Oriole had identical
peaks. Every run finished with **zero live requested bytes and zero live blocks**
after parser destruction. These are bytes requested from the application's memory
suite, excluding its bookkeeping and system-allocator overhead; they are not
process RSS measurements. The absent-handler modes do not establish delivered
default-attribute values. Callback hashes cover delivered data and order without
requiring identical text-callback splitting.

The preceding accepted Oriole build failed with resource error 43 for text,
namespaces and defaults at 65 MiB, and for every shape at 257 MiB. Its original
control results are retained alongside the new successful runs.

### Separate stream beyond 2 GiB

A separate bounded run parsed **2,049 MiB of text** with Expat and both Oriole
builds. All three consumed 2,148,532,231 total XML bytes, delivered 2,148,532,224
text bytes with matching hashes, completed 128 suspensions and 128 resumes, and
reported byte index and column 2,148,532,231 on line 1. Oriole's peak remained
92,483 requested bytes, and all three freed every tracked allocation. An independent saved-data audit
rechecked all 48 stream workers, their canonical results, source changes and
observed allocation peaks; its receipt is pinned in [evidence metadata](report.json).

This is an actual streamed document using the standalone text harness. It is
separate from the original upstream `test_misc_input_2gb`: its two active
configurations still hit the unchanged three-second test limit. The other ten
upstream configurations return early by the original fixture's design and do
not exercise a 2 GiB stream.

## Compatibility and safety checks

- **420 workspace tests and one doc test passed**, with formatting and strict
  all-target workspace Clippy. Twelve focused tests cover source bounds,
  positions/encodings, compaction, work amplification, future-input padding,
  counter overflow and child/reset ownership.
- **4,347 of 4,740 original API configurations passed:** 391 assertion failures
  and two timeouts remain. No passing configuration regressed. Exactly two rows
  changed from the earlier result: the active 2 GiB cases now time out instead
  of failing an `XML_Parse` assertion. No original assertion or resource bound
  was relaxed, and those cases are not counted as fixed.
- Dynamic and static C integration, adversarial and allocation checks passed,
  including 327 allocation scenarios per linkage. The integration source
  includes the previously published allocation-success regression coverage.
  **ASan/UBSan instrumented the C consumers; the Rust release libraries were
  not sanitizer-instrumented.** Leak detection was disabled for these C runs.
- Against the accepted Oriole control, 3,318 complete traces, 36,456 custom-encoding
  comparisons, 2,392 malformed-input pairs and 1,304 publication cases retained
  their checked behavior. The 38 lifecycle cases and six selected end-tag
  allocation cases also matched. Existing Expat callback/position differences
  remain recorded separately. Custom-encoding helpers retain counts, source and
  differences; they do not emit successful raw pairs.

The arithmetic prerequisite's first compilation failed on test borrow syntax.
Two subsequent Clippy attempts found test-style issues; their workspace tests
passed. The final checks passed after test-only repairs, with no changed assertion
or runtime behavior. All first-attempt outputs remain retained.

## CPython consumer checks

Strict CPython 3.12.13 checks ran against the fresh PGO library with both shared
and static linkage. The consumer source is revision
`3bb231a6a5dc02b95658877318bf61501a7209e9` with the
[pinned allocation-failure cleanup backport](../../../../integration/python-build-standalone/consumer-fix/cpython-3.12.13-external-parser.patch)
applied only to `Modules/pyexpat.c`. The six upstream test modules are unchanged;
there is no text-fragmentation relaxation or system-allocator override.

| Linkage | Method entries | Failures | Expected failures | Reported skips | Raw exit |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shared | 802 | 2 | 3 | 14 | 2 |
| Static | 802 | 2 | 3 | 14 | 2 |

Each method map contains 790 `ok` entries, two subtest-parent entries, two failures,
three expected failures and five skipped methods. The 14 reported skips count
**five methods, eight subtests and one class setup**; they are not 14 distinct
members of the 802-method map.

Both linkages retain the failures in `test.test_pyexpat.BufferTextTest.test1` and
`test.test_sax.CDATAHandlerTest.test_handlers`. Every method outcome matches the
accepted Oriole control. That earlier control used an **unmodified consumer**;
these new runs use the **cleanup-patched consumer**, so consumer source identity
is not claimed. The source and patch hashes are recorded in [metadata](report.json).

Each linkage verified four persistent-loader origins: initial and fresh imports
of `pyexpat` and `_elementtree`. The expected compiled extensions loaded in all
four checks; accelerator aliases and pure-Python/blocked import checks passed.
The collector reconstructed all saved method outcomes and checked module hashes;
that receipt is identified as a collector review, not an independent collection
audit. The two failures and raw exit 2 remain visible. These runs do not constitute
a full PBS build or a fresh consumer allocation-fault campaign.

## Benchmarks

The final policy was measured against the preceding accepted attribute parser
and Expat 2.8.4 in four fixed campaigns. The real-project inputs are the original
Vulkan registry, Wayland protocol, Maven POM, Batik SVG, GTK UI and DocBook XSL.
They are held out of generated-XML profile training. Each condition has seven
seeded process pairs, with one discarded warmup per timed worker.

| Consumer | Fixtures | Profile | Oriole / accepted control | Oriole / Expat | Conditions faster than control |
| --- | --- | --- | ---: | ---: | ---: |
| native C | real | normal | 0.98619× | 1.60391× | 21/24 |
| native C | real | PGO | 0.99735× | 1.37540× | 15/24 |
| native C | generated | normal | 1.01183× | 5.05404× | 0/4 |
| native C | generated | PGO | 1.00588× | 5.06024× | 1/4 |
| CPython | real | normal | 0.99950× | 1.26546× | 13/24 |
| CPython | real | PGO | 0.99707× | 1.13775× | 15/24 |

Ratios compare elapsed time; smaller is better. Each aggregate is the geometric
mean of its conditions' median paired ratios, calculated from raw samples. The
normal and PGO campaigns remain separate, as do native C and actual CPython.
Every condition and adverse result is retained in the
[independent numerical review](benchmark-review.json) and
[benchmark evidence](benchmark-evidence.tar.gz).

The streaming policy has nearly unchanged real-project performance relative to
the accepted parser. It does **not** meet the faster-than-Expat goal: the fixed PGO
real-project aggregate takes **1.38× Expat's native time and 1.14× its CPython
time**. Generated native conditions take about 5.06× Expat's PGO time. These are
one fixed campaign per mode, with no significance interval or causal claim;
shared host frequency, caches and memory bandwidth remain uncontrolled.

Native C uses 24 real-project conditions (4/64 KiB chunks, namespaces off/on)
and four generated controls. Each profile completed 84 preflights and 588 timed
workers. Callback hashes, element counts and text-byte counts match across the
three parsers. The inherited driver binds explicit library paths and before/after
hashes; it does not add a fresh same-process `XML_Parse` symbol-origin receipt.

Actual CPython uses 24 conditions (4/64 KiB chunks, ElementTree/pyexpat-events).
Each profile completed 72 preflights and 504 timed workers. Each worker proves
both loaded extension hashes and the pyexpat extension's `XML_Parse` library
origin. Timing includes parser construction, feed/finalization, callbacks/tree
creation and explicit result destruction. Imports, input reads, canonical output
checks and explicit `gc.collect()` are outside timing; automatic GC stays enabled.
Canonical pyexpat output coalesces adjacent text callbacks and does not replace
the strict compatibility suite.

Benchmark consumers use **unmodified CPython 3.12.13 source**, whereas the strict
shared/static checks above use the cleanup backport. No patched-consumer timing
is inferred. The first normal preparation built its consumers, then its preflight
stopped at the old CPU4 guard before parser execution. The retained second
preparation changed that guard to the scheduled CPU3; fixtures, assertions, worker
code, seeds, iteration counts and time limits stayed intact. The original PGO
preparation was not executed.

The independent audit reconstructed all **2,496 workers and 265,080 samples**,
including preflights and warmups, and verified all raw medians, seeded engine
orders, output hashes, source/library bindings and parent comparisons. It also
reconstructed the main README's six-condition display from 84 worker medians and
42 paired ratios. That display is a subset of the full real-project cohort.
See [benchmark metadata](benchmarks.json) and the
[archive member index](benchmark-evidence-members.json.gz). The
[source-origin map](benchmark-source-origins.json) connects every original input
path to archived bytes or an explicitly excluded compiled binary.

## Current-source sanitizer follow-up

A fresh Rust AddressSanitizer build passed all 13 focused policy regressions,
68,897 retained-corpus replay executions and seven 120-second campaigns totaling
2,170,731 executed units. No failure artifacts or target retries occurred. Six
harnesses are unchanged; a separate seventh lowers the initial work allowance and
enables relative work so fuzz inputs can exercise the new policy.

The [sanitizer report](ASAN.md) retains exact commands, source and instrumentation
proof, raw logs, corpus bytes and origin maps. These are new instrumented binaries
for the same source; the PGO benchmark binaries remain separate. Leak detection
was disabled. The earlier six 600-second campaigns retain their historical scope. The
[independent sanitizer audit](asan-independent-review/independent-audit.json)
checks all compiler records, commands, raw outcomes, corpus origins and archive
members.

## Evidence and remaining gates

[Evidence metadata](report.json) identifies the exact source, libraries, commands
and saved results. The implementation is based on PR122
`1058868a7067671a1cb3b5f0de571e47cfcafe17`; its 70-file build manifest is
`e52c4fa16e71a7dcd76864b29509b157a42b6fbe4531f87b68f98395418d5fc3`.
The normal and PGO builds share that source. Documentation updates follow the
frozen runtime and test files.

The stream workers retained their original 120-second CPU, 150-second wall and
1 GiB address-space bounds. The upstream API matrix retained three seconds per
configuration, 1 GiB address space, 768 MiB RSS and 240 seconds total. The streaming and compatibility workers are correctness runs; their wall times
are not the benchmark results above. Current-source distribution validation
remains a separate gate before advancing deployment claims.

The [core evidence archive](evidence.tar.gz) retains the compact source, workspace,
C/API, large-stream and strict CPython records, including original failures. Its
[member index](evidence-members.json.gz) maps original paths to archived files and
identical-byte aliases. [Benchmark packaging](benchmark-evidence-archive.json)
and [core packaging](evidence-archive.json) record deterministic member hashes and
complete readback. Libraries, compiled extensions, interpreters, build caches and
historical nested archives are excluded; source and binary identities remain in
the saved manifests. Current Rust sanitizer evidence is retained in the separate [ASan archive](asan-evidence.tar.gz).


The [independent archive readback](package-root-review.json) checks every member
and identical-byte alias against its index and retained original file. To verify
the published archives without running parser targets or extracting files:

```console
python3 docs/validation/2026-09-11/streaming-input-bounds/verify-packages.py
```

The optional `--check-originals` flag also verifies the collector's original
absolute paths when they are available on the same machine.
