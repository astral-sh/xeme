# Benchmarks

Use [hillclimb.py](HILLCLIMB.md) to build and compare a baseline, candidate and
Expat in matched rounds. It reports project and generated inputs separately.

## Corpus selection

The six project files are a tuning set used for repeated optimization.
The [2026-09-13 pre-rebase evaluation](../docs/evidence/2026-09-13-review.md#performance)
tested runtime `ec4d068`: 1.1906× Expat's native time and 1.0489× its CPython time
on that set. Five independently selected projects instead reported **1.7867×
native and 1.2158× CPython**, missing the 1.20× goal. Most of that gap already
existed in the measured baseline.

The [five-project holdout](holdout/README.md) is now a regression corpus. Report
all its conditions separately; a new independent evaluation needs new inputs.
These historical results do not measure later revisions.
[Checksummed archives](../docs/evidence/README.md) retain the full raw evidence.

## Running benchmarks

Build Xeme with ordinary `-O3`, ThinLTO and one codegen unit, and the Expat
control with `-O3` without LTO, as described in the performance iteration guide.
Keep the compiler and build settings fixed throughout each comparison.

The native C driver compares the Expat-compatible ABI against system Expat with
identical element and character-data callbacks. The Python runner performs an
untimed comparison of complete normalized callbacks before measuring, shuffles
paired process order, and records every observation and source/input/library hash.

```console
python3 benchmarks/run.py --library /path/to/libxeme_expat.so \
  --output /tmp/xeme-benchmark
```

`run.py` generates the inputs and delegates measurement to `projects.py`. It keeps
the XML, `corpus.json`, and build-manifest copies in the output directory. Results
now use the shared runner's schema at `results/summary.json`, replacing the former
top-level `summary.json`: condition keys include `/namespaces-0` or
`/namespaces-1`, and sample rows use `chunk` instead of `chunk_size`. Console
output reports status and worker counts; timing ratios are in the summary file.
Preflight callbacks and worker logs are retained alongside the summary.

Each process measures ten complete parses after one discarded warmup. Creation,
handler registration, parsing, callbacks, and parser destruction are included.
Process startup, library loading, and input loading are excluded. The driver
hashes names, attributes, and text independently of text callback fragmentation.
The corpus contains elements, text, entity references, and prefixed names; the
default timing configuration disables namespace processing in both libraries.
Pass `--namespaces` to enable namespace expansion in both the complete callback
preflight and the timed C driver; the report records the selected mode. Keep
results from the two modes separate.

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
executables unchanged throughout the comparison.

`dtd_workload.py` generates separate declaration-only and repeated-element inputs
with 128–1,024 declared attributes. It exercises attribute type and ID lookup while
remaining below the parser's resource limits. The native callback digest omits
DTD declaration and ID metadata; deterministic regressions check those contracts
separately. Compare complete normalized callbacks outside the timed region too.

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
  --library /absolute/libxeme_expat.so --reference /absolute/libexpat.so \
  --inputs /tmp/dtd-inputs --driver /tmp/dtd-driver \
  --build-manifest /absolute/build.json --output /tmp/dtd-results
```

Include header and driver hashes in the build manifest as well. The report retains
the manifest hash, samples, preflight metadata, worker failures and input/library hashes.
The optional `--external-grammar` generator flag adds repeated empty external
references inside declarations; use it only for implementations supporting that
mode. Positions and Default callbacks remain separate compatibility checks.

## Real-project inputs and consumers

The [pinned corpus](projects/README.md) contains XML from Vulkan, Wayland, Maven,
Batik, GTK and DocBook. The runners compare native callbacks, CPython consumers
built against each library, and complete Wayland code-generation commands. They
validate outputs before reporting timings. See the
[reproduction commands](projects/RERUN.md).

## Historical profile-guided builds

Current comparisons do not use PGO.

The optional [PGO workflow](../tools/pgo/) builds an instrumented library, trains on
a fixed generated corpus, and rebuilds with a fresh profile. It preserves runtime
configuration and keeps real project XML out of training. Every run records source,
compiler, profile and library identities.
