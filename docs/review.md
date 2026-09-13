# Reviewing Oriole

## Implementation

| Area | Entry points |
| --- | --- |
| Incremental parsing and public Rust API | [`oriole`](../crates/oriole/src/lib.rs) |
| Input decoding and original byte positions | [`encoding.rs`](../crates/oriole/src/encoding.rs) |
| Tags, XML names, and references | [`tag.rs`](../crates/oriole/src/tag.rs), [`names.rs`](../crates/oriole/src/names.rs), [`lexical.rs`](../crates/oriole/src/lexical.rs) |
| DTDs and entity replacement | [`dtd.rs`](../crates/oriole/src/dtd.rs), [`dtd/`](../crates/oriole/src/dtd/), [`value.rs`](../crates/oriole/src/value.rs) |
| Fallible allocation and ownership | [`oriole_storage`](../crates/oriole_storage/src/lib.rs) |
| Expat ABI, callback frames, and parser families | [`oriole_expat`](../crates/oriole_expat/src/lib.rs) |
| Command-line input and output | [`oriole_cli`](../crates/oriole_cli/src/main.rs) |

The core forbids unsafe Rust. Review C pointer validity, allocator ownership, and
callback lifetimes at the adapter boundary. Callbacks may suspend parsing, change
handlers, or operate on another parser; no Rust borrow of the active parser may
cross a callback. The [C interface guide](../crates/oriole_expat/README.md) defines
the supported operations and ownership contracts.

Changes to streaming paths need coverage for split input, encoding conversion,
error positions, suspension, and allocation failure. Keep resource accounting
before the work or allocation it limits. External children share budgets and
allocator state, and may outlive a reset or freed parent.

## Validation

Start with the workspace tests, formatting, and Clippy commands in
[contributing](../CONTRIBUTING.md). Choose additional checks for the affected
behavior:

| Change | Checks |
| --- | --- |
| XML acceptance, events, or diagnostics | [`tools/differential.py`](../tools/differential.py), [W3C corpus](../tools/w3c/README.md) |
| Expat API behavior | [Upstream Expat tests](../tools/upstream-expat/README.md), [native consumer tests](compatibility.md#native-consumer-tests) |
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
