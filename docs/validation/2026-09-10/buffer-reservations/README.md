# Input buffers and captured ATTLIST types

We reserve bounded initial blocks when UTF-8 decoded input and raw callback context first receive nonempty bytes. Existing buffers keep ordinary geometric growth. The parser constructor remains lazy. Captured ATTLIST enumeration types reserve their remaining lexical suffix once, excluding XML whitespace and later defaults in valid declarations.

The final source is `fcf2813`, with shared library `9cc87e9bc5c22dfc33eba7bf074908421dd7b1c303920f9c8021eab887793213` and static archive `b803db7dc365b54ef4f1070b9057d0f284fdbd76f3c6549f94eb431be85f60eb`. Root builds match both independently tested artifacts and all 64 author source files. The root manifest additionally records its complete source inventory.

## Original API results

The unchanged 4,740-configuration upstream suite now has **4,323 passing / 417 failing** configurations. Relative to the preceding ATTLIST layer, 220 failing configurations pass and 12 namespace binding reallocation assertions newly fail. Those 12 expect a buffer reallocation that successful parses no longer need. Independent reset probes retain exact semantic metadata, verify no failed realloc callback occurred on those successful paths, and check zero selected allocations after free. We retain the original failing assertions and do not insert artificial allocations to satisfy them.

The initial composition had 4,303 passing / 437 failing configurations: enumeration type capture repeatedly grew its allocation and consumed retry allowances. Reserving the available enumeration once restores 20 cases, with no regression against that immediate composition. The [buffer-only study](../../../../benchmarks/results/2026-09-10/buffer-minimum-study/) and [initial composition](../../../../benchmarks/results/2026-09-10/buffer-initial-composition/) preserve all original counts, failed assertions and allocation attribution.

## Validation and memory

The root runs 367 workspace tests plus one documentation test, strict workspace Clippy and formatting. Normal Ohm release builds match the independently validated shared/static libraries. The initial sandbox build failed read-only, and Ohm then repeated a cached no-space error despite ample free disk. Tests and Clippy pass with `cargo +ohm -Zohm-defaults=no`; the ordinary release configuration is unchanged. All initial errors and retry logs remain available; no shared cache was deleted.

Root comparison passes 1,518 exact cases, 36,456 custom-encoding cases, malformed-input oracles and incremental suspension/DefaultCurrent checks. Exact-library author evidence includes six native C sanitizer programs with 327 allocation scenarios per linkage, the complete original API matrix, 3,764 exact callback/control comparisons and 108 additional actual-Expat controls. Existing Default fragmentation and per-call differences remain visible. C sanitizers instrument the consumers, while release Rust is uninstrumented and LeakSanitizer is disabled; selected live allocations are checked separately.

The original buffer-only study reports 2,032 additional selected bytes for tiny UTF-8 input, unchanged constructor storage, and reduced one-byte context/decoded reallocations from 23 to 5 on a namespace fixture. These measurements identify their own earlier binary. Final capture probes retain only 56–62 selected bytes for short enumerations surrounded by 1 MiB whitespace and a 1 MiB later default. All 16 memory observations free every selected block; peak differences are at most one byte relative to the immediate composition.

All allocation and work limits remain unchanged. For malformed declarations without a closing parenthesis, the new capture scan may reserve against the bounded remaining token and report allocation failure before the later syntax error. It preserves nonfailing grammar outcomes and handler-sensitive capture, callback ownership, positions and existing expansion charges. Buffer reservation bounds are not a promise of exact allocator capacity, which has its own minimum allocation floor.

## Project performance

All 24 normalized project preflights finish before timing on separate CPUs. Five randomized native cohorts compare this final library with the immediate buffer composition and Expat 2.8.4, using six original project files, both namespace modes, 4 KiB/64 KiB chunks, and two generated controls. Fixed measured counts range from 7 to 512 parses per process, plus one warmup. All 420 processes preserve callback counts and hashes, and input/driver/library hashes match before and after collection.

The final capture repair has 1.0010× geometric project speedup, with 12 of 24 conditions positive. Results are mixed and near parity; no general throughput improvement is claimed. Oriole takes 2.52× Expat's time overall. CPU 0 is pinned, while host load and CPU frequency remain uncontrolled. Earlier component and initial-composition measurements are retained separately, including their collection limitations.

This layer improves allocation behavior and compatibility. It does not meet the faster-than-Expat goal. Actual CPython/Wayland consumer measurements, W3C conformance, sustained fuzz and PBS distribution reports identify their own source and build configurations; they must not be assigned to this library without an explicit final-source run.
