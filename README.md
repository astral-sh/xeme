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
| Vulkan registry | 36.646 ms | 24.445 ms | 1.50× |
| Wayland protocol | 0.926 ms | 0.945 ms | 0.97× |
| Maven POM | 0.598 ms | 0.362 ms | 1.66× |
| Batik SVG | 0.106 ms | 0.125 ms | 0.85× |
| GTK UI | 0.266 ms | 0.176 ms | 1.52× |
| DocBook XSL | 0.189 ms | 0.166 ms | 1.15× |

These [native measurements](docs/validation/2026-09-11/streaming-input-bounds/) use original XML from six pinned projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host. Both parsers use PGO trained on generated XML; these projects are held out of training. Times are medians of seven process medians; ratios are medians of paired ratios. Expat is version 2.8.4.

Across all 24 real-project conditions, Oriole takes **1.38× Expat PGO's native time and 1.14× its CPython time**. The new streaming policy is effectively flat against the preceding parser: PGO native and CPython time each decrease by 0.3%. Normal native time decreases by 1.4%, while normal CPython time is flat. Generated native controls regress by 1.2% normally and 0.6% with PGO. The [full report](docs/validation/2026-09-11/streaming-input-bounds/) retains every condition, sample and adverse result.

ThinLTO is the default, and [PGO is opt-in](tools/pgo/). The [PGO/LTO study](benchmarks/results/2026-09-11/pgo-lto/) on the preceding `be22a27` runtime measured PGO reductions of 24.8% natively and 15.5% through CPython. Fat LTO alone added little; combining it with PGO regressed native time by 6.8%.

A separate [C allocator study](benchmarks/results/2026-09-11/c-allocators/) finds less than 0.2% aggregate real-project time change with jemalloc or mimalloc, alongside higher peak resident memory. The C allocator default remains unchanged.

The [latest compatibility report](docs/validation/2026-09-11/streaming-input-bounds/) records 420 workspace tests, one doc test, and **4,347 passing / 391 failing / two timed-out upstream API configurations**. Shared and static CPython each retain the same two text-grouping failures across 802 method outcomes. Actual incremental streams through 257 MiB and a separate 2,049 MiB text stream match Expat's checked output and positions with constant tracked allocation peaks. Remaining allocation, resource, diagnostic, and callback differences are explicit in the [compatibility guide](docs/compatibility.md).

Seven [current-source ASan campaigns](docs/validation/2026-09-11/streaming-input-bounds/ASAN.md) completed 2,170,731 executions and 68,897 retained-corpus replays without findings, alongside 13 focused policy regressions. Each campaign ran for 120 seconds; the earlier [six 600-second campaigns](docs/validation/2026-09-11/current-asan/) remain scoped to `be22a27`. The [`be22a27` PBS distribution](docs/validation/2026-09-11/current-pbs/) loads both XML accelerators on glibc 2.17 and passes 1,024 threaded parses; its XML suite retains the same two grouping failures.

The [Windows test portability supplement](docs/validation/2026-09-11/detached-end-windows-portability/) corrects an expected-value cast and verifies byte-identical C libraries.

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
