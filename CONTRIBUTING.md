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

## Python bindings

With Python 3.10 or later, run these commands from the repository root:

```console
uv venv
uv pip install .
uv run --no-sync python -m unittest discover -s crates/xeme_python/tests -v
uv build --wheel --out-dir dist
uv build --sdist --out-dir dist
```

Reinstall with `uv pip install --reinstall .` after changing Rust code. Keep type
annotations synchronized with the bindings. See the
[Python API guide](crates/xeme_python/README.md) for API behavior and examples.

## Review and performance

The [review guide](docs/review.md) maps the implementation and validation tools.
See the [compatibility guide](docs/compatibility.md) for known differences and
release gates.

Measure optimizations against a fresh parent build on the same input, with
equivalent compilers, allocators, callbacks, and chunk sizes. Include raw samples,
compiler and allocator configuration, and output comparisons with the pull
request. Measure allocation requests separately from elapsed time and peak process
memory. Report regressions.
A new dependency or data structure needs a demonstrated benefit that justifies
its complexity.

The performance target is within roughly 20% of Expat on representative,
held-out project XML, measured separately through the C interface and CPython.
Report individual workload results alongside aggregates, and keep generated
stress workloads separate. Compiler and allocator experiments must meet the
same compatibility and safety requirements as the default build.
