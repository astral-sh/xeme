# Event and buffer throughput

The first performance layer retains owned events and allocator-aware fallible storage. It inlines event transfers, handles ASCII names before Unicode ranges, reuses markup-token scratch, avoids hash-table allocation for at most eight explicit attributes, and copies `Copy` slices in bulk. Larger attribute lists retain randomized hashing. The scalar string-inlining experiment regressed throughput and was discarded.

Token scratch now retains the largest successful token buffer until reset/free; token-size and parser-family allocation limits still apply. No borrowed parser data crosses callbacks. The isolated storage helper checks length overflow and reserves before copying; independent review found no ownership or safety blocker.

## Measurements

The final library is `57b4b6c9575ea274a5f73a8940938124ac09fec4c305c16a927382137c491ae3`; the static archive is `5a11d6a94d76034d18efa8aad4f394489f8c9f7d738478155574137b05e900bc`. Pinned Expat 2.8.4 is unchanged. Seven randomized process pairs measure twenty parses after a warmup. All six original project inputs remain selected; complete normalized callbacks and every timed hash/count match. These native runs time parser creation, callbacks, parsing and freeing, excluding process/library/file loading. Shared-host CPU frequency and other-CPU load remain uncontrolled.

At 4 KiB with namespaces disabled:

| Project | Oriole ms | Expat ms | Expat / Oriole |
| --- | ---: | ---: | ---: |
| batik | 0.349 | 0.138 | 0.401× |
| docbook | 0.642 | 0.199 | 0.309× |
| gtk | 0.893 | 0.219 | 0.246× |
| maven | 1.989 | 0.480 | 0.237× |
| vulkan | 115.371 | 29.403 | 0.258× |
| wayland | 3.881 | 1.086 | 0.280× |

The equal-project geometric mean remains **3.53× slower** with namespaces disabled and **3.78× slower** with namespaces enabled at 4 KiB; at 64 KiB the gaps are 3.83× and 4.07×. This improves the [previous native baseline](../../../../benchmarks/results/2026-09-10/real-project-baseline/README.md), but does not meet the performance goal. Paired screening against the old runtime also shows about 1.3× improvements on Vulkan and Wayland. Full Python-consumer and complete Wayland-command timings will be repeated after the next runtime changes.

## Validation and evidence

All 249 Rust checks pass, including selected-allocation failure sweeps and new duplicate/default-attribute boundary cases. Strict Clippy passes. The storage crate's 28 tests also pass under AddressSanitizer with the external Clang runtime; leak detection is disabled, with explicit selected-allocation live counts still checked. The Rust standard library is uninstrumented.

All **24,318** generated differential configurations match the preceding runtime exactly, including callbacks, fragmentation, final positions, acceptance and error codes. The real-project runner passes all 24 full callback preflights and 336 timed processes. Independent safety and simplification reviews are retained.

[evidence.tar.gz](evidence.tar.gz) contains the raw validation, benchmark and profiling records, discarded experiments, exact source/build metadata and reviews. [files.json](files.json) lists every member hash; [summary.json](summary.json) provides compact results. Archive SHA256: `22e13ecaaa66160d5c0b1b33efed2033637dc299bbbf5578505f22ee72dcfdb4`.
