# DTD runtime checkpoint

These measurements use PR56 (`0ea8c7f`) with release library SHA256 `35bac262f1b77c0066b17ce82529bd52d71e1b850de4faa477e340bcbbd4c710`. The source archive and manifests include the namespace, version, foreign-DTD, decoder, API-state and internal DTD composition fixes. The later external grammar implementation is not included.

## Native C interface

Seven randomized pairs of processes each measure ten parses after a discarded warmup, using the established driver and complete normalized-callback preflight. The table shows 4 KiB input chunks; raw reports also cover 64-byte and 1 MiB chunks. Ratios are medians of paired Oriole/Expat ratios, not quotients of the displayed rounded medians.

| Namespaces | Workload | Oriole ms | Expat ms | Oriole / Expat |
| --- | --- | ---: | ---: | ---: |
| Off | elements | 19.901 | 3.678 | 5.39× |
| Off | text | 10.053 | 1.612 | 6.24× |
| Off | entities | 22.797 | 2.142 | 10.66× |
| Off | namespaces | 21.773 | 3.199 | 6.69× |
| On | elements | 25.225 | 3.725 | 6.75× |
| On | text | 10.083 | 1.594 | 6.28× |
| On | entities | 22.943 | 2.131 | 10.67× |
| On | namespaces | 26.014 | 4.350 | 5.95× |

Both modes pass their 12 complete callback preflights. Source, input, and binary hashes remain unchanged. The C interface remains slower than Expat. The earlier system/jemalloc/mimalloc comparison belongs to the separately frozen `b68bdca` checkpoint and is not relabeled as a measurement of this runtime.

## DTD composition scaling

The new native DTD driver includes declaration callbacks, complete content model traversal/freeing, an in-memory external resolver, parser/child creation, and destruction. Before timing, it serializes every selected callback field and callback depth, checks expected fixture counts, and compares both engines exactly. Every timed sample must retain the matching preflight digest and counts.

All 72 preflight observations and 504 timed processes pass. Each process measures five parses after one warmup; paired process order is randomized. The following one-byte-feed medians use workload sizes 64, 256 and 1,024. Literal sizes are 4, 16 and 64 KiB; other rows scale repetitions. Raw reports also contain 64-byte and 4 KiB feeds.

| Workload | Size 64 ms | Size 256 ms | Size 1,024 ms | 16× workload time ratio |
| --- | ---: | ---: | ---: | ---: |
| closing-pe | 0.364 | 1.445 | 5.661 | 15.55× |
| header-pe | 0.419 | 1.617 | 6.562 | 15.67× |
| empty-internal | 0.074 | 0.218 | 0.811 | 10.92× |
| literal | 0.557 | 2.042 | 7.965 | 14.30× |

These curves support bounded incremental scanning for the measured inputs. They do not prove worst-case complexity for arbitrary XML. The optional external-grammar workload is generated separately and is not part of this checkpoint.

Independent review covers child lifetimes, model freeing, timing boundaries, exact metadata checks and report integrity. The C driver passes 48 ASan/UBSan runs; an independent nested-model/resolver fixture also passes. Linked Rust libraries are uninstrumented in those C driver checks. Worker exit, malformed JSON, invalid schema, timeout and launch failures retain diagnostic reports. A stable but different timed digest is rejected after its successful preflight. Earlier harness iterations, the missing-header manifest setup error, and the failed hardware-profiler attempt are retained separately from the reviewed results.

## Consumers and compatibility

Four actual CPython 3.12.13 consumers—shared/static linkage, with and without the explicit upstream cleanup backport—report SUCCESS for all six XML modules, 803 reported tests and 31 skips per run. Upstream expected failures remain in the logs. All six native shared/static callback and allocation probes pass, including 353 allocation-failure scenarios per linkage. The C callbacks have ASan/UBSan; the Rust release library does not.

The generated and named differential corpus passes 12,318 semantic comparisons with no acceptance, error-code, or normalized-callback differences. Six exact callback-fragmentation differences and 450 final-position differences remain. The full Expat matrix and W3C acceptance results for this same runtime are recorded in the DTD composition validation report. The successful full PBS distribution and long Rust ASan campaigns still identify their earlier `b68bdca` source snapshot; these local consumer checks do not substitute for a new distribution or campaign.

All timings ran on CPU 0 of a shared host. Host load, CPU frequency, and memory bandwidth are uncontrolled. File/library loading and process startup are excluded. These generated workloads do not establish CPython application performance or production readiness.
