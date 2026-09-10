# Oriole

A streaming XML parser and Expat C interface, written in Rust.

> [!WARNING]
> This project's code, PR summaries, and documentation were authored by AI agents
> in [Codex](https://openai.com/codex/). Oriole is experimental and is not yet a
> production-ready replacement for Expat. Use at your own risk.

## Highlights

- Parse XML incrementally, with namespaces, encodings, entities, and DTD attribute defaults.
- Check XML and inspect parser events from the command line.
- Embed the safe Rust parser or use the Expat-compatible C interface.
- Try an opt-in python-build-standalone integration for CPython's XML consumers.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 88.774 ms | 28.882 ms | 3.08× |
| Wayland protocol | 2.017 ms | 1.072 ms | 1.92× |
| Maven POM | 1.571 ms | 0.446 ms | 3.53× |
| Batik SVG | 0.153 ms | 0.134 ms | 1.12× |
| GTK UI | 0.680 ms | 0.213 ms | 3.19× |
| DocBook XSL | 0.445 ms | 0.184 ms | 2.40× |

Oriole remains slower than Expat. These [native measurements](docs/validation/2026-09-10/streaming-attlist-composed/)
use original XML from six pinned projects, Expat 2.8.4, 4 KiB chunks, and namespaces
disabled on a shared Linux AMD EPYC-Milan host. Times are medians of process medians;
ratios are medians of paired ratios. See the [benchmark guide](benchmarks/README.md)
for methodology, complete results, and separately identified actual-consumer timings.

The [current validation report](docs/validation/2026-09-10/streaming-attlist-composed/)
records 365 workspace Rust checks and **4,115 passing / 625 failing upstream API
configurations**. Remaining allocation, resource, diagnostic, and callback differences
are explicit in the [compatibility guide](docs/compatibility.md). Earlier fuzzing and
PBS distribution reports identify their own tested revisions.

## Installation

Build from this checkout with Rust 1.96 or later:

```console
cargo install --path crates/oriole_cli --bin oriole --locked
```

The executable uses jemalloc on supported Unix platforms and mimalloc on Windows.
Library consumers choose their own allocator. See the [command-line guide](docs/usage.md)
for allocator options, input handling, and resource limits.

## Getting started

Given `message.xml`:

```xml
<message>Hello &amp; goodbye</message>
```

Check the document:

```console
oriole message.xml
```

Invalid XML exits with a nonzero status. Omit the filename or pass `-` to read
standard input. Oriole can also print parser events and expand namespaces:

```console
oriole --events message.xml
oriole --events --namespaces --chunk-size 4096 message.xml
```

See the [command-line guide](docs/usage.md), the [Rust library guide](docs/library.md),
or the [Expat interface](crates/oriole_expat/) for embedding and callback contracts.
The parser performs no filesystem or network I/O; applications supply input and
resolve external entities. The CLI does not fetch external resources.

See [contributing](CONTRIBUTING.md) for development and acceptance criteria, and the
[stack review guide](docs/review.md) for implementation and validation evidence.

## License

Oriole is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oriole
by you, as defined in the Apache-2.0 license, shall be dually licensed as above, without any
additional terms or conditions.
