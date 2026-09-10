# Streaming ATTLIST integration

We publish attributes from a complete ATTLIST declaration one at a time. The continuation owns its lexical token, and DTD tables are committed before each callback. Earlier callbacks and callback-created parameter children can affect later attributes. A monotonic position cursor avoids rescanning the declaration for each callback. Incremental input still waits for the complete lexical token.

The root source matches all 66 independently validated files, and its release build produces byte-identical shared and static libraries. We reuse those exact-binary gates through `reused-evidence.json`: 365 full workspace checks, strict workspace Clippy and formatting, six native C sanitizer programs with 327 allocation scenarios per linkage, 1,518 exact baseline comparisons, and the full unchanged 4,740-configuration API matrix. The matrix improves from 4,113 passing / 627 failing to **4,115 passing / 625 failing**, with exactly two `test_alloc_realloc_attributes` whole-input configurations fixed. The 365-check count includes storage-crate tests; the preceding README count of 326 covered the core and adapter only.

Independent review replays 3,656 complete callback records and checks continuation ownership, source compaction, handler changes, custom encoding provenance, DefaultCurrent, suspension and callback-created child publication. The final read-only security review finds no additional blocker. Complete-token callback delay and the existing Default, negative-input coordinate and publication differences remain recorded. C sanitizers instrument the consumers, with uninstrumented release Rust and LeakSanitizer disabled; selected allocation cleanup is checked separately.

## Resource and diagnostic findings

Root review found and repaired an intermediate path that skipped repeated-element-name work when enumeration callbacks were absent. An 8,192-byte element name with 1,025 attributes succeeds at the existing 8 MiB work boundary; 1,026 returns error 43. The bypassing candidate, patch and failing observations remain in the [original studies](../../../../benchmarks/results/2026-09-10/streaming-attlist-study/).

All configured limits remain unchanged. Omitting unused semantic callback payloads intentionally omits their adapter payload-byte charge, so complete counter parity is not claimed. Original allocation measurements show lower peak storage for large declarations, with an additional capture allocation per enumeration callback. Those earlier-source measurements are kept separate from current-binary timing.

The broader custom-encoding comparison retains 1,356 changed error byte indices. All are independently verified corrections to match actual Expat, with unchanged acceptance, error codes and callback content/order. The initial zero-difference assertion and subsequent reference comparison remain visible. The separate 7,644 external semantic comparisons and all other saved broader observations remain unchanged.

## Performance

The current library is compared with the exact event-output baseline and Expat 2.8.4 on six pinned project files, both namespace modes and 4 KiB/64 KiB chunks, plus two generated controls. Five randomized cohorts use one warmup and fixed measured counts from the preceding confirmation: Vulkan 7, Wayland 64, Maven 128, Batik 512, GTK/DocBook 256 and generated controls 32. All 24 normalized project preflights and every timed callback hash/count pass.

The geometric project speedup is 1.0002×, with 11 of 24 conditions improving; the results are mixed and close to parity. The generated controls take about 2–4% longer. This is a compatibility and memory change, not a general throughput improvement. Oriole still takes 2.54× Expat's time geometrically. CPU 0 is pinned on a shared host; CPU frequency and host load remain uncontrolled.

The initial timing cohort overlapped an unrestricted-affinity normalized-preflight process. Its first four Vulkan 4 KiB cohorts took roughly twice as long in all three engines. Every sample remains in `initial-overlap-screen`; the primary results above repeat all 28 conditions after preflight completion, with unchanged inputs, libraries, iterations and a new fixed random seed. No condition or unfavorable sample was selectively removed.

Source snapshots, exact build hashes, commands, original logs, prior negative phases and raw timing samples are retained. Actual CPython and Wayland timings belong to the separately identified preceding event-output library. No fresh local full CPython, W3C, sustained Rust fuzzing or PBS distribution campaign is claimed for this composition.
