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

| Project XML | Oriole (PGO) | Expat (PGO) | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 35.853 ms | 24.132 ms | 1.47× |
| Wayland protocol | 0.906 ms | 0.947 ms | 0.95× |
| Maven POM | 0.575 ms | 0.360 ms | 1.58× |
| Batik SVG | 0.104 ms | 0.125 ms | 0.83× |
| GTK UI | 0.255 ms | 0.176 ms | 1.44× |
| DocBook XSL | 0.187 ms | 0.164 ms | 1.14× |

These [native measurements](docs/validation/2026-09-11/reference-frames/) use the selected character-reference frame runtime and original XML from six pinned projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host. Both parsers use PGO trained on generated XML; these projects are held out of training. Times are medians of seven process medians; ratios are medians of paired ratios. Expat is version 2.8.4.

The selected runtime takes **1.33× Expat PGO's native time and 1.12× its CPython time** across the respective 24 real-project conditions. Both exceed our target of roughly 1.10×; nine of 24 individual CPython PGO conditions fall within it. The [full report](docs/validation/2026-09-11/reference-frames/) retains every condition, sample and regression.

ThinLTO is the default, and [PGO is opt-in](tools/pgo/). The [C allocator study](benchmarks/results/2026-09-11/c-allocators/) finds less than 0.2% aggregate time change with jemalloc or mimalloc and higher peak resident memory, so the C allocator default remains unchanged.

Compatibility testing records **4,347 passing / 391 failing / two timed-out upstream API configurations**, plus two text-grouping failures across 802 CPython method outcomes per linkage. The [compatibility guide](docs/compatibility.md) explains the remaining allocation, resource, diagnostic and callback differences. The [review guide](docs/review.md) links the adversarial tests, sanitizer campaigns, distribution trials and optimization studies.

Fresh [selected-runtime validation](docs/validation/2026-09-11/reference-validation/) adds 9.94 million sanitizer-backed fuzz executions without findings and a PGO CPython distribution that runs on glibc 2.17. Its strict XML gates retain the same two callback assertions.

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
