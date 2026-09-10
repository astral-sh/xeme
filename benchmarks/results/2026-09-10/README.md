# Initial measurements — September 10, 2026

These measurements establish a baseline before the DTD index and buffer reuse
optimizations. Oriole is substantially slower than Expat on these generated
workloads. They are not CPython application benchmarks.

## C interface

Median milliseconds for a complete parse, with 4 KiB input chunks:

| Workload | Input bytes | Oriole | Expat 2.8.4 |
| --- | ---: | ---: | ---: |
| 10,000 empty elements with attributes | 310,013 | 23.06 | 3.53 |
| Text with line breaks | 440,013 | 11.44 | 1.61 |
| Predefined and numeric references | 340,013 | 31.50 | 2.15 |
| Prefixed names, namespace processing disabled | 330,034 | 26.24 | 3.24 |

Across the tested 64-byte, 4 KiB, and 1 MiB chunk sizes, Oriole took approximately
6–17 times as long as Expat. The [complete native results](native-baseline.json.gz)
retain every sample and all paired ratios, including the discarded warmups.

## Safe Rust API and allocators

Median milliseconds with the same 4 KiB chunks:

| Workload | System | jemalloc | mimalloc |
| --- | ---: | ---: | ---: |
| Elements and attributes | 16.71 | 15.85 | 15.79 |
| Text with line breaks | 9.84 | 9.65 | 9.98 |
| References | 25.08 | 25.53 | 25.19 |
| Prefixed names, namespace processing disabled | 19.06 | 18.05 | 18.44 |

No allocator won on every workload. These small differences do not establish a
universal preference. The CLI follows uv's platform allocator policy; library
users choose their allocator. The safe API and C API include different ownership
and accounting work, so their absolute timings are separate comparisons.

Separate instrumented runs counted 140,021 allocation/reallocation requests for
elements, 40,234 for text, 190,096 for references, and 150,047 for prefixed names.
These counts motivated buffer and temporary-string work. Requested bytes are the
sum of requests, including reallocations; they are not peak live memory. The
[allocator results](allocators-baseline.json.gz) retain those observations
separately from the uninstrumented timing samples.

## Method and provenance

Each comparison used seven randomized pairs of processes. Each process discarded
one warmup and timed ten complete parses, including construction, callback/event
processing, and destruction. Loading files and libraries, process startup, and
JSON output were excluded. Results are medians of process medians. The runners
check callback output before or during comparisons; native preflight compares
complete normalized streams separately from its timed FNV digest.

The host reported an AMD EPYC-Milan processor. Processes were pinned to one
available logical CPU, but competing host activity, frequency, and memory
bandwidth were uncontrolled. Filesystem caches were warm. Rust builds used Ohm
1.98.1-dev (`f62703110`), thin LTO, and one codegen unit. The reference was Expat
2.8.4, built in CMake Release mode, at
`12cf0b1f25f026a022fe728ad8f7e3d017285b80`.

The [source archive](baseline-source.tar.gz) preserves the exact baseline Rust
sources and benchmark executable source. The [source manifest](baseline-source.json)
and [build manifest](baseline-complete-build.json) identify source, compiler,
commands, and executable hashes. The latter is a supplemental manifest assembled
from the retained artifacts after the initial timing run. The original result
files remain unchanged; newer runner versions also hash their Python helpers,
copy build manifests, and isolate preflight in a process with a timeout.

These are local development results. They do not establish performance on other
architectures, production toolchains, real application corpora, or a PBS build.

## DTD declaration index

With 16 elements specifying every declared attribute, 4 KiB chunks, and the system
allocator:

| Declared attributes | Before, ms | Indexed, ms | Paired speedup |
| ---: | ---: | ---: | ---: |
| 128 | 3.25 | 1.59 | 2.02× |
| 256 | 10.11 | 3.15 | 3.22× |
| 512 | 33.61 | 6.34 | 5.28× |
| 1,024 | 120.88 | 12.98 | 9.32× |

The index removes repeated scans for type and ID lookup, as well as duplicate
declaration checks. Declaration order and first-declaration-wins behavior remain
unchanged. The largest input is 237,770 bytes; the corpus stays below the attribute,
token, and expansion limits. No resource ceiling was raised for measurement.

Seven paired processes each timed five parses after a warmup. The
[DTD results](bench-index-dtd.json.gz) include declaration-only inputs and both
4 KiB and 1 MiB chunks. The 128-attribute declaration-only case was about 6% slower;
the extra index has a setup cost. The [ordinary workloads](bench-index-common.json.gz)
stayed within about 2% of their parent. The [build manifest](index-build.json)
and [source hashes](index-source.json) identify the measured candidate.

## Reusing raw-token storage

Reusing `current_raw` storage reduces temporary allocation while leaving returned
event payloads independently owned. At 4 KiB chunks with the system allocator:

| Workload | Indexed parent, ms | Reused buffer, ms | Allocation requests before → after |
| --- | ---: | ---: | ---: |
| Elements and attributes | 16.36 | 15.90 | 140,021 → 130,021 |
| Text with line breaks | 9.89 | 9.08 | 40,234 → 20,127 |
| References | 25.19 | 22.25 | 190,096 → 110,057 |
| Prefixed names, namespace processing disabled | 18.58 | 17.52 | 150,047 → 120,036 |

These [paired results](bench-reuse-common.json.gz) used seven pairs and ten timed
parses per process. The [build manifest](reuse-build.json) and
[source hashes](reuse-source.json) identify the candidate. Allocation counts come
from separate instrumented runs. The retained buffer keeps its largest capacity
until reset or destruction; existing token/input and C-family allocation limits
still apply. This is an allocation/time improvement with a retention tradeoff.
