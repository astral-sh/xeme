# Actual CPython consumers over pinned project XML

These three separate screens build unmodified CPython 3.12.13 `pyexpat.c` and `_elementtree.c` with identical settings against frozen Oriole and Expat libraries. Module/library origins, complete preflight trees/events and every measured output are verified. All six original project files are pinned by hash. Each screen uses five randomized pairs, five parses per process and 4 KiB chunks on CPU 0.

| Isolated checkpoint | ElementTree time / Expat | pyexpat time / Expat | Wayland pyexpat time / Expat |
| --- | ---: | ---: | ---: |
| Name-rule compatibility | 2.069× | 1.659× | 1.202× |
| Scanning utilities and name reuse | 1.870× | 1.541× | 1.063× |
| Utilities, names, byte count and tag scanner | 1.825× | 1.519× | 1.048× |

The first two columns use the geometric mean of per-project paired median time ratios. Lower is faster. Every measured condition remains slower than Expat. These are separate cohorts on a shared host with uncontrolled load/frequency, and their compatibility bases differ; they do not establish an exact causal speedup between rows. The measured work is real CPython XML parsing, not full application execution. The final row is the isolated performance candidate, not the later root composition with attribute fixes.

[report.json](report.json) gives every project ratio; [files.json](files.json) verifies all members of [evidence.tar.gz](evidence.tar.gz). Raw samples, build flags, compiler/source/library hashes, module origins, commands and the initial failed `taskset -c0` startup are retained. No readiness claim follows from these benchmarks.
