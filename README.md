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

Pass `false` until the final chunk. Parse errors are terminal except for an
unresolved custom encoding, which can be supplied through `set_encoding_map`.
Events own their strings, so callers can retain them independently of subsequent input. Namespace
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

The [current integration report](docs/validation/2026-09-10/streaming-attlist-composed/)
records 365 workspace Rust checks, selected-allocation failure tests, native C
sanitizer checks, and exact differential comparisons. The full adapted Expat API matrix
reports **4,115 passing and 625 failing configurations**. Allocation schedules,
resource limits, diagnostics and some callback behavior still differ from Expat;
these failures remain recorded.

Earlier [full validation](docs/validation/2026-09-10/final-runtime/) includes
12,318 semantic differential comparisons, W3C acceptance checks, and three
sustained Rust AddressSanitizer campaigns totaling 3.9 million executions without
findings. Earlier [CPython checks](docs/validation/2026-09-10/text-coalescing/)
retain two callback-fragmentation failures behind an explicit semantic gate.
The [PBS distribution](docs/validation/2026-09-10/pbs-final/) passes its installed
XML suites, including on glibc 2.17. Those broad campaigns identify their exact
older runtime; they have not yet been repeated for every subsequent change.

See [fuzzing](fuzz/README.md) for the harnesses and
[compatibility](docs/compatibility.md) for the separate release gates.

## Benchmarks

The [current benchmarks](docs/validation/2026-09-10/streaming-attlist-composed/) compare
Oriole's C interface with Expat 2.8.4 on original, pinned XML files from real
projects. These results use 4 KiB chunks with namespaces disabled, on a shared
Linux AMD EPYC-Milan host. Five randomized process pairs use 7–512 measured
parses per process, depending on input size, after warmup. Complete normalized
callbacks are checked before timing.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 88.774 ms | 28.882 ms | 3.08× |
| Wayland protocol | 2.017 ms | 1.072 ms | 1.92× |
| Maven POM | 1.571 ms | 0.446 ms | 3.53× |
| Batik SVG | 0.153 ms | 0.134 ms | 1.12× |
| GTK UI | 0.680 ms | 0.213 ms | 3.19× |
| DocBook XSL | 0.445 ms | 0.184 ms | 2.40× |

Times are medians of process medians; ratios are medians of paired time ratios.
Oriole remains slower than Expat. Across both namespace modes and 4 KiB/64 KiB
chunks, the ATTLIST change is near parity overall, with 11 of 24 conditions
improving. Its benefits are callback publication and lower memory use. Raw samples,
generated controls and exact source/library hashes are retained; host load and CPU
frequency are uncontrolled.

Matched [CPython and Wayland measurements](docs/validation/2026-09-10/event-output-final/)
use the preceding event-output library. It takes 1.71× Expat's time in ElementTree
and 1.46× in pyexpat on average; Wayland through pyexpat takes 1.06×. Actual Wayland client-header
and private-code generation take 1.24× and 1.31× respectively, with identical output.
See the [benchmark guide](benchmarks/README.md) to reproduce the workloads and
[allocator comparison](benchmarks/results/2026-09-10/real-project-allocators/)
for system/jemalloc measurements.

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
