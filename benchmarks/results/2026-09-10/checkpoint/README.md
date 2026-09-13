# Validated checkpoint measurements

The runtime at PR #22 (`72dbb2a`) includes the callback, resource-budget, lexical,
and macOS allocator fixes. The retained source archive and build manifests identify
the exact library and allocator binaries. Later harness changes do not alter these binaries.

## Native C interface

Milliseconds per complete parse with 4 KiB chunks. Seven randomized paired processes,
ten measured parses per process after one discarded warmup. Lower is better.

| Workload | Input bytes | Oriole | Expat 2.8.4 | Paired Oriole/Expat time |
|---|---:|---:|---:|---:|
| Elements | 310,013 | 21.774 | 3.743 | 5.85× |
| Text | 440,013 | 10.435 | 1.605 | 6.50× |
| References | 340,013 | 26.154 | 2.167 | 11.92× |
| Prefixed names¹ | 330,034 | 24.532 | 3.223 | 7.53× |

¹ Namespace processing is disabled in both libraries for these timings.

Oriole remains about 6–12× slower on the 4 KiB workloads. Parser creation, callback
registration, parsing, native callbacks, and destruction are timed; process startup
and input/library loading are excluded. Complete normalized callback streams pass
the untimed reference preflight. The raw reports also retain 64-byte and 1 MiB chunks.

## Safe Rust API and allocators

The same generated inputs and 4 KiB chunks, using the safe parser directly.
These timings do not include the C adapter. Counts come from a separate instrumented
system-allocator executable and never enter the timing samples.

| Workload | System (ms) | jemalloc (ms) | mimalloc (ms) | Allocation/reallocation calls |
|---|---:|---:|---:|---:|
| Elements | 15.642 | 14.976 | 15.138 | 110,021 |
| Text | 9.387 | 9.004 | 9.131 | 20,127 |
| References | 21.998 | 20.316 | 21.591 | 80,057 |
| Prefixed names | 17.567 | 16.875 | 17.073 | 110,035 |

Allocator effects depend on workload and chunk size; the raw paired ratios include
regressions as well as improvements. The CLI follows uv’s platform allocator choices,
while library consumers retain control. These results do not justify forcing an
allocator on embedding applications.

## Reproduction and limits

AMD EPYC-Milan, Linux x86_64, pinned to CPU 0 on a shared development host.
Rust/Ohm 1.98.1-dev, optimized thin-LTO builds; compiler details and commands are
in the manifests. Frequency, competing host load, and memory bandwidth were uncontrolled.
Warm filesystem caches and generated inputs do not establish CPython application performance.

The [earlier optimization comparisons](../README.md) retain separate parent/candidate
builds, including the 9.3× large-DTD gain, raw-token reuse, and the text regression
observed in the literal fast path. The DTD workload now also has a retained complete
normalized-callback preflight against reference Expat in `dtd-reference-preflight/`.

`native/` and `allocators/` contain compressed complete observations, preflight results,
and input/library/source hashes. `source.tar.gz`, `library-build.json`, and
`allocator-build.json` retain build provenance. No compatibility failure was waived
to produce these performance results.
