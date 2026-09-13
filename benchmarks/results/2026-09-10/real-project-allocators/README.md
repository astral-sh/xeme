# System allocator and jemalloc on real XML

We screened the six pinned project inputs on the frozen `ed7748a` runtime, comparing the system allocator with process-local `LD_PRELOAD` of Debian's jemalloc 5.3.0. Both Oriole and Expat receive the same allocator change; the C driver and parser/input bytes remain identical. This does not override an embedding application's custom Expat memory suite or measure CPython allocator behavior.

The protocol uses 4 KiB feeds, namespace processing off/on, five randomized process groups and five measured parses after a discarded warmup, pinned to CPU 0. Every sample has matching callback hashes, element counts and text bytes. A separate symbol-origin probe and loader binding logs verify malloc-family resolution for both libraries. No preload configuration is installed globally.

Median paired system/jemalloc ratios are generally near neutral to 1.05× for Oriole, with one DocBook namespace condition at 1.07×; Expat is also near neutral to 1.06×. A median-of-all-samples calculation showed a larger DocBook outlier, so all samples and per-process ratios remain available. These short shared-host screens show modest allocator effects and do not close Oriole's gap with Expat. They are not evidence of a full-application speedup or a reason to change the default allocator.

[summary.json](summary.json) contains every process median and paired ratio. [evidence.tar.gz](evidence.tar.gz) contains the exact script, input/parser/allocator hashes, frozen parser build metadata, native driver source, provider probe and raw loader/timing observations. [files.json](files.json) verifies every archive member. Reproduction uses the commands/paths in `run.py` with the supplied pinned corpus and frozen parser sources; the standard native driver build is documented in the benchmark README.

Archive SHA256: `99f1b04254e7b7f8726427ec3fcd3a37f198815f7a14692a2cc54db2f3aa8539`.
