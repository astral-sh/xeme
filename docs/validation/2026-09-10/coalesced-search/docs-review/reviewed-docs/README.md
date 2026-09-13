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
| Vulkan registry | 67.187 ms | 28.823 ms | 2.33× |
| Wayland protocol | 1.464 ms | 1.066 ms | 1.39× |
| Maven POM | 1.153 ms | 0.442 ms | 2.61× |
| Batik SVG | 0.145 ms | 0.133 ms | 1.08× |
| GTK UI | 0.497 ms | 0.213 ms | 2.33× |
| DocBook XSL | 0.340 ms | 0.186 ms | 1.82× |

These [native measurements](benchmarks/results/2026-09-10/coalesced-search/) use original XML from six pinned projects, Expat 2.8.4, 4 KiB chunks, and namespaces disabled on a shared Linux AMD EPYC-Milan host. Times are medians of process medians; ratios are medians of paired ratios.

The latest text-scanning change reduces elapsed time by 3.3% across the 24 native project conditions and 2.4% across the 24 actual CPython conditions versus `5bc806e`. Oriole still takes 2.04× Expat's time natively and 1.45× through CPython. The two Wayland pyexpat conditions are faster than Expat. The [full report](docs/validation/2026-09-10/coalesced-search/) retains all conditions and samples, including the native and Python regressions.

Optional [profile-guided builds](tools/pgo/) are available. The [earlier PGO results](benchmarks/results/2026-09-10/version-consistent-pgo/) measure `4b11ace`; they do not measure these new parser changes.

The [latest compatibility report](docs/validation/2026-09-10/coalesced-search/) records 394 workspace tests and **4,347 passing / 393 failing upstream API configurations**. Shared and static CPython each execute 802 tests with the same two text-grouping failures. Remaining allocation, resource, diagnostic, and callback differences are explicit in the [compatibility guide](docs/compatibility.md). Six [sustained ASan campaigns](docs/validation/2026-09-10/version-consistent-fuzz/) completed 14.50 million executions on the preceding `4b11ace` runtime without findings. The [PBS distribution report](docs/validation/2026-09-10/version-consistent-pbs/) also applies to that earlier runtime.

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
