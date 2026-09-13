# Explicit C allocator measurements

**Keep the current C allocator default.** In this native XML study, jemalloc and mimalloc change Oriole's real-project elapsed time by less than 0.2%, while raising workload peak resident memory. The generated controls improve, but the six real-project workloads do not show a useful speed gain.

| Allocator mode | Oriole time | Oriole peak RSS | Expat time | Expat peak RSS |
| --- | ---: | ---: | ---: | ---: |
| Ordinary constructor | 1.000000× | 1.000000× | 1.000000× | 1.000000× |
| Explicit libc | 1.002669× | 1.001004× | 1.002685× | 1.001449× |
| Explicit jemalloc | 0.998749× | 1.030060× | 0.984615× | 1.012483× |
| Explicit mimalloc | 0.999783× | 1.733960× | 0.985886× | 1.629564× |

Each ratio compares the selected mode with that parser's ordinary constructor. Lower is better. The 24 real conditions cover six project XML files, both namespace modes and 4 KiB/64 KiB chunks. Time ratios use seven paired rounds; RSS ratios use a separate three-round campaign. Aggregates are geometric means of per-condition medians of paired ratios. RSS is the absolute process high-water value from `/proc/self/status`, including both providers' startup state, the input buffer, library mappings and workload pages. It is not isolated allocator memory or a single aggregate peak value.

Both parsers use their measured PGO libraries from the [PGO/LTO study](../pgo-lto/). The four modes are the ordinary parser constructor and `XML_ParserCreate_MM` with libc, jemalloc or mimalloc. The explicit libc control measures the effect of selecting a memory suite. The process-wide allocator stays libc. The study does not deliberately tune providers; the validation report records the configuration evidence and its limits.

## Memory examples

Oriole's median process peak RSS in KiB, with 4 KiB chunks and namespaces disabled:

| Project XML | Ordinary | Explicit libc | jemalloc | mimalloc |
| --- | ---: | ---: | ---: | ---: |
| Vulkan | 6992 | 7052 | 7020 | 10740 |
| Wayland | 3868 | 3868 | 3988 | 7700 |
| Maven | 3632 | 3644 | 3728 | 7584 |
| Batik | 3616 | 3616 | 3768 | 7612 |
| GTK | 3496 | 3488 | 3680 | 7560 |
| DocBook | 3572 | 3516 | 3672 | 7488 |

Both alternate providers increase Oriole's peak RSS in all 24 real conditions. Across those conditions, jemalloc's ratio is 1.030× and mimalloc's is 1.734×. These measurements use one executable containing both provider archives, with their mandatory startup constructors active in every mode.

## Adverse and generated results

Jemalloc is slower than ordinary allocation in 11 of 24 Oriole real conditions; mimalloc is slower in seven. The shared-host experiment does not establish significance for their near-zero aggregate time differences. With the same allocator in both parsers, Oriole takes 1.429× Expat's time with ordinary allocation, 1.432× with explicit libc, 1.446× with jemalloc and 1.450× with mimalloc; each mode has only three winning real conditions.

The four generated conditions are separate. Oriole time falls by 11.5% with jemalloc and 12.3% with mimalloc, with zero and one regressions respectively. Workload peak RSS rises in all four conditions for both alternate providers. These gains do not support replacing the C allocator for the measured real-project workload set.

The [validation report](../../../../docs/validation/2026-09-11/c-allocators/) retains provider builds, allocation-failure/lifecycle proofs, driver source, all raw time and RSS workers, and review evidence. This study measures native C consumers; it does not measure the CLI, CPython, other platforms or process-wide allocator replacement.
