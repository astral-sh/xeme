# Combined runtime measurements

Runtime: PR #44 (`b68bdca`). The [frozen source](../../../../docs/validation/2026-09-10/external-values/) and build manifests identify every measured library and executable. The C library SHA-256 is `f7e602e300daebedfc08854ed32adebeb656165cddc019dcc0b0226063f9f569`.

## Native C interface

Milliseconds per complete parse with 4 KiB chunks. Seven randomized paired processes, ten measured parses after one discarded warmup. Lower is better.

| Namespace mode | Workload | Oriole (ms) | Expat 2.8.4 (ms) | Paired Oriole/Expat time |
|---|---|---:|---:|---:|
| Off | Elements | 20.057 | 3.581 | 5.67× |
| Off | Text | 10.302 | 1.605 | 6.40× |
| Off | References | 23.592 | 2.133 | 11.06× |
| Off | Prefixed names | 22.153 | 3.224 | 6.84× |
| On | Elements | 26.142 | 3.595 | 7.20× |
| On | Text | 10.388 | 1.605 | 6.47× |
| On | References | 22.438 | 2.144 | 10.43× |
| On | Prefixed names | 26.184 | 4.395 | 5.96× |

Complete normalized callback preflights pass for all four workloads and all three chunk sizes in both modes. Native callback digests agree throughout timing. The archives also retain 64-byte and 1 MiB chunks and every paired observation. Oriole remains substantially slower than Expat.

## Safe Rust API and allocators

Namespace processing is disabled for this separate safe-core comparison. Instrumented allocation counts are measured in a separate executable and do not enter timing samples.

| Workload | System (ms) | jemalloc (ms) | mimalloc (ms) | Allocation/reallocation calls |
|---|---:|---:|---:|---:|
| Elements | 16.213 | 15.627 | 15.696 | 110,021 |
| Text | 9.238 | 9.383 | 9.520 | 10,010 |
| References | 20.269 | 20.964 | 20.553 | 19 |
| Prefixed names | 17.872 | 18.010 | 17.783 | 100,025 |

Allocator results vary by workload. At 4 KiB, paired element throughput improves about 4–5%, while the reference-heavy workload regresses about 2–4% with the alternate allocators. These results support preserving allocator choice for embedding applications.

## Method and limits

AMD EPYC-Milan, Linux x86_64, CPU 3; Rust/Ohm 1.98.1-dev, thin LTO. Parser creation, callback registration, parsing, callback/event hashing, and destruction are timed. File loading and process startup are excluded. The host is shared; sanitizer campaigns ran on other CPUs, and frequency, cache, bandwidth, and competing load were uncontrolled. These generated workloads do not measure real CPython application performance.

These are a combined checkpoint, not an isolated optimization comparison. The [inline text experiment](../inline-text/) retains the accepted reference-heavy improvement and the rejected global inline-string design. The [callback allocation comparison](../callback-allocation/) retains its paired parent/candidate result. Earlier measurements remain separate.

Each archive contains full raw reports and source/input/binary hashes. `build.tar.gz` contains exact build commands, compiler identity, logs, and benchmark sources; the parser source is linked above. No compatibility failure was waived to produce timings.
