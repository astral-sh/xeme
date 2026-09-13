# Rust library

Oriole provides incremental XML parsing with owned events. The parser core forbids
unsafe Rust and has no XML parser dependency. It performs no filesystem or network
I/O; applications provide input and resolve external entities themselves.

## Parse incrementally

Use `crates/oriole` as a Cargo path dependency. Feed a chunk, then drain its events:

```rust
use oriole::{Config, Parser};

fn main() -> Result<(), oriole::Error> {
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

## Expat consumers

The [C interface](../crates/oriole_expat/) exports shared and static libraries with
Expat's narrow-character ABI, callbacks, parser reset, suspension, buffer input,
and custom allocation. Its documentation defines ownership and callback lifetimes,
supported encoding modes, and remaining compatibility gaps.

The [python-build-standalone integration](../integration/python-build-standalone/)
links Oriole into CPython 3.12.13. The recipe is opt-in and targets Linux x86-64.
It includes checks for the glibc 2.17 baseline; see the
[compatibility guide](compatibility.md) for remaining consumer differences.

## Workspace

| Crate | Responsibility |
| --- | --- |
| [`oriole`](../crates/oriole) | Streaming XML parser and owned events |
| [`oriole_storage`](../crates/oriole_storage) | Fallible storage, allocator ownership, and resource accounting |
| [`oriole_expat`](../crates/oriole_expat) | Expat C interface and callback integration |
| [`oriole_cli`](../crates/oriole_cli) | Command-line XML checker |

See [contributing](../CONTRIBUTING.md) for development commands and acceptance
criteria, [fuzzing](../fuzz/README.md) for the adversarial harnesses, and the
[review guide](review.md) for implementation and test entry points.
