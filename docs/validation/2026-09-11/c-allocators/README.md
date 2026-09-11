# Explicit C allocator validation

**Decision:** retain the existing default. In the measured PGO C libraries, explicit jemalloc and mimalloc provide negligible aggregate speed improvement on real-project XML and increase resident memory. The [benchmark report](../../../../benchmarks/results/2026-09-11/c-allocators/) includes all real and generated conditions. This layer records an experiment; parser source, default allocators and resource limits are unchanged.

## Providers and contracts

The study uses the same unmodified `be22a27` runtime and pinned Expat 2.8.4 PGO library as the [PGO/LTO report](../pgo-lto/). Oriole's shared library hashes to `4f1518fc819b…`; Expat's hashes to `12d33ad26315…`. Their distinct compiler and training provenance remains scoped in that report.

The static PIC jemalloc build uses pinned `5.3.0-1-ge13ca993` source, the `oriole_je_` public prefix and a private namespace, with C++ operators disabled. The mimalloc build uses pinned `libmimalloc-sys 0.1.49` vendored mimalloc 2.3.2 and its packaged release build recipe, with allocator override disabled. The study does not deliberately tune providers; the timing controller removes the listed inherited allocator-tuning and loader-preload variables.

The source receipt captures `/etc/malloc.conf`, while the namespaced jemalloc build consults `/etc/oriole_je_malloc.conf`. Both are absent at the later review, but the prefixed file's absence was not recorded during the experiment. Build flags, observed proof-process options and environment cleanup are retained; they do not establish the historical absence of every configuration source in each benchmark process.

An instrumented provider proof checks symbol origins, ordinary and namespace traces, allocation/reallocation/free behavior, failed growth preserving the old allocation, injected failures, reset, content-model lifetime, child parsers, callback-created children, suspend/resume, malformed input and amplification limits. The same frozen proof is replayed against the PGO pair in eight fresh processes. All 144 parser-entry origin checks pass. Every tested custom allocation returns to a zero-live-block ledger after cleanup. The provider proof's ledger and metadata queries are absent from the timing driver.

Review corrected the initial proof's parent/child destruction order before executing it: Expat requires children to be freed first. Provider build failures, a strict-C compilation warning, a broad-export attempt and the corrected narrow-export build remain in the archive. The timing executable exports only the six provider allocation functions; it does not define global malloc-family replacements.

## Measurement protocol

A sibling native driver preserves the original callbacks, output hash and timed loop, changing only constructor selection and adding an optional post-workload RSS read. All modes use the same executable. Each process times parser creation, feeds, callbacks, finalization and destruction; input loading and output validation remain outside the measured interval. Every process runs one warmup followed by the original per-fixture iteration count.

The elapsed campaign completes 224 preflights and 1,568 timed workers: 196 seeded cohorts, 281,120 measured parses and 1,568 excluded warmups. The separate RSS campaign completes 672 workers in 84 cohorts: 120,480 workload parses plus 672 warmups. RSS runs begin only after elapsed timing ends; their raw time fields are retained but never included in elapsed summaries. Worker timeouts are 90 seconds; preflights have a 600-second aggregate bound and each main campaign a 1,200-second aggregate bound, with process-group cleanup.

The elapsed controller reserves CPU0 among cooperating agents, while current PGO compatibility checks run on other CPUs. RSS runs on CPU2. This is a shared host with uncontrolled frequency, cache, bandwidth and kernel resident-page accounting. Every condition and sample remains recorded. Near-zero timing differences do not establish statistical significance.

RSS comes from the current executable's `/proc/self/status` `VmHWM` after all parse/free iterations, while its input and parser library remain loaded. Both static providers have mandatory startup constructors in every mode. Values include those common initializations, resident executable/library pages, libc input/output storage, and selected-provider workload pages. They do not isolate the allocator's memory or represent requested live bytes. No proof-process RSS or inherited pre-exec `getrusage` value is substituted.

## Validation and scope

All eight modes produce matching callback hashes, element counts and text-byte counts in every preflight and timed/RSS worker. The study author recomputes each saved median, seeded order, paired ratio and aggregate. Independent [provider/driver](reviews/source.json), [saved-data](reviews/results.json) and [package](reviews/package.json) reviews pass, with their complete evidence in [the review archive](reviews/index.json).

The ordinary measured PGO artifacts have the full C/API and strict CPython checks in the preceding report. Those checks are not transferred to all custom allocation modes: this experiment covers the explicit provider proof and the fixed native corpus, and retains that narrower claim. CLI allocator choices, process-wide replacement, CPython custom allocator performance and other platforms are outside this study.

## Evidence

`evidence.tar.gz` retains 8,502 indexed source and evidence files, including every original provider attempt, the PGO proof replay, drivers/controllers, raw elapsed/RSS output, saved summaries and author audits. Its SHA-256 is `0428afb9b64b99c077b84387e84232caf0f0c4af6bfafbb1accad3eff162278e`. The archive excludes 202 compiled binaries, objects and bytecode files with their exact hashes retained. Its original flat/nested manifest packaging failure is preserved; no benchmark workers were rerun for packaging.

The compact `receipt.json` and `summary.json` accompany the archive. The independent review archive retains the full source, arithmetic and package audits, including their recorded limitations. The publication file index excludes itself and records every accompanying file.
