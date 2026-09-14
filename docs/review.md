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
the supported operations and ownership rules.

Changes to streaming paths need coverage for split input, encoding conversion,
error positions, suspension, and allocation failure. Keep resource accounting
before the work or allocation it limits. External children share budgets and
allocator state, and may outlive a reset or freed parent.

## Validation

CI runs the pinned API, allocation-behavior, W3C and differential suites through
[`tools/compatibility.py`](../tools/compatibility.py). It checks known failures
against the assertion baseline and verifies that every expected test ran.
All adapted allocation tests and ownership checks must pass.

Start with the workspace checks:

```console
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Choose additional checks for the affected behavior:

| Change | Checks |
| --- | --- |
| XML acceptance, events, or diagnostics | [`tools/differential.py`](../tools/differential.py), [W3C corpus](../tools/w3c/README.md) |
| Expat API behavior | [Upstream Expat tests](../tools/upstream-expat/README.md), [native consumer tests](../tests/c/) |
| Storage or callback ownership | Allocator and callback Miri jobs in [CI](../.github/workflows/ci.yml), [fuzz harnesses](../fuzz/README.md) |
| Python consumers | [CPython extension harness](../tools/cpython/README.md) |
| Performance | [Benchmarks](../benchmarks/README.md), [performance iteration guide](../benchmarks/HILLCLIMB.md) |

Compare compatibility changes with reference Expat and keep reduced failures as
regression tests. Exact callback fragmentation and diagnostic positions matter to
some consumers even when acceptance and coalesced text agree. The
[compatibility guide](compatibility.md) describes known differences and resource
limits.

The [test reports](evidence/README.md) link raw results and their checksums.
Include the tested revision with benchmark results.
