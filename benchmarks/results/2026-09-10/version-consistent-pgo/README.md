# Final native and consumer measurements with PGO

Profile-guided optimization reduces Oriole's native project time by **22.7%** and
Expat's by **13.4%**. With both trained, Oriole still takes **2.23×** Expat's time
across all 24 native conditions. Actual CPython consumer time falls by **16.7%**;
the corresponding comparison with trained Expat is **1.42×**. These results do not
meet the broader faster-than-Expat goal.

## Source and builds

All Oriole measurements use runtime `4b11ace3d89fe6b7bf66ca55290eb23444f11de2`.
The published six-file PGO tool generates fresh profiles from twelve deterministic
generated inputs. All six original project files remain held out. Training and
optimized validation each check 288 parses. The full frozen snapshot contains 358
source and tool files.

The normal comparison uses the same compiler, explicit host target and base build
environment as PGO, removing only the profile-use flags. It is a distinct build
from the default-release correctness library. Expat 2.8.4's normal and trained
libraries are reused byte-for-byte from the independently reviewed original study.
Oriole uses Rust/LLVM 22; Expat uses GCC 13.3 with `-O3` and no LTO.
The Oriole release profile requested ThinLTO, but the archived nonverbose build
logs do not establish that it was effective for the combined
`cdylib`/`staticlib`/`rlib` build. The earlier unconditional ThinLTO wording was
unsupported. This clarification leaves the recorded binaries and measurements
unchanged; see the [current C-library build guide](../../../../crates/oriole_expat/README.md#build-the-c-libraries)
for the explicit Cargo target override.
Aggregate ratios are equal-weight geometric means of the per-condition median
paired ratios.

| Library | Shared-library SHA-256 |
| --- | --- |
| Oriole, matched normal | `5974e2718f50690a27e48f91e1e1648aa5c4ef81ac17187a582957c8653208f3` |
| Oriole, PGO | `4584afbf7361732cf506f762cf25885cded19ecc36ee6b03053ae674bf809505` |
| Expat, normal | `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478` |
| Expat, PGO | `12d33ad26315e8b46a02e598df8581679d0561fb2d98a3d4744e483a573fbdd0` |

## Native callbacks

The native study covers six projects, namespaces off/on, and 4 KiB/64 KiB chunks.
All 96 output preflights finish before seven randomized cohorts. The 672 timed
processes contain 136,976 measured parses and 672 warmups. Per-process measured
counts are fixed in advance: Vulkan 7, Wayland 64, Maven 128, Batik 512, GTK 256
and DocBook 256.

Constructor calls, handler setup, parsing, callback hashing and destruction are
timed. Process startup, dynamic loading and file I/O are excluded. Native and
Python workloads do not load external DTDs, including the Batik SVG's declaration.

Times below are medians of process medians for 4 KiB chunks without namespaces.
Ratios are medians of paired ratios, so they need not equal the quotient of the
displayed rounded times. A ratio above one means Oriole takes longer.

| Project | Oriole | Expat | Normal ratio | Oriole PGO | Expat PGO | PGO ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Vulkan | 89.795 ms | 30.275 ms | 2.97× | 69.384 ms | 25.029 ms | 2.78× |
| Wayland | 2.011 ms | 1.094 ms | 1.84× | 1.664 ms | 0.965 ms | 1.73× |
| Maven | 1.599 ms | 0.462 ms | 3.46× | 1.236 ms | 0.377 ms | 3.20× |
| Batik | 0.155 ms | 0.135 ms | 1.14× | 0.122 ms | 0.126 ms | 0.97× |
| GTK | 0.684 ms | 0.217 ms | 3.12× | 0.536 ms | 0.183 ms | 2.92× |
| DocBook | 0.459 ms | 0.189 ms | 2.43× | 0.340 ms | 0.169 ms | 2.03× |

Across all 24 conditions, Oriole/Expat is 2.4996× for normal builds and 2.2293× for
PGO builds. Each parser improves in all 24 conditions after training. Oriole wins
one of the 24 comparisons with trained Expat: Batik without namespaces at 4 KiB.
[Every condition](native-reconstruction.json) and raw sample remains available.

## Actual CPython consumers

The benchmark builds unchanged CPython 3.12.13 pyexpat and ElementTree extensions
against all four libraries. It checks complete canonical results and loaded module
origins in 96 preflights before timing. Seven randomized cohorts use 672 workers,
34,384 measured parses and 672 warmups across both interfaces and chunk widths.
Fixed measured counts per worker are Vulkan 3, Wayland 16, Maven 32, Batik 128,
GTK 64 and DocBook 64.

| Consumer | Normal Oriole / Expat | PGO Oriole / Expat | Oriole PGO / normal |
| --- | ---: | ---: | ---: |
| ElementTree | 1.79× | 1.55× | 0.816× |
| pyexpat event list | 1.46× | 1.30× | 0.851× |
| Both interfaces | 1.62× | 1.42× | 0.833× |

The ratios equally weight each project/interface/chunk condition. Parser creation,
feeding, callbacks and result destruction are timed. Imports, input preparation,
canonicalization and explicit `gc.collect()` calls are outside timing. Automatic
garbage collection remains enabled during parsing. The original worker and its
destruction policy are unchanged. Oriole wins two of 24 PGO comparisons. See the
[project table](consumer-table.md) and [all consumer records](consumer-reconstruction.json).

## Actual Wayland code generation

The pinned, unchanged Wayland scanner generates client headers and private code
from the original protocol XML. Optional libxml DTD validation is disabled for
every build. Eight preflights check byte-identical generated output; four additional
untimed loader probes verify the actual `XML_ParseBuffer` binding and library hash.
Seven cohorts measure 20 processes per mode and engine, totaling 1,120 processes.

| Command | Oriole | Expat | Oriole PGO | Expat PGO | PGO ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Client header | 7.889 ms | 6.416 ms | 7.547 ms | 6.256 ms | 1.21× |
| Private code | 6.374 ms | 4.832 ms | 5.939 ms | 4.687 ms | 1.30× |

The combined normal comparison is 1.2874× and PGO comparison is 1.2546×. PGO reduces
Oriole's command time by 5.6%. Startup, loading, input/output and process exit are
included. This measures the scanner commands, not a complete Wayland project build.
The eight final output files are retained; preceding per-invocation files were
replaced, so their checks are preserved as hashes and lengths rather than separate
generated files.

## Review and limitations

Timing uses CPU 0 on a shared Linux host. Concurrent sanitizer work on other CPUs,
memory bandwidth, caches and frequency remain uncontrolled. Separate native and
consumer load records preserve the observations and timing uncertainty.

The first Python and Wayland timing attempts accidentally overlapped on CPU 0.
All 363 partial Python worker records and 1,120 Wayland observations from those
attempts are retained and excluded entirely. The unchanged protocol was rerun
through one controller that waits for Python to finish before launching Wayland.
No favorable samples were selected from the invalid attempts. Earlier build and
loader-probe mistakes, and superseded pre-version preflights, remain separate.

Independent reviews reconstruct every native and consumer sample, verify complete
output checks, hashes, randomized order and paired arithmetic. The native owner
reconstruction retains an earlier “review pending” label; the included completed
[native review](native-independent-review.json) and
[consumer review](consumer-independent-review.json) supersede that collection-stage
label. The original handoff's shorthand about garbage collection refers only to
explicit collection calls; automatic GC remains enabled.

The [full compatibility report](../../../../docs/validation/2026-09-10/version-consistent-runtime/)
tests default-release normal `ac6a` and optimized `4584`. Its strict API and CPython
results are not assigned to matched benchmark normal `5974`, which receives the
output and loader gates described here. Bounded benchmarks do not establish a
general production replacement.

`files.json` records every member of `evidence.tar.gz`, including the unchanged
handoff manifest, source, profile/training/build records, raw successes and failures,
and independent reviews. The [rejected scan-fusion study](../rejected-fused-tag-study/)
and its structural design assessment are preserved separately. No rejected runtime
change is included in these measurements.
