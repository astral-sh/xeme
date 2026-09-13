# XML name throughput and namespace compatibility

We retain unprefixed attribute names without expanding or hashing them again, store one element name when its raw and expanded spellings agree, and validate names in one pass. All end-event names remain owned. Unprefixed names may equal another attribute's serialized namespace name; prefixed-name collisions retain Expat's behavior. A NUL separator now ignores triplet mode, matching Expat.

The quoted-value scanning experiment regressed ordinary workloads and is excluded. Raw-attribute offset scratch is also excluded pending measurement with the next storage-recycling layer. Their exact patches, binaries' provenance and timing samples remain in the evidence archive.

## Measurements

Seven randomized process pairs measure twenty parses after a warmup on each of six original project inputs, both namespace modes and 4/64 KiB feeds. All 24 complete normalized callback preflights and 336 timed processes pass. Creation, callbacks, parsing and free are timed; file/library loading and process startup are excluded. Shared-host CPU frequency and other-CPU load remain uncontrolled. These are parser workloads, not complete project builds.

The final shared library is `4795fe261be9de67ad4e38f91fb5a5ad6b5785d55b6dc5d7a00c2b482cba957b`; the static archive is `690c6bf72476ee17cff6533b80bec49f3bb0d5a608420cfe0eb5c60ae86ba7bf`. Pinned Expat 2.8.4 remains the reference.

At 4 KiB with namespaces disabled:

| Project | Oriole ms | Expat ms | Expat / Oriole |
| --- | ---: | ---: | ---: |
| batik | 0.362 | 0.137 | 0.380× |
| docbook | 0.635 | 0.196 | 0.310× |
| gtk | 0.879 | 0.218 | 0.249× |
| maven | 1.907 | 0.480 | 0.250× |
| vulkan | 113.740 | 28.929 | 0.255× |
| wayland | 3.986 | 1.084 | 0.272× |

The equal-project geometric mean remains 3.54× slower without namespaces and 3.57× slower with namespaces at 4 KiB; at 64 KiB the gaps are 3.85× and 3.83×. Paired screening against the preceding runtime shows roughly 3–11% throughput improvement without namespaces and 4–15% with namespaces; the complete final comparison above remains slower than Expat on every project. The performance goal is unfinished.

## Validation

All 227 core/C-adapter Rust tests pass, including selected-allocation failures. Formatting and strict Clippy pass. All 1,518 generated differential configurations match the preceding runtime exactly; the separate 84-case namespace probe matches pinned Expat, including the newly corrected cases. Shared/static C integration, adversarial and allocation-failure suites pass under ASan/UBSan, with 332 injected failures per linkage. The Rust library is uninstrumented in those C runs; leak detection is disabled and explicit allocation live counts are checked.

Independent name-lifetime and namespace reviews found no issues. A Callgrind instruction profile is included for further work; it is not hardware sampling or wall-clock evidence.

[evidence.tar.gz](evidence.tar.gz) contains raw checks, benchmark results, discarded experiments, source/build metadata and reviews, excluding ELF executables and static archives. [files.json](files.json) verifies every archived member; [summary.json](summary.json) provides compact results. Archive SHA256: `c6d5d9d1b7a3f7d105355fd700b627d03fee9a5c828f87c3807611926d683397`.
