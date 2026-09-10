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
| Vulkan registry | 65.120 ms | 29.120 ms | 2.23× |
| Wayland protocol | 1.360 ms | 1.073 ms | 1.26× |
| Maven POM | 1.047 ms | 0.439 ms | 2.39× |
| Batik SVG | 0.138 ms | 0.134 ms | 1.04× |
| GTK UI | 0.452 ms | 0.213 ms | 2.11× |
| DocBook XSL | 0.327 ms | 0.185 ms | 1.77× |

These [native measurements](benchmarks/results/2026-09-10/end-tag-integration/) use original XML from six pinned projects, Expat 2.8.4, 4 KiB chunks, and namespaces disabled on a shared Linux AMD EPYC-Milan host. Times are medians of process medians; ratios are medians of paired ratios.

The latest closing-tag change reduces elapsed time by 5.1% across the 24 native project conditions and 2.8% across the 24 actual CPython conditions versus `64a270e`. Oriole still takes 1.91× Expat's time natively and 1.38× through CPython. The two Wayland pyexpat conditions are faster than Expat. The [full report](docs/validation/2026-09-10/end-tag-integration/) retains all conditions and samples, including regressions in the earlier isolated study.

Optional [profile-guided builds](tools/pgo/) are available. The [earlier PGO results](benchmarks/results/2026-09-10/version-consistent-pgo/) measure `4b11ace`; they do not measure these new parser changes.

The [latest compatibility report](docs/validation/2026-09-10/end-tag-integration/) records 397 workspace tests, one doc test, and **4,347 passing / 393 failing upstream API configurations**. Shared and static CPython each execute 802 tests with the same two text-grouping failures. Remaining allocation, resource, diagnostic, and callback differences are explicit in the [compatibility guide](docs/compatibility.md). Six [sustained ASan campaigns](docs/validation/2026-09-10/detached-frames-fuzz/) completed 14.62 million executions on `5bc806e` without findings; these precede the latest parser changes. The [PBS distribution report](docs/validation/2026-09-10/version-consistent-pbs/) applies to the earlier `4b11ace` runtime.

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
