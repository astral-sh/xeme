# Reviewing Xeme

## Implementation

| Area | Entry points |
| --- | --- |
| Incremental parsing and public Rust API | [`xeme`](../crates/xeme/src/lib.rs) |
| Input decoding and original byte positions | [`encoding.rs`](../crates/xeme/src/encoding.rs) |
| Tags, XML names, and references | [`tag.rs`](../crates/xeme/src/tag.rs), [`names.rs`](../crates/xeme/src/names.rs), [`lexical.rs`](../crates/xeme/src/lexical.rs) |
| DTDs and entity replacement | [`dtd.rs`](../crates/xeme/src/dtd.rs), [`dtd/`](../crates/xeme/src/dtd/), [`value.rs`](../crates/xeme/src/value.rs) |
| Fallible allocation and ownership | [`xeme_storage`](../crates/xeme_storage/src/lib.rs) |
| Expat ABI, callback frames, and parser families | [`xeme_expat`](../crates/xeme_expat/src/lib.rs) |
| Command-line input and output | [`xeme_cli`](../crates/xeme_cli/src/main.rs) |

The core forbids unsafe Rust. Review C pointer validity, allocator ownership, and
callback lifetimes at the adapter boundary. Callbacks may suspend parsing, change
handlers, or operate on another parser; no Rust borrow of the active parser may
cross a callback. The [C interface guide](../crates/xeme_expat/README.md) defines
the supported operations and ownership contracts.

Changes to streaming paths need coverage for split input, encoding conversion,
error positions, suspension, and allocation failure. Keep resource accounting
before the work or allocation it limits. External children share budgets and
allocator state, and may outlive a reset or freed parent.

## Validation

CI runs the complete pinned API, W3C and differential regression gates through
[`tools/compatibility.py`](../tools/compatibility.py). Known strict failures are
checked against exact assertions and complete inventories; a passing regression
gate does not turn those upstream failures into passes.

Start with the workspace tests, formatting, and Clippy commands in
[contributing](../CONTRIBUTING.md). Choose additional checks for the affected
behavior:

| Change | Checks |
| --- | --- |
| XML acceptance, events, or diagnostics | [`tools/differential.py`](../tools/differential.py), [W3C corpus](../tools/w3c/README.md) |
| Expat API behavior | [Upstream Expat tests](../tools/upstream-expat/README.md), [native consumer tests](../tests/c/) |
| Storage or callback ownership | Allocator and callback Miri jobs in [CI](../.github/workflows/ci.yml), [fuzz harnesses](../fuzz/README.md) |
| Python consumers | [CPython extension harness](../tools/cpython/README.md) |
| Distribution packaging | [python-build-standalone recipe](../integration/python-build-standalone/README.md) |
| Performance | [Benchmarks](../benchmarks/README.md), [performance iteration guide](../benchmarks/HILLCLIMB.md) |

Compare compatibility changes with reference Expat and keep reduced failures as
regression tests. Exact callback fragmentation and diagnostic positions matter to
some consumers even when acceptance and coalesced text agree. The
[compatibility guide](compatibility.md) describes known differences and resource
limits.

See [contributing](../CONTRIBUTING.md#review-and-performance) for performance
comparison requirements.

The [evidence index](evidence/README.md) separates tested-source reports from current
contracts and links checksummed raw archives. Benchmark results must identify the
measured runtime; historical qualification does not cover later code changes.
