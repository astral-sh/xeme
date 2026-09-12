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
| Vulkan registry | 50.623 ms | 29.073 ms | 1.751× |
| Wayland protocol | 1.177 ms | 1.066 ms | 1.104× |
| Maven POM | 0.847 ms | 0.444 ms | 1.914× |
| Batik SVG | 0.134 ms | 0.134 ms | 0.997× |
| GTK UI | 0.360 ms | 0.213 ms | 1.686× |
| DocBook XSL | 0.272 ms | 0.185 ms | 1.463× |

These [native measurements](docs/validation/2026-09-12/expanded-start/) use the expanded namespace Start runtime and original XML from six pinned projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host. Both parsers use normal release builds: Oriole uses O3 and ThinLTO with Ohm rustc 1.98.1-dev; Expat 2.8.4 uses GCC 13.3 O3 without LTO. Times are medians of seven process medians; ratios are medians of paired ratios.

Across their respective 24 real-project conditions, the candidate takes **1.54× Expat's native time and 1.24× its CPython time**, improvements of 3.40% and 2.58% against the previous selected runtime. Both remain above our target of roughly 1.10×; four of 24 individual CPython conditions fall within it. The [full report](docs/validation/2026-09-12/expanded-start/) retains every condition and regression.

ThinLTO is the default. The earlier [C allocator study](benchmarks/results/2026-09-11/c-allocators/) found less than 0.2% aggregate time change with jemalloc or mimalloc and higher peak resident memory, so the C allocator default remains unchanged.

Current-source compatibility checks preserve **4,347 passing / 391 failing / two timed-out API configurations**, plus two text-grouping failures across 802 strict CPython method outcomes per linkage. Six C consumers and supplemental shared/static callback-semantic checks pass. The [compatibility guide](docs/compatibility.md) separates current gates from earlier results; the [review guide](docs/review.md) links source reviews, adversarial tests and measurements.

Earlier [PGO measurements](docs/validation/2026-09-11/reference-frames/) and [opt-in x86-64-v3 results](docs/validation/2026-09-11/reference-v3/) describe the previous `4064b065` source. They have not been remeasured for this change. Its [sanitizer and PBS validation](docs/validation/2026-09-11/reference-validation/) and [installed v3 distribution trial](docs/validation/2026-09-12/v3-distribution/), including glibc 2.17 probes, also apply only to that previous source.

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
