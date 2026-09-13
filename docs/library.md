# Rust library

Xeme provides incremental XML parsing with owned events. The parser core forbids
unsafe Rust and has no XML parser dependency. It performs no filesystem or network
I/O; applications provide input and resolve external entities themselves.

## Parse incrementally

Use `crates/xeme` as a Cargo path dependency. Feed a chunk, then drain its events:

```rust
use xeme::{Config, Parser};

fn main() -> Result<(), xeme::Error> {
    let mut parser = Parser::new(Config::default());
    parser.feed(b"<message>Hello &amp; goodbye</message>", true)?;
    while let Some(event) = parser.next_event()? {
        println!("{:?}", event.kind);
    }
    Ok(())
}
```

Pass `false` until the final chunk. Parse errors are terminal except for an
unresolved custom encoding, which can be supplied through `set_encoding_map`.
Events own their strings, so callers can retain them independently of subsequent
input. Namespace processing is opt-in through `Config::namespace_separator`;
namespace triplets are optional.

Names use XML 1.0 Fifth Edition rules by default. `Config::name_rules` can select
Fourth Edition rules, which the C interface uses to match Expat. The parser is
non-validating: it checks XML syntax without validating documents against their
DTD content models.

## Resource limits

`Config::limits` bounds document bytes, unfinished tokens, element depth,
attributes, entity declarations, and entity expansion. Defaults allow 256 MiB of
input, 16 MiB tokens, 256 nested elements, and 8 MiB of entity expansion.
Applications should choose limits suited to their inputs.

Incremental token scanning retains its progress across chunks to avoid repeatedly
scanning a growing unfinished token. Internal entity replacement must remain
balanced and is bounded separately from document input. The C interface also
bounds aggregate allocation and work across a parser's external-entity family.
Library crates leave global allocator selection to the embedding application.

See [XML denial-of-service protections](security.md) for entity-expansion and
large-token protections, and the limits required when a caller decompresses input.

## Python API

The [`xeme_python` crate](../crates/xeme_python/) provides an independently
installable `xeme` Python package backed by PyO3. Install it from this checkout
with `uv pip install .`, then feed bytes and consume owned events:

```python
from xeme import Parser

parser = Parser()
parser.feed(b"<message>Hello</message>", final=True)
for event in parser.read_events():
    print(event.kind, event.data)
```

The package exposes namespace options, configurable limits, source positions,
and structured parse errors. Its event API is separate from `pyexpat`'s callback
interface. See its [documentation](../crates/xeme_python/README.md) for supported
events, incremental input, and external-entity restrictions.

## Expat consumers

The [C interface](../crates/xeme_expat/) exports shared and static libraries with
Expat's narrow-character ABI, callbacks, parser reset, suspension, buffer input,
and custom allocation. Its documentation defines ownership and callback lifetimes,
supported encoding modes, and remaining compatibility gaps.

The [CPython consumer harness](../tools/cpython/README.md) builds CPython 3.12.13's
XML extensions against Xeme and runs their upstream tests. See the
[compatibility guide](compatibility.md) for remaining consumer differences.

## Workspace

| Crate | Responsibility |
| --- | --- |
| [`xeme`](../crates/xeme) | Streaming XML parser and owned events |
| [`xeme_storage`](../crates/xeme_storage) | Fallible storage, allocator ownership, and resource accounting |
| [`xeme_expat`](../crates/xeme_expat) | Expat C interface and callback integration |
| [`xeme_cli`](../crates/xeme_cli) | Command-line XML checker |
| [`xeme_python`](../crates/xeme_python) | Python event API and extension package |

See [contributing](../CONTRIBUTING.md) for development commands and acceptance
criteria, [fuzzing](../fuzz/README.md) for the adversarial harnesses, and the
[review guide](review.md) for implementation and test entry points.

[Validation evidence](evidence/README.md) identifies the runtime and installed
artifact each report tested; earlier results do not qualify later changes.
