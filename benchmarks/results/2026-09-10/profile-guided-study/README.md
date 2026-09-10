# Profile-guided optimization study

On the frozen streaming-ATTLIST source (`0546fcf5` ordinary shared library), generated-input profile-guided optimization reduces Oriole's native project time by 22.2% geometrically. Expat trained on the same generated inputs improves by 12.7%. The fair PGO-versus-PGO comparison still takes 2.26× Expat's time overall; only one of 24 conditions is an Oriole win. This study supports an optional build workflow, not a general faster-than-Expat claim.

Twelve fresh generated documents train both engines. All six real project inputs remain held out. Separate normal, instrumented and profile-use builds preserve the existing runtime, allocators and resource limits. LLVM tools match the recorded compiler exactly, both optimized builds have no missing/mismatched profile warnings, and training replay preserves every recorded callback digest/count within each engine. Original tool-version rejection and build-context differences remain recorded.

Ninety-six normalized callback preflights finish before seven randomized four-engine cohorts on CPU 0: 672 native processes, 136,976 measured parses and 672 warmups. Every raw sample is retained, with input, driver, source, profile and library identities. The shared host's frequency and external load remain uncontrolled. Instruction counts are a separate measure from wall time.

Independent reviews verify the source/profile provenance, reconstruct all native callback hashes and recompute all paired ratios. The archive contains all 131 verified author artifacts, including complete raw profiles, generated training inputs and records, all held-out preflights/timings, exact commands, source snapshots and caveats. Runtime libraries and tool binaries are represented by hashes.

This study precedes the final buffer-reservation source. Its profile must not be reused for newer code. The subsequent portable workflow regenerates training profiles for each source/compiler build and reports its own validation and performance evidence. Actual CPython and Wayland timings are not part of this initial native-driver study.
