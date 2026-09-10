# Frozen-runtime real-project baseline

Runtime: `1262888`; Oriole shared-library SHA256 `ff5d86621114e464a0b20f108b6308fe155d9e8c87bd764bc00872cf124750f8`. Reference: pinned Expat 2.8.4, SHA256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

All six projects are slower with this baseline. These measurements identify optimization targets; no input was removed after seeing its result.

## Native C parser, 4 KiB feeds

Times are medians of process medians. The ratio is the median of paired Expat/Oriole times, so below one means Oriole is slower. Namespace processing is disabled in this table.

| Project | Oriole ms | Expat ms | Expat / Oriole |
| --- | ---: | ---: | ---: |
| vulkan | 156.878 | 29.101 | 0.186× |
| wayland | 5.294 | 1.083 | 0.204× |
| maven | 2.652 | 0.476 | 0.175× |
| batik | 0.484 | 0.138 | 0.286× |
| gtk | 1.187 | 0.219 | 0.185× |
| docbook | 0.850 | 0.197 | 0.232× |

## Matched CPython consumers, 4 KiB feeds

These unmodified CPython extensions use the same Python interpreter, compiler flags and input bytes. All output, resolved parser-library and source checks pass.

| Project | ElementTree Oriole ms | Expat ms | Expat / Oriole | pyexpat Oriole ms | Expat ms | Expat / Oriole |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| vulkan | 233.008 | 83.268 | 0.360× | 329.252 | 155.228 | 0.471× |
| wayland | 6.552 | 1.785 | 0.270× | 8.170 | 3.158 | 0.387× |
| maven | 3.252 | 0.938 | 0.288× | 4.173 | 1.687 | 0.403× |
| batik | 0.620 | 0.216 | 0.356× | 0.665 | 0.258 | 0.387× |
| gtk | 1.545 | 0.441 | 0.291× | 1.889 | 0.724 | 0.385× |
| docbook | 1.185 | 0.398 | 0.336× | 1.424 | 0.586 | 0.411× |

## Equal-project geometric means

Every project has equal weight within each group. These aggregates do not describe whole-project build performance.

| Consumer | 4 KiB Expat / Oriole | 64 KiB Expat / Oriole |
| --- | ---: | ---: |
| Native namespaces off | 0.208× | 0.192× |
| Native namespaces on | 0.196× | 0.185× |
| ElementTree | 0.315× | 0.313× |
| pyexpat events | 0.406× | 0.397× |

## Actual Wayland scanner commands

Full-process wall times include startup, input I/O, parsing, code generation and output writes. Optional libxml DTD validation is disabled identically. Each command generates byte-identical output with both parsers.

| Command | Oriole ms | Expat ms | Expat / Oriole |
| --- | ---: | ---: | ---: |
| client-header | 10.961 | 6.267 | 0.571× |
| private-code | 9.480 | 4.716 | 0.498× |

## Evidence

- Native: 24 full normalized preflights, 336 timed processes and 6,720 measured parses.
- Python: 48 consumer preflights, 336 timed processes and 3,360 measured parses.
- Wayland: four exact output preflights and 280 full code-generation processes.
- All source, corpus and binary hashes stay unchanged across each campaign; every recorded sample matches its paired output.
- Stable incorrect-output and worker-exit injections are rejected and preserved in separate negative reports.
- Raw per-pair ranges and all 64 KiB results are retained in the JSON reports.
- Linux shared host; CPU 0 reserved among agents for timing, with other-CPU setup/build/profiling allowed. CPU frequency, unrelated host load and shared memory bandwidth are uncontrolled.

The [evidence archive](evidence.tar.gz) contains the full reports, source and corpus provenance, licenses, build commands and failure-injection checks. Its SHA256 is `bdfdb9d5258fe7633b0ed841c34c63aef1097a25ca2638f2188a7441310d50e6`; [files.json](files.json) records member hashes. ELF binaries are excluded; their recorded hashes identify the measured builds.
