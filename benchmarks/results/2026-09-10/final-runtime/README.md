# Final runtime benchmarks

These measurements use the runtime at PR59 (`1262888`), with shared library SHA256 `ff5d86621114e464a0b20f108b6308fe155d9e8c87bd764bc00872cf124750f8`. Fresh safe Rust API executables use the same parser source. The manifests record every source, compiler, command, input and executable hash. Timings ran sequentially on CPU 0 after the final sanitizer campaigns and stress replays ended.

## Native C interface

Seven randomized pairs of processes each measure ten parses after a discarded warmup. Parser creation, handler registration, parsing, callback hashing and destruction are included; loading files/libraries and starting processes are excluded. Complete normalized callbacks are compared before timing. Each namespace mode passes all 12 preflights and 168 timed processes; raw results also include 64-byte and 1 MiB chunks.

The table uses 4 KiB chunks. Ratios are medians of paired Oriole/Expat times, rather than quotients of the rounded columns.

| Namespaces | Workload | Oriole ms | Expat ms | Oriole / Expat |
| --- | --- | ---: | ---: | ---: |
| Off | elements | 19.442 | 3.658 | 5.30× |
| Off | text | 10.150 | 1.609 | 6.31× |
| Off | entities | 24.031 | 2.133 | 11.32× |
| Off | namespaces | 22.249 | 3.222 | 6.85× |
| On | elements | 25.315 | 3.826 | 6.67× |
| On | text | 10.470 | 1.597 | 6.56× |
| On | entities | 21.869 | 2.136 | 10.21× |
| On | namespaces | 26.637 | 4.336 | 6.16× |

Oriole remains about 5–11 times slower on these generated C workloads. These measurements do not establish CPython application performance. Earlier optimizations and negative experiments retain their separate before/after source snapshots.

## Safe Rust API allocators

The safe API benchmark measures system, jemalloc and mimalloc executables separately from the C ABI. Namespace processing is disabled, including for the prefixed-name workload. Each of 252 timed processes measures ten parses after one warmup. Output digests, element counts and character byte counts match across configurations and chunks.

The table shows 4 KiB chunks; speedup columns are medians of paired system/candidate times. Values below 1 indicate a slowdown.

| Workload | System ms | jemalloc ms | mimalloc ms | jemalloc speedup | mimalloc speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| elements | 15.944 | 14.912 | 14.928 | 1.057× | 1.057× |
| text | 9.091 | 8.966 | 9.144 | 1.009× | 0.988× |
| entities | 20.356 | 19.815 | 19.243 | 1.036× | 1.057× |
| namespaces | 18.104 | 17.635 | 17.824 | 1.029× | 1.011× |

Allocator effects are modest and vary across workloads and samples. The existing CLI allocator choices remain explicit; embedding applications choose their own allocator.

Allocation counting uses a fourth instrumented executable in 12 separate processes. The following numbers are allocation/reallocation requests and total requested bytes per parse, including parser lifetime. Requested bytes are not peak live memory; instrumented elapsed times are excluded from the timing comparison.

| Workload, 4 KiB chunks | Requests | Requested bytes |
| --- | ---: | ---: |
| elements | 110,021 | 8,496,226 |
| entities | 19 | 135,123 |
| namespaces | 100,025 | 8,427,010 |
| text | 10,010 | 573,803 |

## DTD scaling

The native DTD driver includes caller-resolved child parsing and full declaration metadata, including content models and their freeing. All 90 exact metadata preflight observations and 630 timed processes pass. Each process measures five parses after one warmup; every timed digest and declaration/request count must match its preflight.

This table uses one-byte feeds and workload sizes 64, 256 and 1,024. Literal sizes are 4, 16 and 64 KiB; other inputs scale repetitions. Raw results also cover 64-byte and 4 KiB feeds.

| Workload | Size 64 ms | Size 256 ms | Size 1,024 ms | 16× workload time ratio |
| --- | ---: | ---: | ---: | ---: |
| closing-pe | 0.370 | 1.402 | 5.551 | 14.99× |
| header-pe | 0.378 | 1.526 | 5.953 | 15.74× |
| empty-internal | 0.077 | 0.224 | 0.827 | 10.75× |
| empty-external | 0.084 | 0.266 | 0.995 | 11.87× |
| literal | 0.574 | 2.155 | 8.620 | 15.01× |

The measured curves support bounded incremental scanning for these inputs; they do not prove worst-case complexity. The external-reference workload exercises the final declaration grammar implementation.

The unchanged reviewed C driver additionally passes 60 ASan/UBSan processes against both libraries and all 15 fixtures at chunks 1/4096. Both samples in each of 30 paired runs have identical complete trace metadata. The linked Rust release library is uninstrumented in this driver check; separate sustained Rust ASan campaigns are recorded in the [final validation report](../../../../docs/validation/2026-09-10/final-runtime/). An initial postprocessing error compared JSON containing engine version and timing; its failed report is retained, and corrected comparisons use the original successful process outputs.

## Reproduction and limits

`measurements.tar.gz` contains raw reports, preflights, inputs, all build commands, compiler/environment details, source and binary hashes, counting observations and sanitizer outputs. Buildable source and separate compatibility results are in the [final validation package](../../../../docs/validation/2026-09-10/final-runtime/). The shared host uses an AMD EPYC-Milan processor; CPU frequency, host load and memory bandwidth are uncontrolled. All source/input/binary hashes remain unchanged across each measurement campaign. No hardware-counter profile was available on this host.
