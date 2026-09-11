# Deferred C coordinates prototype

**Draft: performance and Miri results are pending.** Local checks and source review support evaluating this implementation; they do not establish a speedup or production readiness.

We defer line and column calculations across eligible native UTF-8 C callback deliveries. Byte coordinates stay current. Line/column getters resolve and cache the exact committed event location, while ordinary Rust event APIs retain eager positions. Source compaction, feeds, conversion and generic fallbacks resolve pending coordinates before their source bytes can change.

The Source retains separate committed and prospective event starts. A failed Start preserves the previous publication; a deliverable Text prefix still precedes its sticky accounting error. Opaque descriptors bind parser generation and positive byte range, so stale or foreign descriptors fail closed. No source borrow crosses a C callback, and the safe core introduces no unsafe code or interior mutability.

## Local validation

All 443 core, C-adapter and storage tests across 32 groups passed, plus the compile-fail doctest. Strict Clippy and formatting passed. A subsequent test-only change makes the allocation sweep positively witness a failed Start after unresolved native Text; that affected test, Clippy and formatting passed again. Runtime code stayed unchanged after the full suite.

Focused cases cover exact event/raw/position comparisons, UTF-8 and UTF-16, namespaces and DTD fallback, CRLF, 64 KiB compaction, committed/prospective publication, stale and foreign frames, rejected feeds, selected-allocator failures, repeated getters, suspension, reset-release visibility, and callback pointer validity after getter reentry.

[Independent source review](source-review.md), [check summary](summary.json), [source manifest](source.json), and [attempt notes](attempt-notes.md) retain commands, source/log hashes and the initial mechanical compiler failure. The local Ohm toolchain lacks Miri; the existing CI matrix now verifies and runs all three new coordinate tests under both aliasing models, alongside existing context tests. Runner selection keeps the owner-sensitive uv workflow pattern.

## Memory cost

Saved baseline DWARF and current test output establish these x86-64 sizes:

| Type | Baseline | Prototype | Increase |
| --- | ---: | ---: | ---: |
| Source | 384 B | 520 B | 136 B |
| Parser | 2,408 B | 2,416 B | 8 B |
| AdapterFrame | 256 B | 264 B | 8 B |
| C parser | 3,160 B | 3,176 B | 16 B |

The C parser increase includes its embedded Parser. [Layout readback](layout-readback.json) identifies the exact baseline artifacts. The allocation tracker accounts for the new sizes. No reduction in retained source bytes or allocation sizes is claimed.
