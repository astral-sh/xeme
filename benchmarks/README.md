# Benchmarks

See the [initial measurements](results/2026-09-10/README.md) for results, source
archives, raw observations, and limitations.

The native C driver compares the Expat-compatible ABI against system Expat with
identical element and character-data callbacks. The Python runner performs an
untimed comparison of complete normalized callbacks before measuring, shuffles
paired process order, and records every observation and source/input/library hash.

```console
python3 benchmarks/run.py --library /path/to/liboriole_expat.so \
  --output /tmp/oriole-benchmark
```

Each process measures ten complete parses after one discarded warmup. Creation,
handler registration, parsing, callbacks, and parser destruction are included.
Process startup, library loading, and input loading are excluded. The driver
hashes names, attributes, and text independently of text callback fragmentation.
The corpus contains elements, text, entity references, and prefixed names; the
native timing configuration uses namespace processing disabled in both libraries.
All numbers are generated-workload measurements on the recorded host.

## Allocators and the safe Rust API

`inprocess` is a separate workspace using the safe Rust parser API with the same
streaming callback digest. Build each configuration to a distinct target directory
and copy the resulting binaries before rebuilding. The default uses the system
allocator; `jemalloc` and `mimalloc` are explicit alternatives.

```console
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml --features jemalloc
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml --features mimalloc
```

The executable accepts `XML_FILE CHUNK_SIZE ITERATIONS` and emits JSON containing
one warmup and all measured samples. Keep results from distinct builds separate.
`allocation-counts` counts allocation/reallocation requests and requested bytes;
it does not count unique live bytes. It uses atomics and must be measured in a
separate run from uninstrumented timings. Library consumers choose their allocator.

Compare copied executables with randomized paired process order:

```console
python3 benchmarks/allocators.py \
  --binary system=/absolute/bench-system \
  --binary jemalloc=/absolute/bench-jemalloc \
  --binary mimalloc=/absolute/bench-mimalloc \
  --counts /absolute/bench-counts --inputs /absolute/xml-inputs \
  --build-manifest /absolute/build.json --output /tmp/allocator-results
```

The first executable is the baseline. Labels may also identify a parent and a
candidate build for an optimization. Both runners accept repeated
`--build-manifest` arguments and retain copies and hashes with their results.
Manifests should identify source hashes, the compiler, build commands, features,
and the resulting executable hashes. Build before measuring and keep source and
executables unchanged throughout each campaign.

`dtd_workload.py` generates separate declaration-only and repeated-element inputs
with 128–1,024 declared attributes. It exercises attribute type and ID lookup while
remaining below the parser's resource limits. The native callback digest omits
DTD declaration and ID metadata; deterministic regressions check those contracts
separately. Compare complete normalized callbacks outside the timed region too.

No benchmark result is a compatibility or production-readiness certification.
