# Xeme

A streaming XML parser and Expat C interface, written in Rust.

> [!WARNING]
> This project's code, PR summaries, and documentation were authored by AI agents
> in [Codex](https://openai.com/codex/). Xeme is experimental and is not yet a
> production-ready replacement for Expat. Use at your own risk.

## Highlights

- Parse XML incrementally, with namespaces, encodings, entities, and DTD attribute defaults.
- Check XML and inspect parser events from the command line.
- Embed the safe Rust parser or use the Expat-compatible C interface.
- Try an opt-in python-build-standalone integration for CPython's XML consumers.

| Project XML | Xeme (normal) | Expat (normal) | Xeme / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 36.366 ms | 28.906 ms | 1.266× |
| Wayland protocol | 0.887 ms | 1.069 ms | 0.840× |
| Maven POM | 0.589 ms | 0.440 ms | 1.332× |
| Batik SVG | 0.124 ms | 0.134 ms | 0.931× |
| GTK UI | 0.248 ms | 0.212 ms | 1.165× |
| DocBook XSL | 0.225 ms | 0.189 ms | 1.192× |

These [measurements](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-12/native-start-end-namespace-declaration)
use six pinned XML projects, 4 KiB chunks, and namespaces disabled on a shared
Linux host. Times are medians of seven process medians; ratios are medians of
paired ratios. See the [benchmark guide](benchmarks/README.md) to measure a new build.

Validation combines Rust and C tests, differential XML checks, Expat API and
CPython suites, and fuzzing; the [compatibility guide](docs/compatibility.md)
describes remaining gaps.

## Installation

Build from this checkout with Rust 1.96 or later:

```console
cargo install --path crates/xeme_cli --bin xeme --locked
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
xeme message.xml
```

Invalid XML exits with a nonzero status. Omit the filename or pass `-` to read
standard input. Xeme can also print parser events and expand namespaces:

```console
xeme --events message.xml
xeme --events --namespaces --chunk-size 4096 message.xml
```

See the [command-line guide](docs/usage.md), the [Rust library guide](docs/library.md),
or the [Expat interface](crates/xeme_expat/) for embedding and callback contracts.
The parser performs no filesystem or network I/O; applications supply input and
resolve external entities. The CLI does not fetch external resources.

See [contributing](CONTRIBUTING.md) for development and acceptance criteria, and the
[review guide](docs/review.md) for implementation and test entry points.

## License

Xeme is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Xeme
by you, as defined in the Apache-2.0 license, shall be dually licensed as above, without any
additional terms or conditions.
