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
| Vulkan registry | 52.248 ms | 29.862 ms | 1.74× |
| Wayland protocol | 1.233 ms | 1.078 ms | 1.15× |
| Maven POM | 0.850 ms | 0.443 ms | 1.91× |
| Batik SVG | 0.134 ms | 0.134 ms | 1.00× |
| GTK UI | 0.387 ms | 0.213 ms | 1.83× |
| DocBook XSL | 0.274 ms | 0.185 ms | 1.47× |

These [native measurements](benchmarks/results/2026-09-11/detached-end-frames/) use original XML from six pinned projects, Expat 2.8.4, 4 KiB chunks, and namespaces disabled on a shared Linux AMD EPYC-Milan host. Times are medians of process medians; ratios are medians of paired ratios.

Detaching matched closing-tag names into callback frames reduces elapsed time by 4.0% across the 24 native project conditions and 2.1% through CPython. Both Oriole builds use effective ThinLTO. Oriole still takes 1.64× Expat's time natively and 1.29× through CPython; three Wayland CPython conditions are faster than Expat in this measurement. Generated controls regress by 2.5% overall, with entity input regressing by 3.3–6.2%. The [full report](docs/validation/2026-09-11/detached-end-frames/) retains all conditions, samples and regressions.

Optional [profile-guided builds](tools/pgo/) are available. The [earlier PGO results](benchmarks/results/2026-09-10/version-consistent-pgo/) measure `4b11ace`; they do not measure these new parser changes.

The [latest compatibility report](docs/validation/2026-09-11/detached-end-frames/) records 407 workspace tests, one doc test, and **4,347 passing / 393 failing upstream API configurations**. Shared and static CPython each execute 802 tests with the same two text-grouping failures. Remaining allocation, resource, diagnostic, and callback differences are explicit in the [compatibility guide](docs/compatibility.md). Six [sustained ASan campaigns](docs/validation/2026-09-10/detached-frames-fuzz/) completed 14.62 million executions on `5bc806e` without findings; these precede the latest parser changes. The [PBS distribution report](docs/validation/2026-09-10/version-consistent-pbs/) applies to the earlier `4b11ace` runtime.

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
