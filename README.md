# Oriole

A streaming XML parser in Rust. Parse XML incrementally through owned events,
without building a document tree.

**Oriole is experimental.** The C interface targets Expat, but API and callback
compatibility are incomplete. It is not yet a production-ready replacement for
Expat. See the [C interface](crates/oriole_expat) for supported modes and gaps.

## Use the library

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

Pass `false` until the final chunk. A parse error is terminal. Events own their
strings, so callers can retain them independently of subsequent input.

The parser handles elements, attributes, namespaces, comments, processing
instructions, CDATA, character references, internal entities, and DTD attribute
defaults. It supports UTF-8, UTF-16, ASCII, and ISO-8859-1 input. Namespace processing
is opt-in through `Config::namespace_separator`; namespace triplets are optional.
The parser is non-validating: it checks XML syntax, without validating documents
against their DTD content models.

The core forbids unsafe Rust and uses no XML parser dependency. It performs no
filesystem or network I/O and leaves allocator selection to the application.

## Check XML from the command line

```console
cargo run --release -p oriole_cli -- document.xml
cargo run --release -p oriole_cli -- --events --namespaces document.xml
```

Omit the file to read standard input. Invalid XML exits with a nonzero status;
`--events` prints the event stream and `--chunk-size` controls input buffering.

The standalone executable uses jemalloc on supported Unix platforms and mimalloc
on Windows, following uv's allocator configuration. Build with
`--no-default-features` to use the system allocator. Library users retain control
over their allocator.

## Resource limits

`Config::limits` bounds document bytes, unfinished tokens, element depth,
attributes, entity declarations, and entity expansion. Defaults allow 256 MiB of
input, 16 MiB tokens, 256 nested elements, and 8 MiB of entity expansion. Limits are
part of the parser configuration; applications should choose values suited to
their inputs.

Incremental token scanning retains its progress across chunks to avoid repeatedly
scanning a growing unfinished token. Internal entity replacement must remain
balanced and is bounded separately from document input.

## Compatibility and performance

The [validation report](docs/validation/2026-09-10/) records differential testing,
actual CPython consumers, native C allocation and callback probes, and sanitizer
campaigns. It retains the upstream Expat failures alongside passing results.
The [PBS integration](integration/python-build-standalone/) is opt-in.

[Benchmarks](benchmarks/README.md) compare Expat and Oriole,
including system, jemalloc, and mimalloc configurations. Oriole remains slower
than Expat on the generated C workloads; measured optimizations and their tradeoffs
are retained as separate comparisons.

## Development

Build with Rust 1.96 or later:

```console
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

See [contributing](CONTRIBUTING.md) for review, performance, and production
acceptance criteria.

## License

Oriole is licensed under either the [Apache License, Version 2.0](LICENSE-APACHE),
or the [MIT license](LICENSE-MIT), at your option.
