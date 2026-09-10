# Oriole

A streaming XML parser in Rust. Check XML from the command line, use the Expat C
interface, or embed the parser in your own tools.

Oriole provides incremental parsing, namespace processing, entity expansion, and
DTD attribute defaults through a reusable library. The Expat-compatible C interface
is the first integration target, with an opt-in python-build-standalone recipe for
CPython's XML consumers.

**Oriole is experimental.** It checks XML syntax within the supported feature set.
It is not yet a production-ready replacement for Expat. See
[compatibility](docs/compatibility.md) for supported features and remaining gaps.

## Installation

Build from this checkout with Rust 1.96 or later:

```console
cargo install --path crates/oriole_cli --bin oriole --locked
```

The parser uses no XML parser dependency. The library performs no filesystem or
network I/O; applications provide input and resolve external entities themselves.
The CLI reads a file or standard input and does not fetch external resources.

The standalone executable uses jemalloc on supported Unix platforms and mimalloc
on Windows. Install with `--no-default-features` to use the system allocator.
Library users retain control over their allocator.

## Check XML

Given `message.xml`:

```xml
<message>Hello &amp; goodbye</message>
```

Check the document or print its parser events:

```console
oriole message.xml
oriole --events message.xml
oriole --events --namespaces --chunk-size 4096 message.xml
```

Omit the file or pass `-` to read standard input. Invalid XML exits with a nonzero
status. `--events` prints the event stream, `--namespaces` expands namespace names
with `|` as the separator, and `--chunk-size` controls input buffering.

The parser handles elements, attributes, namespaces, comments, processing
instructions, CDATA, character references, internal and caller-resolved external
entities, and DTD attribute defaults. It supports UTF-8, UTF-16, ASCII, and
ISO-8859-1 input. It is non-validating: it checks XML syntax without validating
documents against their DTD content models.

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
strings, so callers can retain them independently of subsequent input. Namespace
processing is opt-in through `Config::namespace_separator`; namespace triplets
are optional. Names use XML 1.0 Fifth Edition rules by default;
`Config::name_rules` can select Fourth Edition rules. The C interface selects
Fourth Edition to match Expat. The parser core forbids unsafe Rust.

### Expat interface

The [C interface](crates/oriole_expat/) exports shared and static libraries with
Expat's narrow-character ABI, callbacks, parser reset, suspension, buffer input,
and custom allocation. Its documentation defines ownership and callback lifetimes,
supported encoding modes, and remaining compatibility gaps.

The [python-build-standalone integration](integration/python-build-standalone/)
links Oriole into CPython 3.12.13. The recipe is opt-in and currently validated for
Linux x86-64, including the glibc 2.17 baseline.

### Resource limits

`Config::limits` bounds document bytes, unfinished tokens, element depth,
attributes, entity declarations, and entity expansion. Defaults allow 256 MiB of
input, 16 MiB tokens, 256 nested elements, and 8 MiB of entity expansion. Applications
should choose limits suited to their inputs.

Incremental token scanning retains its progress across chunks to avoid repeatedly
scanning a growing unfinished token. Internal entity replacement must remain
balanced and is bounded separately from document input. The C interface also
bounds aggregate allocation and work across a parser's external-entity family.

## Validation

The [validation report](docs/validation/2026-09-10/final-runtime/) identifies the
source, inputs, commands, and binaries used for compatibility and safety checks.
Four actual CPython 3.12.13 shared/static, original/fixed consumer configurations
succeed across all six XML modules, with 803 reported tests and 31 skips per run.
The [complete PBS distribution](docs/validation/2026-09-10/pbs-final/) also passes
its archive validator and installed XML suites, including on glibc 2.17.

The generated differential corpus passes 12,318 semantic comparisons. Exact
callback fragmentation and final-position differences remain. The full adapted
Expat API matrix reports 3,753 passing and 987 failing configurations; diagnostic
experiments do not waive those failures. The W3C acceptance corpus reports 5,916
passing and six failing mandatory checks.

Three sustained Rust AddressSanitizer campaigns complete 3.9 million executions
without findings, alongside corpus replays, allocation-failure probes, and
independent source reviews. See [fuzzing](fuzz/README.md) for the harnesses and
[compatibility](docs/compatibility.md) for the separate release gates.

## Benchmarks

The [recorded benchmarks](benchmarks/results/2026-09-10/final-runtime/) compare
Oriole's C interface with Expat 2.8.4 on generated XML. These results use 4 KiB
chunks with namespace processing disabled, on a shared Linux AMD EPYC-Milan host.
Seven randomized process pairs are run for each workload; each process measures
ten parses after one discarded warmup. Complete normalized callbacks are compared
before timing.

| Workload | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Elements | 19.442 ms | 3.658 ms | 5.30× |
| Text | 10.150 ms | 1.609 ms | 6.31× |
| Entity references | 24.031 ms | 2.133 ms | 11.32× |
| Prefixed names | 22.249 ms | 3.222 ms | 6.85× |

Times are medians of process medians; ratios are medians of paired ratios.
Oriole remains slower on these workloads. The report retains raw samples,
namespace-enabled results, separate system/jemalloc/mimalloc measurements through
the safe Rust API, and DTD scaling checks. Host load and CPU frequency are
uncontrolled; these generated workloads do not establish CPython application
performance. See the [benchmark guide](benchmarks/README.md) to reproduce them.

## Development

| Crate | Responsibility |
| --- | --- |
| [`oriole`](crates/oriole) | Streaming XML parser and owned events |
| [`oriole_storage`](crates/oriole_storage) | Fallible storage, allocator ownership, and resource accounting |
| [`oriole_expat`](crates/oriole_expat) | Expat C interface and callback integration |
| [`oriole_cli`](crates/oriole_cli) | Command-line XML checker |

```console
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

See [contributing](CONTRIBUTING.md) for review, performance, and production
acceptance criteria, and the [stack review guide](docs/review.md) for the
implementation and evidence layers. Expat is used by validation tools as an
independent reference.

## License

Oriole is licensed under either the [Apache License, Version 2.0](LICENSE-APACHE),
or the [MIT license](LICENSE-MIT), at your option.
