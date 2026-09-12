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

| Project XML | Oriole (normal) | Expat (normal) | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 42.999 ms | 28.767 ms | 1.500× |
| Wayland protocol | 0.996 ms | 1.076 ms | 0.929× |
| Maven POM | 0.710 ms | 0.438 ms | 1.622× |
| Batik SVG | 0.129 ms | 0.134 ms | 0.961× |
| GTK UI | 0.296 ms | 0.216 ms | 1.375× |
| DocBook XSL | 0.247 ms | 0.191 ms | 1.295× |

These [normal-build measurements](docs/validation/2026-09-12/empty-state-guards/) use six pinned XML projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host, with elapsed work pinned to CPU0. Oriole uses Ohm rustc 1.98.1-dev with O3/ThinLTO; Expat uses GCC 13.3 O3 without LTO. Times are medians of seven process medians; ratios are medians of paired ratios. Across the 24 real-project conditions in each confirmation run, Oriole takes **1.34× Expat’s native time and 1.14× its CPython time**, above the roughly 1.10× goal.

Current compatibility checks preserve **4,347 passing / 391 failing / two timed-out API configurations** and two strict CPython text-grouping failures. Six C consumers and supplemental semantic checks pass. The [compatibility guide](docs/compatibility.md) records the remaining production-readiness gates and separates current results from historical validation.

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
