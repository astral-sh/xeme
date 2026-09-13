# Benchmarks

See the [current native and consumer measurements](results/2026-09-11/native-byte-count/),
[earlier matched PGO measurements](results/2026-09-10/version-consistent-pgo/),
[earlier full runtime measurements](results/2026-09-10/final-runtime/),
[DTD runtime checkpoint](results/2026-09-10/dtd-checkpoint/),
[earlier combined runtime measurements](results/2026-09-10/combined-runtime/),
[inline text experiment](results/2026-09-10/inline-text/),
[earlier validated checkpoint](results/2026-09-10/checkpoint/),
[explicit namespace measurements](results/2026-09-10/namespaces/),
[callback allocation measurements](results/2026-09-10/callback-allocation/), and
[initial optimization comparisons](results/2026-09-10/README.md) for results, source
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
default timing configuration disables namespace processing in both libraries.
Pass `--namespaces` to enable namespace expansion in both the complete callback
preflight and the timed C driver; the report records the selected mode. Keep
results from the two modes separate.
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

## DTD composition and incremental scanning

`dtd_composition_workload.py` generates declaration delimiters supplied by internal
parameter entities, conditional headers, repeated empty replacements, and long
quoted literals. `native_dtd_driver.c` adds an in-memory external DTD resolver and
declaration callbacks, including complete content model traversal and freeing.
The Linux runner compares exact serialized callback metadata before timing and
checks every timed digest and declaration/request count against that preflight.

```console
python3 benchmarks/dtd_composition_workload.py --output /tmp/dtd-inputs
cc -std=c11 -O3 -Wall -Wextra -Werror -I include \
  benchmarks/native_dtd_driver.c -ldl -o /tmp/dtd-driver
python3 benchmarks/dtd_scaling.py \
  --library /absolute/liboriole_expat.so --reference /absolute/libexpat.so \
  --inputs /tmp/dtd-inputs --driver /tmp/dtd-driver \
  --build-manifest /absolute/build.json --output /tmp/dtd-results
```

As with the other runners, the build manifest should identify the parser source,
compiler, commands, header and driver hashes. The report retains its hash along
with all samples, preflight metadata, worker failures, and input/library hashes.
The optional `--external-grammar` generator flag adds repeated empty external
references inside declarations; use it only for implementations supporting that
mode. Positions and Default callbacks remain separate compatibility checks.

## Real-project inputs and consumers

The [pinned corpus](projects/README.md) contains original XML from Vulkan, Wayland, Maven, Batik, GTK and DocBook. The runners compare native callbacks, matched CPython consumers and complete Wayland code-generation commands, validating all outputs before reporting timings. See the [baseline results](results/2026-09-10/real-project-baseline/README.md) and [reproduction commands](projects/RERUN.md).

## Profile-guided builds

The optional [PGO workflow](../tools/pgo/) builds an instrumented library, trains on
a fixed generated corpus, and rebuilds with a fresh profile. It preserves runtime
configuration and keeps real project XML out of training. Every run records source,
compiler, profile and library identities.

The [current matched-build study](results/2026-09-10/version-consistent-pgo/) measures
runtime `4b11ace` through native callbacks, unchanged CPython consumers and the
Wayland scanner. PGO reduces Oriole's native time by 22.7% and Expat's by 13.4%;
with both trained, Oriole takes 2.23× Expat's time overall. The corresponding
comparisons are 1.55× for ElementTree, 1.30× for pyexpat's event interface and 1.25×
for Wayland code generation. All output gates, raw samples, independent arithmetic
reviews, invalid overlapping attempts and exact build identities are preserved.

The [initial paired study](results/2026-09-10/profile-guided-study/) improves Oriole
by 22.2% geometrically on held-out native project workloads. Expat trained on the
same generated corpus improves by 12.7%; the fair PGO comparison still takes 2.26×
Expat's time overall. Those measurements identify an earlier source and do not
certify newer builds. The [rejected optimization studies](results/2026-09-10/rejected-position-entity-studies/)
retain unsuccessful position and entity candidates, including memory regressions
fixed during review.
