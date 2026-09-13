# Benchmarks

Use the [performance iteration guide](HILLCLIMB.md) for new work. Its
`hillclimb.py` entrypoint freezes Cargo's emitted artifact and compares parent,
candidate and Expat in matched process rounds. It retains raw workers and hashes,
checks outputs before timing, and reports every condition.

## Interpreting results

The goal is roughly within 20% of Expat through both the C interface and CPython.
The six [project inputs](projects/README.md) are a tuning set: repeated optimization
has used them. The separate holdout is selected before candidate measurements and
must remain separate in reports. Use `verify-holdout` to check pinned original
bytes without parsing or timing them, and record the candidate selection before
running holdout measurements. After inspection, that holdout can no longer serve
as unseen evidence for later tuning.

Native timing includes parser creation, callbacks, parsing and destruction;
process startup and input loading are outside the measured region. CPython timings
use matched real `pyexpat` and ElementTree consumers. Compare namespace modes and
chunk sizes separately. Use medians of paired ratios for each condition and
geometric means across declared condition groups; keep generated stress cases out
of project aggregates. Shared-host CPU affinity does not establish isolation or
statistical significance.

The [last merged QName confirmation](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-13/qname-confirmation)
records 1.1739× Expat's native project time and 1.0504× its CPython time on the tuning
set. Those numbers identify that source and corpus. New fixes, unseen projects and
installed distributions need separate measurements. Previously committed evidence
and the full QName confirmation workers are in [checksummed archives](../docs/evidence/README.md).

## Build and allocator policy

Ordinary performance builds use optimization level 3, ThinLTO and one codegen unit.
Record Expat's compiler and flags independently. PGO is no longer an optimization
workstream; historical PGO results remain in the archive.

Library consumers select their allocator. The CLI uses jemalloc on supported Unix
platforms and mimalloc on Windows. The safe Rust benchmark supports system,
jemalloc and mimalloc builds; freeze each executable in a separate output before
building another variant:

```console
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml --features jemalloc
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml --features mimalloc
```

Keep distinct `CARGO_TARGET_DIR` values and a separate shared Ohm build directory.
The in-process executable accepts `XML_FILE CHUNK_SIZE ITERATIONS`. Its
`allocation-counts` feature measures allocation/reallocation requests and requested
bytes using atomics; run that separately from uninstrumented timings. Requested
bytes are not peak live memory.

```console
python3 benchmarks/allocators.py \
  --binary system=/absolute/bench-system --binary jemalloc=/absolute/bench-jemalloc \
  --binary mimalloc=/absolute/bench-mimalloc --counts /absolute/bench-counts \
  --inputs /absolute/xml-inputs --build-manifest /absolute/build.json \
  --output /tmp/allocator-results
```

## Specialized workloads

| Entry point | Purpose |
| --- | --- |
| [`run.py`](run.py) | Generated C ABI workloads, callback preflight, namespace on/off |
| [`projects`](projects/RERUN.md) | Native project XML, real CPython consumers and complete Wayland code generation |
| [`dtd_workload.py`](dtd_workload.py) | Declaration/default-attribute lookup and repeated elements |
| [`dtd_scaling.py`](dtd_scaling.py) | DTD composition with local external resources and declaration callbacks |
| [`allocators.py`](allocators.py) | Paired safe Rust executable/allocator comparisons |

The DTD driver compares serialized declaration metadata and frees complete content
models before timing. Its digest does not establish exact positions or Default
callback behavior. Keep those compatibility checks separate.

Every campaign should retain source, compiler, build commands, features, allocator,
input/header/driver hashes, selected-library hashes, preflight outcomes, raw samples
and worker failures. Write fresh output directories; do not pool separate epochs
or omit adverse rows. [Evidence storage](../docs/evidence/README.md) explains how to
publish large campaigns without expanding the source tree.
