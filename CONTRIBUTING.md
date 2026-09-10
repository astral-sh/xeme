# Contributing

Use Rust 1.96 or later. The parser is implemented in this repository; it does not
wrap an existing XML parser or invoke one at runtime.

```console
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The safe parser forbids unsafe code. C ABI code documents pointer validity,
allocation ownership, and callback lifetimes at each unsafe boundary. Library
crates leave global allocator selection to their embedding application.

Changes to compatibility behavior need a regression test and a comparison with
the reference Expat implementation. Preserve reduced failures in the corpus.
Unsupported features must be visible in the compatibility documentation and must
not silently accept malformed XML.

## Review and performance

Keep parser behavior, C integration, validation, and optimizations in separately
reviewable changes. Review input limits and callback reentry independently of the
feature implementation. Record unresolved findings as release blockers.

Measure optimizations against their parent on the same input, with equivalent
callbacks and chunk sizes. Save raw samples, source and binary hashes, compiler
versions, allocator configuration, and output comparisons. Measure allocation
requests separately from elapsed time and peak process memory. Report regressions
and negative experiments. A new dependency or data structure needs a demonstrated
benefit that justifies its complexity.

## Acceptance

Production replacement requires XML conformance, Expat API and callback
compatibility, unmodified CPython XML tests, bounded adversarial input processing,
sanitizer and fuzz coverage, native platform validation, and an opt-in
python-build-standalone integration. Passing a subset does not establish the full
replacement contract. Evidence identifies the exact revision it tested.

